# 历史记录 Diff 视图：应用更新与翻译的内容变更对比

> 备份自 Claude Code 计划文件 · 日期 2026-07-23 · 状态：已批准，实施中

## Context（为什么做）

当前 `apply_update`（覆盖主语言文件）和 `translate`（覆盖目标语言文件）都是直接用新内容覆盖旧文件，**覆盖前不留任何旧版本**。历史日志（`state.json` 的 `log` 数组）只记一条文字（如 `Applied upstream update to 'sessions'`），看不到具体改了什么。

用户点了"应用更新"后无法确认实际变更内容。本次实现：

- 覆盖前自动把旧内容存为**快照**；
- 历史界面里点击「应用更新 / 翻译」记录，弹出 **diff 视图**对比新旧内容；
- diff 视图支持**「行内红绿 / 左右并排」两种，dialog 内可切换**；
- 翻译记录同样支持（对比新旧中文翻译）。

> **重要预期**：功能上线**之前**产生的历史记录（你机器上那 2 条 applyUpdate）没有快照，点击不了；只有上线后新产生的记录才能对比。

## 设计概览

- **快照存储**：覆盖前把旧内容写到 `<dir>/sources/<source_id>/.history/<lang>/<doc_id>.<safe_ts>.md`。随源目录一起被 `remove_source` 删除（`sources.rs:212`）；不进 backup（与文档正文一致，见 `backup.rs`）。
- **日志关联**：`LogEntry` 增加可选 `snapshot: SnapshotRef{lang, file}` 字段。前端凭它判断「可否点击 + 点击后传什么」。`#[serde(default)]` 保证旧 `state.json` 向后兼容。
- **首次不存**：旧文件不存在（首次翻译 / 首次更新）时不存快照、`snapshot = None`，该记录**不可点击**。用户核对的是"这次到底改了什么"，首次本就没有旧版本可对比，对比整篇新增无意义。只有"覆盖已有内容"时才留快照。
- **diff 在后端算**：新增 `similar` crate 做行级 diff，返回结构化行（kind + 行号 + 文本）。前端零新依赖，两种视图共用同一份数据渲染。
- **双视图切换**：dialog 顶部 segmented toggle「行内 / 并排」，切换时用缓存的 diff 数据重渲染（纯前端，后端无需变动）。

## 后端改动（src-tauri/）

### 1. 依赖 — `Cargo.toml`
`[dependencies]` 增加 `similar = "2"`。

### 2. `src/state.rs`
- 新增结构 `SnapshotRef { lang: String, file: String }`（`#[serde(rename_all = "camelCase")]`）。`file` 是快照文件名，相对 `sources/<id>/.history/<lang>/`。
- `LogEntry` 增加：
  ```rust
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub snapshot: Option<SnapshotRef>,
  ```

### 3. 新模块 `src/snapshot.rs`
- `snapshot_dir(dir, source_id, lang) -> PathBuf`：`sources/<id>/.history/<lang>/`。
- `sanitize_ts_for_filename(now: &str) -> String`：RFC3339 → 文件名安全（`:`→`-`、去纳秒、`+00:00`→`Z`），如 `2026-07-23T02-48-07Z`（Windows 文件名禁用 `:`）。
- `save_snapshot(dir, source_id, lang, doc_id, now, content) -> Result<SnapshotRef>`：建目录、写文件（content 允许为空）、返回 `SnapshotRef`。文件名 `<doc_id>.<sanitized_ts>.md`。
- `read_snapshot(dir, source_id, &snap) -> Result<String>`：文件缺失返回 `Err`，供命令转成友好提示。

### 4. `src/lib.rs`
- 注册 `mod snapshot;`、`mod diff;`。
- `invoke_handler` 列表（行 56-76）加入 `commands::get_diff`。

### 5. `src/sync.rs` — `apply_update`（行 87-124）
- 把 `let now = now_iso();`（当前在行 100）**提前到写入之前**。
- 在 `std::fs::write(&dest_path, &content)`（行 98）**之前**，仅当旧文件存在才留快照：
  ```rust
  let snapshot = if dest_path.exists() {
      let old = std::fs::read_to_string(&dest_path)?;
      Some(snapshot::save_snapshot(dir, source_id, &source.primary_language, doc_id, &now, &old)?)
  } else { None };   // 首次无旧文件 -> None，不存快照、记录不可点
  ```
- 日志 `LogEntry`（行 114-121）填 `snapshot`（可能为 `None`）。

### 6. `src/translate.rs` — `translate_doc_with_provider`（行 146-208）
- 同样把 `now`（当前行 183）提前。
- 在 `std::fs::write(&dest_path, &translated)`（行 181）**之前**：同样 `if dest_path.exists()` 才留快照（首次翻译目标语言文件不存在 → `None`，记录不可点；重新翻译已有旧翻译 → 存快照、可点）。日志（行 194-205）填 `snapshot`。

### 7. 新模块 `src/diff.rs`
```rust
#[derive(Serialize)]
#[serde(rename_all = "lowercase")]           // -> "add" / "del" / "ctx"
pub enum DiffLineKind { Add, Del, Ctx }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_number: Option<usize>,
    pub new_number: Option<usize>,
    pub text: String,
}

pub fn compute_diff(old: &str, new: &str) -> Vec<DiffLine>
// 用 similar::TextDiff::from_lines + iter_changes：
// Delete -> Del(old_index) , Insert -> Add(new_index) , Equal -> Ctx(both)
```

### 8. `src/commands.rs`
```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffPayload { pub lines: Vec<DiffLine> }

#[tauri::command]
pub fn get_diff(state, source_id, lang, doc_id, snapshot_file) -> Result<DiffPayload, String>
//   read_snapshot (失败 -> "快照不在本机，可能因导入备份或清理而丢失")
// + read_doc_content (当前文件) -> compute_diff
```

## 前端改动（src/）

### `src/api.ts`
- 新增接口：`SnapshotRef`、`DiffKind = "add"|"del"|"ctx"`、`DiffLine`、`DiffPayload`。
- `LogEntry` 增加 `snapshot: SnapshotRef | null`。
- `getDiff(sourceId, lang, docId, snapshotFile)` → `invoke("get_diff", {...})`。

### `index.html`
- 在现有 dialog 之后加 `<dialog id="diff-dialog" class="remote-dialog diff-dialog">`：标题区（文档名 + 时间 + 语言）、视图切换 toggle（「行内」「并排」）、`<div id="diff-body">`、关闭按钮。

### `src/diff-dialog.ts`（新建，仿 `remote-source-dialog.ts` 模式）
- `initDiffDialog()`：绑定元素，toggle 切换时用缓存 lines 重渲染，关闭/Esc 处理。
- `openDiffDialog(entry, docTitle?)`：`showModal()` → loading → `getDiff(entry.sourceId, entry.snapshot.lang, entry.docId, entry.snapshot.file)` → 缓存 lines → 按当前视图渲染；网络/快照缺失走错误态友好提示。
- `renderInline(lines)`：绿/红/灰行 + 行号列（类 GitHub unified diff）。
- `renderSideBySide(lines)`：左右两栏对齐表格（旧 | 新）。

### `src/history-view.ts`
- `renderHistory(container, entries, onShowDiff?)`：有 `entry.snapshot` 的行加 `clickable` 样式 + `title="点击查看变更对比"` + 点击调 `onShowDiff(entry)`；其余（import/check、旧无快照记录）不可点。

### `src/main.ts`
- import 并在启动时 `initDiffDialog()`。
- `showHistory()` 改为 `renderHistory(contentEl, entries, (e) => openDiffDialog(e))`。

### `src/styles.css`
- `.history-row.clickable` hover 态；`.diff-dialog`（更宽，约 90vw）；`.diff-toggle` segmented control；`.diff-line` / `.diff-line-add`（绿底）/ `.diff-line-del`（红底）/ 行号列；`.diff-side` 两栏表格；loading/error 态。

## 边界与一致性
- **删源**：`remove_source` 删整个 `sources/<id>`（含 `.history`），且 retain 删除该源日志（`sources.rs:219`）→ 无悬空引用。
- **导入备份**：backup 不含快照与正文（`backup.rs`）。新机上点旧记录 → `get_diff` 返回友好错误，dialog 提示。
- **文件名碰撞**：时间戳取秒级，同秒内对同一文档两次操作会覆盖快照——手动场景极罕见，可接受（未来可加纳秒/序号）。
- **旧历史记录**：snapshot 为 null，不可点击。

## 测试（后端，`cargo test`，遵循 testing.md）
- `snapshot.rs`：sanitize 产出文件名安全字符；save→read round-trip；read 缺失报错。
- `sync.rs`：二次 apply 后 `.history/<lang>/<doc>.<ts>.md` 存在且内容=旧版本、`LogEntry.snapshot` 非空；首次 apply（本地无旧文件）→ `snapshot=None`、无快照文件。
- `translate.rs`：重新翻译（已有旧翻译）→ 目标语言快照存在、`snapshot` 非空；首次翻译 → `snapshot=None`、无快照。
- `diff.rs`：增/删/改/混合场景的行类型与行号正确。
- 现有测试保持绿色（`LogEntry` 新字段带 default）。

前端无测试框架（沿用现状），靠手动验证。

## 端到端验证
1. `cd src-tauri && cargo test` 全绿。
2. `pnpm tauri dev` 启动应用。
3. 选 `pi` 源 → 检查更新 → 对一篇 changed 文档「应用更新」→「历史」→ 点击该 applyUpdate 记录 → 弹 diff；切换「行内 / 并排」正常显示红绿/左右对照。
4. 对一篇**已有翻译**的文档重新翻译（如 pi 源里因应用更新而变 `pending` 的文档）→ 历史里点该 translate 记录 → diff 对比新旧中文翻译。（首次翻译的记录不可点。）
5. import / check 记录、以及功能上线前的旧记录 → 不可点击。
