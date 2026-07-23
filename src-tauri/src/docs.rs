//! Reading document content from disk. Content is never cached into the
//! frontend bundle — every request re-reads the current file so translation
//! updates and manual edits show up immediately.

use std::path::Path;

use anyhow::{Context, Result};

use crate::sources::doc_content_path;

pub fn read_doc_content(dir: &Path, source_id: &str, lang: &str, id: &str) -> Result<String> {
    let path = doc_content_path(dir, source_id, lang, id);
    std::fs::read_to_string(&path)
        .with_context(|| format!("reading doc content at {path:?}"))
}
