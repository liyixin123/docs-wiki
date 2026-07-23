//! The translation pipeline: protecting code blocks from the model,
//! chunking long documents, running them through a [`TranslationProvider`],
//! and a structural self-check on the result. `translate_doc` ties this to
//! disk/state; `translate_document`/`translate_doc_with_provider` take an
//! injected provider so the pipeline logic is fully unit-testable without
//! a real API key.

use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::config::load_config;
use crate::provider::{build_provider, TranslationProvider};
use crate::sources::doc_content_path;
use crate::state::{now_iso, AppStateData, DocMeta, LogEntry, LogKind, TranslationStatus};

/// Extract fenced code blocks (` ``` ` or `~~~`) and replace each with a
/// `[[CODEBLOCK_n]]` placeholder line, so only prose gets sent to the
/// model. Far more reliable than asking the model not to touch code via
/// prompt instructions alone — it structurally can't.
fn protect_code_blocks(markdown: &str) -> (String, Vec<String>) {
    let mut blocks = Vec::new();
    let mut out = String::new();
    let mut in_block = false;
    let mut fence = "";
    let mut current = String::new();

    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if !in_block && (trimmed.starts_with("```") || trimmed.starts_with("~~~")) {
            in_block = true;
            fence = if trimmed.starts_with("```") { "```" } else { "~~~" };
            current.clear();
            current.push_str(line);
            current.push('\n');
        } else if in_block {
            current.push_str(line);
            current.push('\n');
            if trimmed.starts_with(fence) {
                in_block = false;
                out.push_str(&format!("[[CODEBLOCK_{}]]\n", blocks.len()));
                blocks.push(std::mem::take(&mut current));
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    // An unterminated fence (malformed input) — keep whatever we collected
    // as trailing prose rather than losing it.
    if in_block {
        out.push_str(&current);
    }
    (out, blocks)
}

fn restore_code_blocks(translated: &str, blocks: &[String]) -> String {
    let mut result = translated.to_string();
    for (idx, block) in blocks.iter().enumerate() {
        let placeholder = format!("[[CODEBLOCK_{idx}]]");
        result = result.replace(&placeholder, block.trim_end_matches('\n'));
    }
    result
}

/// Split into chunks at `##` heading boundaries once the document exceeds
/// `threshold` characters; short documents stay a single chunk.
fn split_into_chunks(markdown: &str, threshold: usize) -> Vec<String> {
    if markdown.len() <= threshold {
        return vec![markdown.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    for line in markdown.lines() {
        if line.trim_start().starts_with("## ") && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
        }
        current.push_str(line);
        current.push('\n');
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// Heading / code-fence / table counts must match before and after
/// translation — a mismatch means the model added, dropped, or mangled
/// structure, and the result needs a human to look at it.
fn structural_check_passes(original: &str, translated: &str) -> bool {
    count_headings(original) == count_headings(translated)
        && count_code_fences(original) == count_code_fences(translated)
        && count_table_rows(original) == count_table_rows(translated)
}

fn count_headings(markdown: &str) -> usize {
    markdown.lines().filter(|l| l.trim_start().starts_with('#')).count()
}

fn count_code_fences(markdown: &str) -> usize {
    markdown
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("```") || t.starts_with("~~~")
        })
        .count()
}

/// Counts markdown table separator rows (`|---|---|`), a reasonable proxy
/// for "how many tables" without a full table parser.
fn count_table_rows(markdown: &str) -> usize {
    markdown
        .lines()
        .filter(|l| {
            let t = l.trim();
            t.starts_with('|') && t.len() > 1 && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
        })
        .count()
}

/// Run the full pipeline (chunk -> protect -> translate -> restore) over a
/// document, returning the translated markdown and whether the structural
/// self-check passed.
pub async fn translate_document(
    provider: &dyn TranslationProvider,
    markdown: &str,
    chunk_threshold: usize,
) -> Result<(String, bool)> {
    let chunks = split_into_chunks(markdown, chunk_threshold);
    let mut translated_chunks = Vec::with_capacity(chunks.len());
    for chunk in &chunks {
        let (protected, blocks) = protect_code_blocks(chunk);
        let translated = provider.translate_chunk(&protected).await?;
        translated_chunks.push(restore_code_blocks(&translated, &blocks));
    }
    let full = translated_chunks.join("");
    let passes = structural_check_passes(markdown, &full);
    Ok((full, passes))
}

/// Translate one doc using an already-built provider — the disk/state-
/// mutating core, kept separate from provider construction so tests can
/// inject a fake provider instead of needing a real API key.
async fn translate_doc_with_provider(
    dir: &Path,
    data: &mut AppStateData,
    source_id: &str,
    doc_id: &str,
    provider: &dyn TranslationProvider,
    chunk_threshold: usize,
    provider_id: &str,
) -> Result<DocMeta> {
    let source = data
        .sources
        .iter()
        .find(|s| s.id == source_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("未知的来源 '{source_id}'"))?;
    if source.languages.len() < 2 {
        bail!("此来源是单语言来源，不支持翻译");
    }
    let target_lang = source
        .languages
        .iter()
        .find(|l| l.as_str() != source.primary_language)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("找不到目标翻译语言"))?;

    let source_path = doc_content_path(dir, source_id, &source.primary_language, doc_id);
    let original =
        std::fs::read_to_string(&source_path).with_context(|| format!("reading {source_path:?}"))?;

    let (translated, passes) = match translate_document(provider, &original, chunk_threshold).await {
        Ok(result) => result,
        Err(e) => {
            // Record the failure in the history log so the user can diagnose
            // provider errors (bad model, proxy shape, empty response…) from
            // the history view — the transient UI error alone isn't enough.
            // The caller persists `data` on both success and failure paths.
            let now = now_iso();
            data.log.push(LogEntry {
                id: format!("translate-fail-{source_id}-{doc_id}-{now}"),
                ts: now,
                source_id: source_id.to_string(),
                doc_id: Some(doc_id.to_string()),
                kind: LogKind::TranslateError,
                detail: format!("翻译 '{doc_id}' 失败：{e}"),
                snapshot: None,
            });
            return Err(e);
        }
    };

    let now = now_iso();
    let dest_path = doc_content_path(dir, source_id, &target_lang, doc_id);
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Snapshot the previous translation (if any) *before* overwriting, so the
    // history UI can diff what this re-translation changed. A first-time
    // translation has no prior file, so no snapshot and the entry stays
    // unclickable.
    let snapshot = if dest_path.exists() {
        let old = std::fs::read_to_string(&dest_path)
            .with_context(|| format!("reading old content at {dest_path:?}"))?;
        Some(crate::snapshot::save_snapshot(dir, source_id, &target_lang, doc_id, &now, &old)?)
    } else {
        None
    };
    std::fs::write(&dest_path, &translated).with_context(|| format!("writing {dest_path:?}"))?;

    let doc = data
        .docs
        .iter_mut()
        .find(|d| d.source_id == source_id && d.id == doc_id)
        .ok_or_else(|| anyhow::anyhow!("未知的文档 '{doc_id}'"))?;
    doc.translation_status = if passes { TranslationStatus::Translated } else { TranslationStatus::NeedsReview };
    doc.translated_at = Some(now.clone());
    doc.translated_by = Some(provider_id.to_string());
    let updated = doc.clone();

    data.log.push(LogEntry {
        id: format!("translate-{source_id}-{doc_id}-{now}"),
        ts: now,
        source_id: source_id.to_string(),
        doc_id: Some(doc_id.to_string()),
        kind: if passes { LogKind::Translate } else { LogKind::TranslateError },
        detail: if passes {
            format!("Translated '{doc_id}' into {target_lang}")
        } else {
            format!("Translated '{doc_id}' into {target_lang}, but the structural self-check failed — needs manual review")
        },
        snapshot,
    });

    Ok(updated)
}

/// Translate one doc using the app's configured active provider. Does not
/// persist `data` — the caller saves once the lock is released.
pub async fn translate_doc(dir: &Path, data: &mut AppStateData, source_id: &str, doc_id: &str) -> Result<DocMeta> {
    let cfg = load_config(dir)?;
    let provider_id = cfg
        .active_provider
        .clone()
        .ok_or_else(|| anyhow::anyhow!("尚未设置默认翻译服务，请先在设置里选择"))?;
    let provider_cfg = cfg
        .providers
        .get(&provider_id)
        .ok_or_else(|| anyhow::anyhow!("找不到 provider '{provider_id}' 的配置"))?;
    let provider = build_provider(provider_cfg)?;

    translate_doc_with_provider(
        dir,
        data,
        source_id,
        doc_id,
        provider.as_ref(),
        cfg.translation.chunk_threshold_chars,
        &provider_id,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_import::import_local_folder;
    use async_trait::async_trait;

    /// A fake provider that just upper-cases its input — lets us verify the
    /// pipeline (protection/chunking/restoration/self-check) without any
    /// network access or API key.
    struct UppercaseProvider;

    #[async_trait]
    impl TranslationProvider for UppercaseProvider {
        async fn translate_chunk(&self, markdown: &str) -> Result<String> {
            Ok(markdown.to_uppercase())
        }
        async fn test_connection(&self) -> Result<()> {
            Ok(())
        }
    }

    /// A fake provider that always fails — lets us verify that provider
    /// errors land in the history log instead of vanishing with the
    /// discarded in-memory snapshot.
    struct FailingProvider;

    #[async_trait]
    impl TranslationProvider for FailingProvider {
        async fn translate_chunk(&self, _markdown: &str) -> Result<String> {
            bail!("Anthropic 响应里没有找到文本内容")
        }
        async fn test_connection(&self) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn protect_and_restore_code_blocks_round_trips_exactly() {
        let markdown = "prose before\n\n```rust\nfn main() {}\n```\n\nprose after\n";
        let (protected, blocks) = protect_code_blocks(markdown);
        assert_eq!(blocks.len(), 1);
        assert!(protected.contains("[[CODEBLOCK_0]]"));
        assert!(!protected.contains("fn main"));

        let restored = restore_code_blocks(&protected, &blocks);
        assert_eq!(restored, markdown);
    }

    #[test]
    fn split_into_chunks_respects_threshold_and_heading_boundaries() {
        let short = "# Title\n\nShort doc.";
        assert_eq!(split_into_chunks(short, 1000).len(), 1);

        let long = format!("# Title\n\n{}\n\n## Section\n\n{}", "a".repeat(50), "b".repeat(50));
        let chunks = split_into_chunks(&long, 60);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].contains("# Title"));
        assert!(chunks[1].starts_with("## Section"));
    }

    #[test]
    fn structural_check_detects_a_dropped_table() {
        let original = "# Title\n\n| a | b |\n|---|---|\n| 1 | 2 |\n";
        let same_structure = "# 标题\n\n| a | b |\n|---|---|\n| 1 | 2 |\n";
        let dropped_table = "# 标题\n\n1、2\n";
        assert!(structural_check_passes(original, same_structure));
        assert!(!structural_check_passes(original, dropped_table));
    }

    #[tokio::test]
    async fn translate_document_uppercases_prose_but_not_code_and_passes_check() {
        let markdown = "# Title\n\nSome prose.\n\n```rust\nfn main() {}\n```\n";
        let (translated, passes) = translate_document(&UppercaseProvider, markdown, 10_000).await.unwrap();
        assert!(translated.contains("# TITLE"));
        assert!(translated.contains("SOME PROSE"));
        assert!(translated.contains("fn main() {}")); // code untouched, not uppercased
        assert!(passes);
    }

    #[tokio::test]
    async fn translate_doc_with_provider_writes_target_language_file_and_updates_status() {
        let app_dir = tempfile::tempdir().unwrap();
        let upstream = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(upstream.path().join("en")).unwrap();
        std::fs::write(upstream.path().join("en/intro.md"), "# Intro\n\nHello world.").unwrap();
        std::fs::create_dir_all(upstream.path().join("zh")).unwrap();
        // Some other doc is already translated, so "zh" is recognized as a
        // real language directory — but intro.md specifically isn't yet.
        std::fs::write(upstream.path().join("zh/other.md"), "# 其它").unwrap();

        let mut data = AppStateData::default();
        let source =
            import_local_folder(app_dir.path(), &mut data, upstream.path(), Some("Bilingual".to_string())).unwrap();
        assert_eq!(
            data.docs.iter().find(|d| d.id == "intro").unwrap().translation_status,
            TranslationStatus::NeverTranslated
        );

        let updated = translate_doc_with_provider(
            app_dir.path(),
            &mut data,
            &source.id,
            "intro",
            &UppercaseProvider,
            10_000,
            "fake-provider",
        )
        .await
        .unwrap();

        assert_eq!(updated.translation_status, TranslationStatus::Translated);
        assert_eq!(updated.translated_by, Some("fake-provider".to_string()));
        let on_disk =
            std::fs::read_to_string(app_dir.path().join("sources").join(&source.id).join("docs/zh/intro.md"))
                .unwrap();
        assert!(on_disk.contains("HELLO WORLD"));

        // First translation: no prior zh file existed, so nothing to snapshot.
        let entry = data
            .log
            .iter()
            .rev()
            .find(|l| matches!(l.kind, LogKind::Translate | LogKind::TranslateError))
            .unwrap();
        assert!(entry.snapshot.is_none());
    }

    #[tokio::test]
    async fn translate_doc_with_provider_rejects_single_language_sources() {
        let app_dir = tempfile::tempdir().unwrap();
        let upstream = tempfile::tempdir().unwrap();
        std::fs::write(upstream.path().join("intro.md"), "# Intro").unwrap();

        let mut data = AppStateData::default();
        let source = import_local_folder(app_dir.path(), &mut data, upstream.path(), None).unwrap();

        let err = translate_doc_with_provider(
            app_dir.path(),
            &mut data,
            &source.id,
            "intro",
            &UppercaseProvider,
            10_000,
            "fake-provider",
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("单语言"));
    }

    #[tokio::test]
    async fn retranslate_snapshots_the_previous_translation_before_overwriting() {
        let app_dir = tempfile::tempdir().unwrap();
        let upstream = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(upstream.path().join("en")).unwrap();
        std::fs::write(upstream.path().join("en/intro.md"), "# Intro\n\nHello world.").unwrap();
        std::fs::create_dir_all(upstream.path().join("zh")).unwrap();
        std::fs::write(upstream.path().join("zh/other.md"), "# 其它").unwrap();

        let mut data = AppStateData::default();
        let source =
            import_local_folder(app_dir.path(), &mut data, upstream.path(), Some("Bilingual".to_string())).unwrap();

        // First translation creates zh/intro.md (no prior file -> no snapshot).
        let _ = translate_doc_with_provider(
            app_dir.path(),
            &mut data,
            &source.id,
            "intro",
            &UppercaseProvider,
            10_000,
            "fake-provider",
        )
        .await
        .unwrap();
        // Second translation overwrites it -> snapshots the first (uppercased) version.
        let _ = translate_doc_with_provider(
            app_dir.path(),
            &mut data,
            &source.id,
            "intro",
            &UppercaseProvider,
            10_000,
            "fake-provider",
        )
        .await
        .unwrap();

        let entry = data
            .log
            .iter()
            .rev()
            .find(|l| matches!(l.kind, LogKind::Translate | LogKind::TranslateError))
            .unwrap();
        let snap = entry.snapshot.as_ref().expect("re-translation should snapshot the prior translation");
        assert_eq!(snap.lang, "zh");
        let old = crate::snapshot::read_snapshot(app_dir.path(), &source.id, snap).unwrap();
        assert!(old.contains("HELLO WORLD"), "snapshot holds the pre-retranslation content");
    }

    #[tokio::test]
    async fn provider_failure_is_recorded_in_the_history_log() {
        let app_dir = tempfile::tempdir().unwrap();
        let upstream = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(upstream.path().join("en")).unwrap();
        std::fs::write(upstream.path().join("en/intro.md"), "# Intro\n\nHello world.").unwrap();
        std::fs::create_dir_all(upstream.path().join("zh")).unwrap();
        std::fs::write(upstream.path().join("zh/other.md"), "# 其它").unwrap();

        let mut data = AppStateData::default();
        let source =
            import_local_folder(app_dir.path(), &mut data, upstream.path(), Some("Bilingual".to_string())).unwrap();

        let err = translate_doc_with_provider(
            app_dir.path(),
            &mut data,
            &source.id,
            "intro",
            &FailingProvider,
            10_000,
            "fake-provider",
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("没有找到文本内容"));

        // The failure is logged (so the history view can show *why*), the doc
        // status is untouched, and no half-written translation file exists.
        let entry = data.log.iter().rev().find(|l| l.kind == LogKind::TranslateError).unwrap();
        assert_eq!(entry.doc_id.as_deref(), Some("intro"));
        assert!(entry.detail.contains("没有找到文本内容"));
        assert!(entry.snapshot.is_none());
        assert_eq!(
            data.docs.iter().find(|d| d.id == "intro").unwrap().translation_status,
            TranslationStatus::NeverTranslated
        );
        assert!(!app_dir
            .path()
            .join("sources")
            .join(&source.id)
            .join("docs/zh/intro.md")
            .exists());
    }
}
