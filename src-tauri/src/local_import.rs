//! Importing a local folder of markdown files as a new documentation
//! source: detects a bilingual language-subfolder layout when present,
//! derives a nav order (a manifest file if one exists, otherwise the
//! filesystem structure with natural sort), and copies every markdown file
//! into this source's private data directory.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::hashing::sha256_hex;
use crate::nav::{natural_cmp, parse_link_manifest, prettify_id, NavCategory, NavItem, NavTree};
use crate::sources::{doc_content_path, write_manifest};
use crate::state::{
    now_iso, AppStateData, CheckStatus, DocMeta, LogEntry, LogKind, Source, SourceKind,
    TranslationStatus,
};

/// Subdirectory names recognized as language folders when a source has two
/// or more of them at its top level, each with markdown content inside.
const KNOWN_LANG_CODES: &[&str] = &[
    "en", "zh", "ja", "ko", "fr", "de", "es", "pt", "ru", "it", "ar", "hi",
];

/// Checked in this priority order so `SUMMARY.md` wins over a plain `README.md`
/// when a folder happens to have both.
const MANIFEST_FILE_NAMES: &[&str] = &["summary.md", "_sidebar.md", "readme.md"];

/// The language key used when a folder has no recognized language
/// subdirectories — it's imported as a single, unnamed-language source.
const SINGLE_LANG: &str = "default";

/// Import `folder` as a new source: detect its language layout, derive a
/// nav order, copy every markdown file into this app's data directory, and
/// register the resulting `Source` + `DocMeta` entries in `data`. Does not
/// persist `data` to disk itself — the caller does that once the state lock
/// is released.
pub fn import_local_folder(
    dir: &Path,
    data: &mut AppStateData,
    folder: &Path,
    display_name: Option<String>,
) -> Result<Source> {
    if !folder.is_dir() {
        bail!("'{}' 不是一个文件夹", folder.display());
    }

    let languages = detect_languages(folder);
    let primary = pick_primary(&languages);
    let primary_root = lang_root(folder, &languages, &primary);
    if !primary_root.is_dir() {
        bail!("未找到语言目录 '{}'", primary_root.display());
    }

    let name = display_name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| folder_display_name(folder));
    let source_id = unique_source_id(data, &slugify(&name));

    let primary_manifest = find_manifest_file(&primary_root);
    let mut categories = primary_manifest
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|content| parse_link_manifest(&content))
        .unwrap_or_default();
    if categories.is_empty() {
        categories = build_fallback_categories(&primary_root, primary_manifest.as_deref());
    }
    if categories.is_empty() {
        bail!("'{}' 下没有找到任何 Markdown 文档", folder.display());
    }

    // Copy every language's tree onto disk, tracking which doc ids actually
    // exist per language (a mirrored structure across languages isn't
    // guaranteed) and hashing the primary language's content as we go.
    let mut docs_by_lang: HashMap<String, HashSet<String>> = HashMap::new();
    let mut primary_hashes: HashMap<String, String> = HashMap::new();
    for lang in &languages {
        let root = lang_root(folder, &languages, lang);
        if !root.is_dir() {
            continue; // secondary language folder missing entirely; skip gracefully
        }
        let manifest = find_manifest_file(&root);
        let dest_root = dir.join("sources").join(&source_id).join("docs").join(lang);
        let mut present = HashSet::new();

        for (rel_id, abs_path) in collect_md_files(&root) {
            if manifest.as_deref() == Some(abs_path.as_path()) {
                continue; // the manifest file itself isn't a browsable doc
            }
            let content = std::fs::read_to_string(&abs_path)
                .with_context(|| format!("reading {abs_path:?}"))?;
            let dest_path = dest_root.join(format!("{rel_id}.md"));
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&dest_path, &content).with_context(|| format!("writing {dest_path:?}"))?;

            if lang == &primary {
                primary_hashes.insert(rel_id.clone(), sha256_hex(&content));
            }
            present.insert(rel_id);
        }
        docs_by_lang.insert(lang.clone(), present);
    }

    copy_static_assets(dir, &source_id, folder, &languages);

    let now = now_iso();
    let primary_present = docs_by_lang.get(&primary).cloned().unwrap_or_default();
    let mentioned: HashSet<&str> = categories
        .iter()
        .flat_map(|c| c.items.iter().map(|i| i.doc_id.as_str()))
        .collect();

    let mut doc_metas = Vec::new();
    for cat in &categories {
        for item in &cat.items {
            let Some(hash) = primary_hashes.get(&item.doc_id) else {
                continue; // manifest mentions a doc missing from the primary language
            };
            doc_metas.push(make_local_doc_meta(
                &source_id, &item.doc_id, item.title.clone(), cat.name.clone(),
                hash.clone(), &languages, &primary, &docs_by_lang, &now,
            ));
        }
    }
    // Anything the manifest/fallback pass didn't mention but the primary
    // language does have on disk still gets a spot, so an incomplete
    // manifest can't silently drop real files.
    for rel_id in &primary_present {
        if mentioned.contains(rel_id.as_str()) {
            continue;
        }
        let Some(hash) = primary_hashes.get(rel_id) else { continue };
        let title = derive_title(&doc_content_path(dir, &source_id, &primary, rel_id), rel_id);
        doc_metas.push(make_local_doc_meta(
            &source_id, rel_id, title, "未分类".to_string(),
            hash.clone(), &languages, &primary, &docs_by_lang, &now,
        ));
    }

    write_manifest(dir, &source_id, &NavTree { source_id: source_id.clone(), categories })?;

    let source = Source {
        id: source_id.clone(),
        name: name.clone(),
        kind: SourceKind::LocalFolder,
        languages: languages.clone(),
        primary_language: primary,
        remote: None,
        local_path: Some(folder.to_string_lossy().to_string()),
        order_override: Vec::new(),
        created_at: now.clone(),
        updated_at: now.clone(),
    };

    let doc_count = doc_metas.len();
    data.sources.push(source.clone());
    data.docs.extend(doc_metas);
    data.log.push(LogEntry {
        id: format!("import-{source_id}-{now}"),
        ts: now.clone(),
        source_id: source_id.clone(),
        doc_id: None,
        kind: LogKind::Import,
        detail: format!("Imported local folder '{name}' from {} ({doc_count} docs)", folder.display()),
        snapshot: None,
    });

    Ok(source)
}

fn make_local_doc_meta(
    source_id: &str,
    doc_id: &str,
    title: String,
    category: String,
    hash: String,
    languages: &[String],
    primary: &str,
    docs_by_lang: &HashMap<String, HashSet<String>>,
    now: &str,
) -> DocMeta {
    let all_secondary_present = languages
        .iter()
        .filter(|l| l.as_str() != primary)
        .all(|l| docs_by_lang.get(l).is_some_and(|set| set.contains(doc_id)));

    let translation_status = if languages.len() < 2 {
        TranslationStatus::NotApplicable
    } else if all_secondary_present {
        TranslationStatus::Translated
    } else {
        TranslationStatus::NeverTranslated
    };

    DocMeta {
        source_id: source_id.to_string(),
        id: doc_id.to_string(),
        title,
        category,
        primary_hash: hash,
        last_checked_at: None,
        last_check_status: CheckStatus::Unknown,
        translated_at: (translation_status == TranslationStatus::Translated).then(|| now.to_string()),
        translation_status,
        translated_by: None,
    }
}

/// First `# Heading` line in the file, or a prettified last path segment of
/// `fallback_id` if the file has none / can't be read.
fn derive_title(path: &Path, fallback_id: &str) -> String {
    if let Ok(content) = std::fs::read_to_string(path) {
        for line in content.lines() {
            if let Some(rest) = line.trim_start().strip_prefix("# ") {
                return rest.trim().to_string();
            }
        }
    }
    let last_segment = fallback_id.rsplit('/').next().unwrap_or(fallback_id);
    prettify_id(last_segment)
}

/// Files directly under `root` become a "未分类" category; each immediate
/// subdirectory becomes its own category (named after the directory) with
/// every markdown file beneath it at any depth. Items and category names
/// are natural-sorted.
fn build_fallback_categories(root: &Path, exclude: Option<&Path>) -> Vec<NavCategory> {
    let mut root_items = Vec::new();
    let mut grouped: HashMap<String, Vec<NavItem>> = HashMap::new();

    for (rel_id, abs_path) in collect_md_files(root) {
        if exclude.is_some_and(|e| e == abs_path) {
            continue;
        }
        let title = derive_title(&abs_path, &rel_id);
        match rel_id.split_once('/') {
            None => root_items.push(NavItem { doc_id: rel_id, title }),
            Some((top_dir, _)) => {
                grouped.entry(top_dir.to_string()).or_default().push(NavItem { doc_id: rel_id, title })
            }
        }
    }

    let mut categories = Vec::new();
    if !root_items.is_empty() {
        root_items.sort_by(|a, b| natural_cmp(&a.doc_id, &b.doc_id));
        categories.push(NavCategory { name: "未分类".to_string(), items: root_items });
    }

    let mut dir_names: Vec<String> = grouped.keys().cloned().collect();
    dir_names.sort_by(|a, b| natural_cmp(a, b));
    for dir_name in dir_names {
        let mut items = grouped.remove(&dir_name).unwrap_or_default();
        items.sort_by(|a, b| natural_cmp(&a.doc_id, &b.doc_id));
        categories.push(NavCategory { name: dir_name, items });
    }
    categories
}

/// Recursively collect every `.md` file under `root`, returning
/// `(forward-slash relative id without extension, absolute path)` pairs.
/// Hidden entries (dotfiles) and `node_modules` are skipped.
fn collect_md_files(root: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    walk_for_md(root, root, &mut out);
    out
}

fn walk_for_md(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        eprintln!("local import: cannot read directory {dir:?}");
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if should_skip_component(&name) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            walk_for_md(root, &path, out);
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("md")) {
            if let Some(rel_id) = relative_doc_id(root, &path) {
                out.push((rel_id, path));
            }
        }
    }
}

fn should_skip_component(name: &str) -> bool {
    name.starts_with('.') || name == "node_modules"
}

fn relative_doc_id(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let no_ext = rel.with_extension("");
    let parts: Vec<String> =
        no_ext.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

/// Find a manifest file directly under `root`, preferring `SUMMARY.md` over
/// `_sidebar.md` over `README.md` when more than one exists (case-insensitive).
fn find_manifest_file(root: &Path) -> Option<PathBuf> {
    let entries: Vec<_> = std::fs::read_dir(root).ok()?.filter_map(|e| e.ok()).collect();
    for wanted in MANIFEST_FILE_NAMES {
        for entry in &entries {
            if !entry.path().is_file() {
                continue;
            }
            if entry.file_name().to_string_lossy().to_lowercase() == *wanted {
                return Some(entry.path());
            }
        }
    }
    None
}

/// A folder is bilingual/multilingual when at least two of its top-level
/// subdirectories match a known language code and actually contain markdown
/// files; otherwise everything is imported under a single `"default"` bucket.
/// Copy top-level entries that are neither language folders nor markdown
/// docs (e.g. an `assets/` image directory) into `sources/<id>/`, so images
/// referenced as `/assets/...` in the markdown remain available after the
/// original folder is moved or deleted.
fn copy_static_assets(
    dir: &Path,
    source_id: &str,
    folder: &Path,
    languages: &[String],
) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if languages.iter().any(|l| l == &name)
            || name.starts_with('.')
            || entry.path().extension().is_some_and(|e| e.eq_ignore_ascii_case("md"))
        {
            continue;
        }
        let dest = dir.join("sources").join(source_id).join(&name);
        let _ = copy_dir_recursive(&entry.path(), &dest);
    }
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> std::io::Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dest)?;
        for entry in std::fs::read_dir(src)?.flatten() {
            copy_dir_recursive(&entry.path(), &dest.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(src, dest).map(|_| ())
    }
}

fn detect_languages(folder: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return vec![SINGLE_LANG.to_string()];
    };

    let mut matched: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            KNOWN_LANG_CODES.contains(&name.as_str()).then_some(name)
        })
        .filter(|lang| has_any_md(&folder.join(lang)))
        .collect();
    matched.sort();
    matched.dedup();

    if matched.len() >= 2 {
        matched
    } else {
        vec![SINGLE_LANG.to_string()]
    }
}

fn has_any_md(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else { return false };
    for entry in entries.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        if should_skip_component(&name) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            if has_any_md(&path) {
                return true;
            }
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("md")) {
            return true;
        }
    }
    false
}

fn pick_primary(languages: &[String]) -> String {
    if languages.iter().any(|l| l == "en") {
        "en".to_string()
    } else {
        languages[0].clone()
    }
}

/// Also used by `sync` to locate a LocalFolder source's upstream copy when
/// checking for changes.
pub(crate) fn lang_root(folder: &Path, languages: &[String], lang: &str) -> PathBuf {
    if languages.len() == 1 && languages[0] == SINGLE_LANG {
        folder.to_path_buf()
    } else {
        folder.join(lang)
    }
}

fn folder_display_name(folder: &Path) -> String {
    folder
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "Imported Docs".to_string())
}

/// ASCII-only, filesystem-safe slug (used as the source id / directory
/// name) — non-ASCII display names still work fine since `Source.name`
/// keeps the original text; the id just needs to be unique and portable.
/// Shared with `remote_import` so every source kind gets the same id scheme.
pub(crate) fn slugify(input: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for ch in input.trim().to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "source".to_string()
    } else {
        slug
    }
}

pub(crate) fn unique_source_id(data: &AppStateData, base: &str) -> String {
    if !data.sources.iter().any(|s| s.id == base) {
        return base.to_string();
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if !data.sources.iter().any(|s| s.id == candidate) {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[test]
    fn imports_single_language_folder_with_manifest() {
        let app_dir = tempfile::tempdir().unwrap();
        let src = tempfile::tempdir().unwrap();

        write(
            &src.path().join("README.md"),
            "## 分类一\n\n- [intro.md](intro.md) - 简介\n- [setup.md](setup.md) - 安装\n",
        );
        write(&src.path().join("intro.md"), "# Intro\n\nHello world.");
        write(&src.path().join("setup.md"), "# Setup\n\nInstall steps.");

        let mut data = AppStateData::default();
        let source =
            import_local_folder(app_dir.path(), &mut data, src.path(), Some("我的笔记".to_string()))
                .unwrap();

        assert_eq!(source.languages, vec![SINGLE_LANG.to_string()]);
        assert_eq!(source.kind, SourceKind::LocalFolder);
        assert_eq!(data.docs.len(), 2);
        assert!(app_dir
            .path()
            .join("sources")
            .join(&source.id)
            .join("docs/default/intro.md")
            .exists());
        // The manifest file itself must not be copied in as a browsable doc.
        assert!(!app_dir
            .path()
            .join("sources")
            .join(&source.id)
            .join("docs/default/README.md")
            .exists());

        let intro = data.docs.iter().find(|d| d.id == "intro").unwrap();
        assert_eq!(intro.title, "简介");
        assert_eq!(intro.category, "分类一");
        assert_eq!(intro.translation_status, TranslationStatus::NotApplicable);
    }

    #[test]
    fn copies_top_level_asset_dirs_alongside_docs() {
        let app_dir = tempfile::tempdir().unwrap();
        let src = tempfile::tempdir().unwrap();

        write(&src.path().join("en/intro.md"), "# Intro\n\n![img](/assets/pic.png)");
        write(&src.path().join("zh/intro.md"), "# 简介\n\n![图](/assets/pic.png)");
        write(&src.path().join("assets/pic.png"), "fake png bytes");
        write(&src.path().join("assets/nested/deep.jpg"), "fake jpg bytes");

        let mut data = AppStateData::default();
        let source =
            import_local_folder(app_dir.path(), &mut data, src.path(), Some("带图文档".to_string()))
                .unwrap();

        let base = app_dir.path().join("sources").join(&source.id);
        assert_eq!(
            fs::read_to_string(base.join("assets/pic.png")).unwrap(),
            "fake png bytes"
        );
        assert_eq!(
            fs::read_to_string(base.join("assets/nested/deep.jpg")).unwrap(),
            "fake jpg bytes"
        );
    }

    #[test]
    fn detects_bilingual_layout_and_flags_missing_translation() {
        let app_dir = tempfile::tempdir().unwrap();
        let src = tempfile::tempdir().unwrap();

        write(&src.path().join("en/intro.md"), "# Intro\n\nHello.");
        write(&src.path().join("en/extra.md"), "# Extra\n\nOnly in English.");
        write(&src.path().join("zh/intro.md"), "# 简介\n\n你好。");
        // Note: no zh/extra.md — this doc has no translation yet.

        let mut data = AppStateData::default();
        let source =
            import_local_folder(app_dir.path(), &mut data, src.path(), Some("Bilingual Wiki".to_string()))
                .unwrap();

        assert_eq!(source.languages, vec!["en".to_string(), "zh".to_string()]);
        assert_eq!(source.primary_language, "en");

        let intro = data.docs.iter().find(|d| d.id == "intro").unwrap();
        assert_eq!(intro.translation_status, TranslationStatus::Translated);
        let extra = data.docs.iter().find(|d| d.id == "extra").unwrap();
        assert_eq!(extra.translation_status, TranslationStatus::NeverTranslated);
    }

    #[test]
    fn falls_back_to_filesystem_structure_when_no_manifest() {
        let app_dir = tempfile::tempdir().unwrap();
        let src = tempfile::tempdir().unwrap();

        write(&src.path().join("intro.md"), "# Intro\n\nRoot-level doc.");
        write(&src.path().join("guides/2-advanced.md"), "# Advanced\n\nDetails.");
        write(&src.path().join("guides/10-appendix.md"), "# Appendix\n\nMore.");
        write(&src.path().join("guides/1-basics.md"), "# Basics\n\nStart here.");

        let mut data = AppStateData::default();
        let source = import_local_folder(app_dir.path(), &mut data, src.path(), None).unwrap();

        let tree = crate::sources::read_manifest(app_dir.path(), &source.id).unwrap();
        assert_eq!(tree.categories[0].name, "未分类");
        assert_eq!(tree.categories[0].items[0].doc_id, "intro");
        assert_eq!(tree.categories[1].name, "guides");
        // Natural sort: 1-basics < 2-advanced < 10-appendix, not lexical order.
        assert_eq!(
            tree.categories[1].items.iter().map(|i| i.doc_id.as_str()).collect::<Vec<_>>(),
            vec!["guides/1-basics", "guides/2-advanced", "guides/10-appendix"],
        );
    }

    #[test]
    fn appends_numeric_suffix_on_source_id_collision() {
        let app_dir = tempfile::tempdir().unwrap();
        let src_a = tempfile::tempdir().unwrap();
        let src_b = tempfile::tempdir().unwrap();
        write(&src_a.path().join("a.md"), "# A");
        write(&src_b.path().join("b.md"), "# B");

        let mut data = AppStateData::default();
        let first =
            import_local_folder(app_dir.path(), &mut data, src_a.path(), Some("Notes".to_string()))
                .unwrap();
        let second =
            import_local_folder(app_dir.path(), &mut data, src_b.path(), Some("Notes".to_string()))
                .unwrap();

        assert_eq!(first.id, "notes");
        assert_eq!(second.id, "notes-2");
    }

    #[test]
    fn rejects_a_path_that_is_not_a_directory() {
        let app_dir = tempfile::tempdir().unwrap();
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut data = AppStateData::default();

        let err = import_local_folder(app_dir.path(), &mut data, file.path(), None).unwrap_err();
        assert!(err.to_string().contains("不是一个文件夹"));
    }

    #[test]
    fn rejects_a_folder_with_no_markdown_files() {
        let app_dir = tempfile::tempdir().unwrap();
        let src = tempfile::tempdir().unwrap();
        write(&src.path().join("notes.txt"), "not markdown");
        let mut data = AppStateData::default();

        let err = import_local_folder(app_dir.path(), &mut data, src.path(), None).unwrap_err();
        assert!(err.to_string().contains("没有找到任何 Markdown 文档"));
    }
}
