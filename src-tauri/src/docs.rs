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

/// Read a static asset (image, etc.) copied alongside a source's docs and
/// return it with a MIME type guessed from its extension, ready to be
/// base64-encoded into a data URL by the command layer.
///
/// `rel_path` is the URL path used in the markdown, e.g. `assets/pic.png`
/// (leading `/` stripped). Paths escaping the source's directory (`..` or
/// absolute components) are rejected so a crafted doc can't read arbitrary
/// files from the app data directory or the host.
pub fn read_asset(dir: &Path, source_id: &str, rel_path: &str) -> Result<(String, Vec<u8>)> {
    let rel = rel_path.trim_start_matches('/');
    if rel.split(['/', '\\']).any(|seg| seg == ".." || seg.is_empty()) {
        anyhow::bail!("非法的静态资源路径: {rel_path:?}");
    }
    let path = dir.join("sources").join(source_id).join(rel);
    let bytes = std::fs::read(&path)
        .with_context(|| format!("reading asset at {path:?}"))?;
    let mime = mime_for(path.extension().and_then(|e| e.to_str()).unwrap_or(""));
    Ok((mime, bytes))
}

fn mime_for(ext: &str) -> String {
    match ext.to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_asset_with_png_mime() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sources/shape-up/assets/pic.PNG");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, b"png bytes").unwrap();

        let (mime, bytes) = read_asset(dir.path(), "shape-up", "/assets/pic.PNG").unwrap();
        assert_eq!(mime, "image/png");
        assert_eq!(bytes, b"png bytes");
    }

    #[test]
    fn rejects_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_asset(dir.path(), "s", "assets/../../state.json").is_err());
        assert!(read_asset(dir.path(), "s", "/etc/passwd").is_err());
    }
}
