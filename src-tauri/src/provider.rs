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
pub const TRANSLATE_SYSTEM_PROMPT: &str = "翻译成简体中文技术文档风格；代码块、行内代码、文件路径、环境变量名、命令/参数名、JSON 字段名、URL 一律保留原文；标题/表格/代码围栏结构与原文一一对应，不得增删内容；表格只翻译描述性单元格，内部链接的 .md 相对路径保持不变。";

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
        value
            .get("content")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .map(str::to_string)
            .context("Anthropic 响应里没有找到文本内容")
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
        value
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|t| t.as_str())
            .map(str::to_string)
            .context("响应里没有找到文本内容")
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
}
