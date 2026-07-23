//! Line-level diffing of a snapshot against the current doc content, computed
//! server-side via `similar` so the frontend needs no JS diff library and the
//! two view modes (inline red/green vs. side-by-side) render the same
//! structured rows.

use serde::Serialize;
use similar::{ChangeTag, TextDiff};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")] // -> "add" / "del" / "ctx"
pub enum DiffLineKind {
    /// A line present only in the new content.
    Add,
    /// A line present only in the old content.
    Del,
    /// An unchanged context line.
    Ctx,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: DiffLineKind,
    /// 1-based line number in the old content (`None` for pure additions).
    pub old_number: Option<usize>,
    /// 1-based line number in the new content (`None` for pure deletions).
    pub new_number: Option<usize>,
    pub text: String,
}

/// Compute a line-level diff from `old` to `new`. `similar`'s LCS-based diff
/// produces minimal Delete/Insert/Equal changes; we flatten them into rows
/// carrying 1-based old/new line numbers for the frontend to render. Trailing
/// newlines are stripped from each row's text so a row is exactly one line.
pub fn compute_diff(old: &str, new: &str) -> Vec<DiffLine> {
    let diff = TextDiff::from_lines(old, new);
    diff.iter_all_changes()
        .map(|change| {
            let (kind, old_number, new_number) = match change.tag() {
                ChangeTag::Delete => {
                    (DiffLineKind::Del, change.old_index().map(|i| i + 1), None)
                }
                ChangeTag::Insert => {
                    (DiffLineKind::Add, None, change.new_index().map(|i| i + 1))
                }
                ChangeTag::Equal => (
                    DiffLineKind::Ctx,
                    change.old_index().map(|i| i + 1),
                    change.new_index().map(|i| i + 1),
                ),
            };
            DiffLine {
                kind,
                old_number,
                new_number,
                text: change.value().trim_end_matches(['\r', '\n']).to_string(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_addition_carries_new_number_and_no_old_number() {
        let d = compute_diff("a\n", "a\nb\n");
        let added = d.iter().find(|l| l.kind == DiffLineKind::Add).unwrap();
        assert_eq!(added.text, "b");
        assert_eq!(added.new_number, Some(2));
        assert_eq!(added.old_number, None);
    }

    #[test]
    fn pure_deletion_carries_old_number_and_no_new_number() {
        let d = compute_diff("a\nb\n", "a\n");
        let del = d.iter().find(|l| l.kind == DiffLineKind::Del).unwrap();
        assert_eq!(del.text, "b");
        assert_eq!(del.old_number, Some(2));
        assert_eq!(del.new_number, None);
    }

    #[test]
    fn modification_is_a_delete_followed_by_an_insert() {
        let d = compute_diff("a\nold\n", "a\nnew\n");
        let del = d.iter().find(|l| l.kind == DiffLineKind::Del).unwrap();
        assert_eq!(del.text, "old");
        let add = d.iter().find(|l| l.kind == DiffLineKind::Add).unwrap();
        assert_eq!(add.text, "new");
    }

    #[test]
    fn identical_content_is_all_context() {
        let d = compute_diff("x\ny\n", "x\ny\n");
        assert!(d.iter().all(|l| l.kind == DiffLineKind::Ctx));
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].text, "x");
    }

    #[test]
    fn empty_inputs_yield_no_rows() {
        assert!(compute_diff("", "").is_empty());
    }
}
