//! Navigation tree: parsing a source's category/order manifest and applying
//! the user's manual drag-to-reorder override on top of it.
//!
//! Order is resolved by layered fallback (highest priority first):
//! 1. `Source.order_override` (user dragged items around) — applied by
//!    [`apply_order_override`].
//! 2. An explicit link-list manifest file (`SUMMARY.md` / `_sidebar.md` /
//!    a `README.md` index), parsed by [`parse_link_manifest`]. This is the
//!    format the bundled Pi source's `zh/README.md` already uses.
//! 3. (Future) front-matter `order:`/`category:` fields.
//! 4. Filesystem structure with natural sort ([`natural_cmp`]) — see
//!    `sources::build_fallback_categories`, used for local-folder imports
//!    that ship no manifest file.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavItem {
    pub doc_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavCategory {
    pub name: String,
    pub items: Vec<NavItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavTree {
    pub source_id: String,
    pub categories: Vec<NavCategory>,
}

/// Turn a raw doc id (a filename stem, possibly with `/`-separated path
/// segments) into a human-readable fallback title when no manifest or
/// heading gives us a better one, e.g. `"custom-provider"` -> `"custom
/// provider"`.
pub fn prettify_id(id: &str) -> String {
    id.replace(['-', '_'], " ")
}

/// Parse a manifest markdown file that looks like:
///
/// ```markdown
/// ## Category Name
///
/// - [index.md](index.md) - Overview
/// - [quickstart.md](quickstart.md) - Getting started
/// ```
///
/// `##` headings become categories; each `- [text](target)` link becomes a
/// nav item, using the trailing " - description" text as the display title
/// when present (falling back to the link text otherwise). Links whose
/// target isn't a `.md` file, or that point at `README.md` itself, are
/// skipped (the manifest file links to itself in some conventions).
pub fn parse_link_manifest(markdown: &str) -> Vec<NavCategory> {
    let mut categories: Vec<NavCategory> = Vec::new();
    let mut current: Option<NavCategory> = None;

    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix("## ") {
            if let Some(cat) = current.take() {
                categories.push(cat);
            }
            current = Some(NavCategory {
                name: name.trim().to_string(),
                items: Vec::new(),
            });
        } else if trimmed.starts_with("- [") {
            if let Some((link_text, target, desc)) = parse_md_link(trimmed) {
                if target.ends_with(".md") && !target.eq_ignore_ascii_case("README.md") {
                    let doc_id = target.trim_end_matches(".md").to_string();
                    let title = if !desc.is_empty() {
                        desc
                    } else {
                        link_text.trim_end_matches(".md").to_string()
                    };
                    if let Some(cat) = current.as_mut() {
                        cat.items.push(NavItem { doc_id, title });
                    }
                }
            }
        }
    }
    if let Some(cat) = current.take() {
        categories.push(cat);
    }
    categories
}

/// Parse a single markdown link line, returning `(link_text, target, trailing_description)`.
/// `trailing_description` is whatever follows the `)` with a leading `-` stripped,
/// e.g. `- [index.md](index.md) - Overview` -> `("index.md", "index.md", "Overview")`.
fn parse_md_link(line: &str) -> Option<(String, String, String)> {
    let start = line.find('[')?;
    let mid = line[start..].find(']')? + start;
    let text = &line[start + 1..mid];

    let rest = &line[mid + 1..];
    let pstart = rest.find('(')?;
    let pend = rest.find(')')?;
    let target = &rest[pstart + 1..pend];
    let after = &rest[pend + 1..];
    let desc = after.trim().trim_start_matches('-').trim().to_string();

    Some((text.trim().to_string(), target.trim().to_string(), desc))
}

/// Reorder items within each category to match `order_override` (a flat list
/// of doc ids). Items not mentioned in the override keep their relative
/// order and are placed after the overridden ones. An empty override leaves
/// the tree untouched.
pub fn apply_order_override(categories: &mut [NavCategory], order_override: &[String]) {
    if order_override.is_empty() {
        return;
    }
    let rank = |id: &str| -> usize {
        order_override
            .iter()
            .position(|o| o == id)
            .unwrap_or(usize::MAX)
    };
    for cat in categories.iter_mut() {
        cat.items.sort_by(|a, b| {
            let (ra, rb) = (rank(&a.doc_id), rank(&b.doc_id));
            match ra.cmp(&rb) {
                Ordering::Equal => Ordering::Equal,
                other => other,
            }
        });
    }
}

/// Natural sort comparator for filenames with numeric prefixes, so
/// `2-setup.md` sorts before `10-advanced.md` (plain lexical order would put
/// `10-` first).
#[allow(dead_code)] // convenience wrapper; production code sorts NavItems directly via natural_cmp
pub fn natural_sort_ids(mut ids: Vec<String>) -> Vec<String> {
    ids.sort_by(|a, b| natural_cmp(a, b));
    ids
}

/// Exposed so callers building a fallback nav tree from raw filesystem
/// structure (no manifest file present) can sort their own item/category
/// lists without duplicating this comparator.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut ac = a.chars().peekable();
    let mut bc = b.chars().peekable();
    loop {
        match (ac.peek(), bc.peek()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let na = take_number(&mut ac);
                let nb = take_number(&mut bc);
                match na.cmp(&nb) {
                    Ordering::Equal => continue,
                    other => return other,
                }
            }
            (Some(x), Some(y)) => match x.cmp(y) {
                Ordering::Equal => {
                    ac.next();
                    bc.next();
                    continue;
                }
                other => return other,
            },
        }
    }
}

fn take_number(iter: &mut std::iter::Peekable<std::str::Chars>) -> u64 {
    let mut n: u64 = 0;
    while let Some(d) = iter.peek().and_then(|c| c.to_digit(10)) {
        n = n * 10 + d as u64;
        iter.next();
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# Index

## 入门

- [index.md](index.md) - 文档首页 / 总览
- [quickstart.md](quickstart.md) - 快速开始

## 定制化

- [extensions.md](extensions.md) - 扩展（TypeScript API）
";

    #[test]
    fn parses_categories_and_items_in_order() {
        let cats = parse_link_manifest(SAMPLE);
        assert_eq!(cats.len(), 2);
        assert_eq!(cats[0].name, "入门");
        assert_eq!(cats[0].items.len(), 2);
        assert_eq!(cats[0].items[0].doc_id, "index");
        assert_eq!(cats[0].items[0].title, "文档首页 / 总览");
        assert_eq!(cats[1].name, "定制化");
        assert_eq!(cats[1].items[0].doc_id, "extensions");
    }

    #[test]
    fn skips_self_referencing_readme_links() {
        let md = "## Cat\n\n- [README.md](README.md) - self\n- [a.md](a.md) - A\n";
        let cats = parse_link_manifest(md);
        assert_eq!(cats[0].items.len(), 1);
        assert_eq!(cats[0].items[0].doc_id, "a");
    }

    #[test]
    fn order_override_reorders_within_category() {
        let mut cats = parse_link_manifest(SAMPLE);
        apply_order_override(&mut cats, &["quickstart".to_string(), "index".to_string()]);
        assert_eq!(cats[0].items[0].doc_id, "quickstart");
        assert_eq!(cats[0].items[1].doc_id, "index");
    }

    #[test]
    fn empty_override_is_noop() {
        let mut cats = parse_link_manifest(SAMPLE);
        let before: Vec<_> = cats[0].items.iter().map(|i| i.doc_id.clone()).collect();
        apply_order_override(&mut cats, &[]);
        let after: Vec<_> = cats[0].items.iter().map(|i| i.doc_id.clone()).collect();
        assert_eq!(before, after);
    }

    #[test]
    fn natural_sort_orders_numeric_prefixes_numerically() {
        let ids = vec![
            "10-advanced".to_string(),
            "2-setup".to_string(),
            "1-intro".to_string(),
        ];
        let sorted = natural_sort_ids(ids);
        assert_eq!(sorted, vec!["1-intro", "2-setup", "10-advanced"]);
    }

    #[test]
    fn prettify_id_replaces_separators_with_spaces() {
        assert_eq!(prettify_id("custom-provider"), "custom provider");
        assert_eq!(prettify_id("session_format"), "session format");
    }
}
