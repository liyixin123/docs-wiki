//! Pluggable LLM provider adapter used for translation (P6) and the
//! settings page's "test connection" button. Adding a new provider kind
//! only means implementing this trait and routing to it by `kind` in
//! `build_provider` — nothing else in the app needs to change.

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;

use crate::config::{resolve_key, ProviderConfig, ProviderKind};

/// The translation instructions used for every chunk sent to a provider.
/// Kept identical across providers so translation quality doesn't depend on
/// which one the user picked.
pub const TRANSLATE_SYSTEM_PROMPT: &str = "翻译成简体中文技术文档风格；关键的技术名词、专有名词在首次出现时采用「中文（英文）」的形式同时保留英文，例如：会话（Session）、智能体（Agent）、提示词（Prompt），之后可直接使用中文；产品、项目、库、工具的固有英文名称（如 GitHub、Docker、Claude Code）保持原样、不要硬译；代码块、行内代码、文件路径、环境变量名、命令/参数名、JSON 字段名、URL 一律保留原文；标题/表格/代码围栏结构与原文一一对应，不得增删内容；表格只翻译描述性单元格，内部链接的 .md 相对路径保持不变。";

#[async_trait]
pub trait TranslationProvider: Send + Sync {
    async fn translate_chunk(&self, markdown: &str) -> Result<String>;
    async fn test_connection(&self) -> Result<()>;
}

/// Build the right adapter for a provider's configured `kind`, resolving
/// its API key expression ($ENV_VAR / !command / literal) up front so a
/// misconfigured key fails fast with a clear error.
pub fn build_provider(config: &ProviderConfig) -> Result<Box<dyn TranslationProvider>> {
    let api_key = resolve_key(&config.api_key).context("resolving API key")?;
    let client = Client::new();
    match config.kind {
        ProviderKind::AnthropicMessages => Ok(Box::new(AnthropicProvider {
            client,
            base_url: config.base_url.clone(),
            api_key,
            model: config.model.clone(),
            max_tokens: config.max_output_tokens,
        })),
        ProviderKind::OpenAiCompatible => Ok(Box::new(OpenAiCompatProvider {
            client,
            base_url: config.base_url.clone(),
            api_key,
            model: config.model.clone(),
            max_tokens: config.max_output_tokens,
        })),
    }
}

struct AnthropicProvider {
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
    max_tokens: u32,
}

#[async_trait]
impl TranslationProvider for AnthropicProvider {
    async fn translate_chunk(&self, markdown: &str) -> Result<String> {
        let body = json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            "system": TRANSLATE_SYSTEM_PROMPT,
            "messages": [{"role": "user", "content": markdown}],
        });
        let value = self.post_messages(&body).await?;
        extract_anthropic_text(&value)
    }

    async fn test_connection(&self) -> Result<()> {
        let body = json!({
            "model": self.model,
            "max_tokens": 8,
            "messages": [{"role": "user", "content": "ping"}],
        });
        self.post_messages(&body).await?;
        Ok(())
    }
}

impl AnthropicProvider {
    async fn post_messages(&self, body: &serde_json::Value) -> Result<serde_json::Value> {
        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(body)
            .send()
            .await
            .context("请求 Anthropic Messages API 失败")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            bail!("Anthropic API 返回 {status}：{text}");
        }
        resp.json().await.context("解析 Anthropic 响应失败")
    }
}

struct OpenAiCompatProvider {
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
    max_tokens: u32,
}

#[async_trait]
impl TranslationProvider for OpenAiCompatProvider {
    async fn translate_chunk(&self, markdown: &str) -> Result<String> {
        let body = json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            "messages": [
                {"role": "system", "content": TRANSLATE_SYSTEM_PROMPT},
                {"role": "user", "content": markdown},
            ],
        });
        let value = self.post_chat_completions(&body).await?;
        extract_openai_text(&value)
    }

    async fn test_connection(&self) -> Result<()> {
        let body = json!({
            "model": self.model,
            "max_tokens": 8,
            "messages": [{"role": "user", "content": "ping"}],
        });
        self.post_chat_completions(&body).await?;
        Ok(())
    }
}

impl OpenAiCompatProvider {
    async fn post_chat_completions(&self, body: &serde_json::Value) -> Result<serde_json::Value> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(body)
            .send()
            .await
            .context("请求 OpenAI 兼容接口失败")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            bail!("接口返回 {status}：{text}");
        }
        resp.json().await.context("解析响应失败")
    }
}

/// Extract the translated text from an Anthropic Messages response by
/// concatenating every `text` block in `content`, in order. The first block
/// is NOT guaranteed to be text: when extended thinking is on (some models
/// or relaying proxies), `content[0]` is a `thinking` block with no `text`
/// field, so taking only block 0 would fail even though the translation
/// succeeded. When no text block exists, the error carries a preview of the
/// actual response so the failure is diagnosable from the UI/log alone.
fn extract_anthropic_text(value: &serde_json::Value) -> Result<String> {
    let content = value
        .get("content")
        .and_then(|c| c.as_array())
        .context("Anthropic 响应缺少 content 数组")?;
    let text = join_text_blocks(content);
    if text.is_empty() {
        bail!(
            "Anthropic 响应里没有找到文本内容（content 共 {} 块）。响应预览：{}",
            content.len(),
            preview_json(value)
        );
    }
    Ok(text)
}

/// Extract text from an OpenAI-compatible chat-completions response. Handles
/// both the standard string `content` and the array-of-parts form some
/// gateways return; errors carry a response preview for the same reason as
/// [`extract_anthropic_text`].
fn extract_openai_text(value: &serde_json::Value) -> Result<String> {
    let content = value
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"));
    let text = match content {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(parts)) => join_text_blocks(parts),
        _ => String::new(),
    };
    if text.is_empty() {
        bail!("响应里没有找到文本内容。响应预览：{}", preview_json(value));
    }
    Ok(text)
}

/// Concatenate the `text` fields of every block whose `type` is `"text"`.
fn join_text_blocks(blocks: &[serde_json::Value]) -> String {
    blocks
        .iter()
        .filter(|block| block.get("type").and_then(|t| t.as_str()) == Some("text"))
        .filter_map(|block| block.get("text").and_then(|t| t.as_str()))
        .collect::<Vec<_>>()
        .join("")
}

/// A short (char-bounded) JSON preview of a provider response, embedded in
/// "no text found" errors so the actual API response is visible without
/// separate logging.
fn preview_json(value: &serde_json::Value) -> String {
    const MAX_CHARS: usize = 300;
    let s = serde_json::to_string(value).unwrap_or_else(|_| "<无法序列化>".to_string());
    if s.chars().count() <= MAX_CHARS {
        s
    } else {
        format!("{}…", s.chars().take(MAX_CHARS).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProviderConfig;

    #[test]
    fn build_provider_fails_fast_on_unresolvable_key() {
        std::env::remove_var("DOCSWIKI_TEST_MISSING_KEY");
        let cfg = ProviderConfig {
            kind: ProviderKind::AnthropicMessages,
            base_url: "https://api.anthropic.com".to_string(),
            api_key: "$DOCSWIKI_TEST_MISSING_KEY".to_string(),
            model: "claude-sonnet-4-5".to_string(),
            max_output_tokens: 8192,
        };
        assert!(build_provider(&cfg).is_err());
    }

    #[test]
    fn build_provider_succeeds_with_a_literal_key() {
        let cfg = ProviderConfig {
            kind: ProviderKind::OpenAiCompatible,
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "sk-literal".to_string(),
            model: "gpt-4o".to_string(),
            max_output_tokens: 8192,
        };
        assert!(build_provider(&cfg).is_ok());
    }

    #[test]
    fn extract_anthropic_text_takes_a_plain_text_block() {
        let value = json!({"content": [{"type": "text", "text": "你好世界"}]});
        assert_eq!(extract_anthropic_text(&value).unwrap(), "你好世界");
    }

    #[test]
    fn extract_anthropic_text_skips_thinking_blocks() {
        // Extended thinking puts a `thinking` block first; the text is later.
        let value = json!({"content": [
            {"type": "thinking", "thinking": "pondering…"},
            {"type": "text", "text": "译文"}
        ]});
        assert_eq!(extract_anthropic_text(&value).unwrap(), "译文");
    }

    #[test]
    fn extract_anthropic_text_concatenates_multiple_text_blocks() {
        let value = json!({"content": [
            {"type": "text", "text": "第一段"},
            {"type": "text", "text": "第二段"}
        ]});
        assert_eq!(extract_anthropic_text(&value).unwrap(), "第一段第二段");
    }

    #[test]
    fn extract_anthropic_text_error_carries_a_response_preview() {
        let value = json!({"content": [{"type": "thinking", "thinking": "pondering"}], "stop_reason": "max_tokens"});
        let msg = extract_anthropic_text(&value).unwrap_err().to_string();
        assert!(msg.contains("没有找到文本内容"));
        assert!(msg.contains("pondering"), "error must embed the response for diagnosis");
    }

    #[test]
    fn extract_anthropic_text_errors_when_content_array_missing() {
        // e.g. an OpenAI-shaped response posted to an Anthropic-typed provider.
        let value = json!({"choices": [{"message": {"content": "x"}}]});
        assert!(extract_anthropic_text(&value).is_err());
    }

    #[test]
    fn extract_openai_text_handles_string_and_array_content() {
        let string_form = json!({"choices": [{"message": {"content": "译文"}}]});
        assert_eq!(extract_openai_text(&string_form).unwrap(), "译文");

        let array_form = json!({"choices": [{"message": {"content": [
            {"type": "text", "text": "译"},
            {"type": "text", "text": "文"}
        ]}}]});
        assert_eq!(extract_openai_text(&array_form).unwrap(), "译文");

        let no_text = json!({"choices": [{"message": {"content": ""}}]});
        assert!(extract_openai_text(&no_text).unwrap_err().to_string().contains("响应预览"));
    }
}
