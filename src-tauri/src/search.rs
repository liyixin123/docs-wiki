//! Wiki-style full-text keyword search.
//!
//! Matching is ASCII-case-insensitive substring search (not tokenized/stemmed):
//! good enough at hundreds-of-docs scale and keeps every byte offset trivially
//! valid for UTF-8 slicing (ASCII-only case folding never changes byte length,
//! so lowercase and original strings stay byte-aligned). Multiple query terms
//! are AND'd — a term "matches" a doc if it appears in the title, a heading,
//! or the body; a doc must satisfy every term to be a hit.
//!
//! [`SearchBackend`] is a trait so a future SQLite FTS5 implementation can
//! replace [`InMemorySearch`] without changing the Tauri command surface.

use std::path::Path;

use serde::Serialize;

use crate::docs::read_doc_content;
use crate::state::AppStateData;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snippet {
    pub text: String,
    /// Char-boundary-safe byte offsets into `text` marking matched terms.
    pub highlight_ranges: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub source_id: String,
    pub doc_id: String,
    pub lang: String,
    pub title: String,
    pub score: f64,
    pub snippets: Vec<Snippet>,
}

pub trait SearchBackend {
    fn search(&self, query: &str, source_id: Option<&str>, lang: Option<&str>, limit: usize) -> Vec<SearchHit>;
}

struct IndexedDoc {
    source_id: String,
    doc_id: String,
    lang: String,
    title: String,
    title_lower: String,
    headings_lower: Vec<String>,
    body: String,
    body_lower: String,
}

/// In-memory index built fresh from whatever's on disk right now. Rebuilding
/// per search is deliberate: at this dataset size (dozens to low hundreds of
/// docs) it's cheap, and it sidesteps needing cache-invalidation logic before
/// any command actually mutates documents on disk (P3/P4/P6).
pub struct InMemorySearch {
    docs: Vec<IndexedDoc>,
}

impl InMemorySearch {
    pub fn build(dir: &Path, data: &AppStateData) -> Self {
        let mut docs = Vec::new();
        for doc_meta in &data.docs {
            let Some(source) = data.sources.iter().find(|s| s.id == doc_meta.source_id) else {
                continue;
            };
            for lang in &source.languages {
                let Ok(content) = read_doc_content(dir, &doc_meta.source_id, lang, &doc_meta.id) else {
                    continue;
                };
                let headings_lower = content
                    .lines()
                    .filter(|l| l.trim_start().starts_with('#'))
                    .map(|l| l.trim_start_matches('#').trim().to_ascii_lowercase())
                    .collect();
                docs.push(IndexedDoc {
                    source_id: doc_meta.source_id.clone(),
                    doc_id: doc_meta.id.clone(),
                    lang: lang.clone(),
                    title: doc_meta.title.clone(),
                    title_lower: doc_meta.title.to_ascii_lowercase(),
                    headings_lower,
                    body_lower: content.to_ascii_lowercase(),
                    body: content,
                });
            }
        }
        Self { docs }
    }
}

impl SearchBackend for InMemorySearch {
    fn search(&self, query: &str, source_id: Option<&str>, lang: Option<&str>, limit: usize) -> Vec<SearchHit> {
        let terms: Vec<String> = query
            .split_whitespace()
            .map(|t| t.to_ascii_lowercase())
            .filter(|t| !t.is_empty())
            .collect();
        if terms.is_empty() {
            return Vec::new();
        }

        let mut hits: Vec<SearchHit> = self
            .docs
            .iter()
            .filter(|d| source_id.is_none_or(|sid| d.source_id == sid))
            .filter(|d| lang.is_none_or(|l| d.lang == l))
            .filter_map(|d| score_doc(d, &terms))
            .collect();

        hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        hits.truncate(limit);
        hits
    }
}

/// Score a doc against all query terms (title hit > heading hit > body
/// frequency), or return `None` if any term is missing entirely (AND).
fn score_doc(doc: &IndexedDoc, terms: &[String]) -> Option<SearchHit> {
    const TITLE_WEIGHT: f64 = 100.0;
    const HEADING_WEIGHT: f64 = 50.0;

    let mut score = 0.0;
    for term in terms {
        let in_title = doc.title_lower.contains(term.as_str());
        let in_heading = doc.headings_lower.iter().any(|h| h.contains(term.as_str()));
        let body_count = doc.body_lower.matches(term.as_str()).count();
        if !in_title && !in_heading && body_count == 0 {
            return None;
        }
        if in_title {
            score += TITLE_WEIGHT;
        }
        if in_heading {
            score += HEADING_WEIGHT;
        }
        score += body_count as f64;
    }

    Some(SearchHit {
        source_id: doc.source_id.clone(),
        doc_id: doc.doc_id.clone(),
        lang: doc.lang.clone(),
        title: doc.title.clone(),
        score,
        snippets: build_snippets(&doc.body, &doc.body_lower, terms, 3),
    })
}

const SNIPPET_CONTEXT_BYTES: usize = 60;

/// Every byte position that starts a term match in `body_lower`, sorted and deduped.
fn all_match_positions(body_lower: &str, terms: &[String]) -> Vec<usize> {
    let mut positions = Vec::new();
    for term in terms {
        let mut cursor = 0;
        while let Some(rel) = body_lower[cursor..].find(term.as_str()) {
            let abs = cursor + rel;
            positions.push(abs);
            cursor = abs + term.len().max(1);
        }
    }
    positions.sort_unstable();
    positions.dedup();
    positions
}

/// Build up to `max_snippets` non-overlapping context windows around match
/// locations, each with highlight ranges for every term found inside it.
fn build_snippets(body: &str, body_lower: &str, terms: &[String], max_snippets: usize) -> Vec<Snippet> {
    let mut snippets = Vec::new();
    let mut last_end = 0usize;

    for pos in all_match_positions(body_lower, terms) {
        if pos < last_end {
            continue; // already covered by a previous snippet's window
        }
        let start = floor_char_boundary(body, pos.saturating_sub(SNIPPET_CONTEXT_BYTES));
        let end = ceil_char_boundary(body, (pos + SNIPPET_CONTEXT_BYTES).min(body.len()));

        let window = &body[start..end];
        let window_lower = &body_lower[start..end];
        let mut ranges: Vec<(usize, usize)> = Vec::new();
        for term in terms {
            let mut cursor = 0;
            while let Some(rel) = window_lower[cursor..].find(term.as_str()) {
                let abs = cursor + rel;
                ranges.push((abs, abs + term.len()));
                cursor = abs + term.len().max(1);
            }
        }
        ranges.sort_by_key(|r| r.0);

        let prefix = if start > 0 { "…" } else { "" };
        let suffix = if end < body.len() { "…" } else { "" };
        let offset = prefix.len();
        let text = format!("{prefix}{window}{suffix}");
        let ranges = ranges.into_iter().map(|(s, e)| (s + offset, e + offset)).collect();

        snippets.push(Snippet { text, highlight_ranges: ranges });
        last_end = end;
        if snippets.len() >= max_snippets {
            break;
        }
    }
    snippets
}

fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn ceil_char_boundary(s: &str, mut idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::doc_content_path;
    use crate::state::{CheckStatus, DocMeta, Source, SourceKind, TranslationStatus};

    fn write_doc(dir: &Path, source_id: &str, lang: &str, id: &str, content: &str) {
        let path = doc_content_path(dir, source_id, lang, id);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn make_doc_meta(id: &str, title: &str) -> DocMeta {
        DocMeta {
            source_id: "demo".to_string(),
            id: id.to_string(),
            title: title.to_string(),
            category: "Docs".to_string(),
            primary_hash: "h".to_string(),
            last_checked_at: None,
            last_check_status: CheckStatus::Unknown,
            translation_status: TranslationStatus::Translated,
            translated_at: None,
            translated_by: None,
        }
    }

    fn sample_state() -> (tempfile::TempDir, AppStateData) {
        let tmp = tempfile::tempdir().unwrap();
        write_doc(
            tmp.path(),
            "demo",
            "en",
            "alpha",
            "# Alpha Guide\n\nThis document explains widgets and gadgets in depth.",
        );
        write_doc(
            tmp.path(),
            "demo",
            "en",
            "beta",
            "# Beta\n\nNo relevant keywords here at all.",
        );
        write_doc(
            tmp.path(),
            "demo",
            "zh",
            "alpha",
            "# Alpha 指南\n\n这里详细介绍小工具和配件的使用方法，包括安装、配置和常见问题排查等内容，适合初学者阅读。",
        );

        let source = Source {
            id: "demo".to_string(),
            name: "Demo".to_string(),
            kind: SourceKind::Seed,
            languages: vec!["en".to_string(), "zh".to_string()],
            primary_language: "en".to_string(),
            remote: None,
            local_path: None,
            order_override: vec![],
            created_at: "t".to_string(),
            updated_at: "t".to_string(),
        };
        let docs = vec![make_doc_meta("alpha", "Alpha Guide"), make_doc_meta("beta", "Beta")];
        (tmp, AppStateData { sources: vec![source], docs, log: vec![] })
    }

    #[test]
    fn finds_body_match_and_builds_highlighted_snippet() {
        let (tmp, data) = sample_state();
        let index = InMemorySearch::build(tmp.path(), &data);
        let hits = index.search("widgets", None, None, 10);

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].doc_id, "alpha");
        assert_eq!(hits[0].lang, "en");
        let snippet = &hits[0].snippets[0];
        let (s, e) = snippet.highlight_ranges[0];
        assert_eq!(snippet.text[s..e].to_ascii_lowercase(), "widgets");
    }

    #[test]
    fn and_semantics_require_every_term_to_match() {
        let (tmp, data) = sample_state();
        let index = InMemorySearch::build(tmp.path(), &data);
        let hits = index.search("widgets nonexistentterm", None, None, 10);
        assert!(hits.is_empty());
    }

    #[test]
    fn title_match_outranks_body_only_match() {
        let (tmp, data) = sample_state();
        let index = InMemorySearch::build(tmp.path(), &data);
        let hits = index.search("alpha", None, Some("en"), 10);
        assert_eq!(hits[0].doc_id, "alpha");
        assert!(hits[0].score >= 100.0);
    }

    #[test]
    fn scopes_by_source_and_lang() {
        let (tmp, data) = sample_state();
        let index = InMemorySearch::build(tmp.path(), &data);

        let hits = index.search("alpha", None, Some("zh"), 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].lang, "zh");

        let hits_wrong_source = index.search("alpha", Some("other-source"), None, 10);
        assert!(hits_wrong_source.is_empty());
    }

    #[test]
    fn respects_limit() {
        let (tmp, data) = sample_state();
        let index = InMemorySearch::build(tmp.path(), &data);
        let hits = index.search("e", None, Some("en"), 1);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn handles_multibyte_utf8_context_and_highlighting() {
        let (tmp, data) = sample_state();
        let index = InMemorySearch::build(tmp.path(), &data);
        let hits = index.search("小工具", None, Some("zh"), 10);

        assert_eq!(hits.len(), 1);
        let snippet = &hits[0].snippets[0];
        let (s, e) = snippet.highlight_ranges[0];
        assert_eq!(&snippet.text[s..e], "小工具");
    }
}
