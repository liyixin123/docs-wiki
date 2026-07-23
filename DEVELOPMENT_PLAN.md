# Docs Wiki 桌面应用 — 开发计划

本文档描述把「Pi 文档中文站」升级为**通用多来源文档 Wiki 桌面应用**（Tauri）的开发计划。相比最初的版本（只服务 Pi 一套文档），本次修订后的目标是：

1. 做成 Tauri 桌面应用（不是浏览器打开的静态站，也不是单独起一个 Node/Python 服务）。
2. **支持同时管理多个文档来源**（不止 Pi）：本地文件夹导入、远程 Git/URL 抓取、内置 Pi 种子三种方式都要支持。
3. **wiki 式关键字全文搜索**（正文检索 + 高亮摘要 + 跳转），不再局限于标题匹配。
4. 中英文档必须是磁盘上可定位的真实 `.md` 源文件，启动时动态读取，不打包进前端 JS。
5. **每个来源可选双语**——翻译/更新检测只对配置了「源语言→目标语言」的来源开启，单语来源（如导入的纯英文 wiki）不参与。
6. 大模型 Provider 做成可插拔适配层，同时支持 Anthropic 和 OpenAI 兼容接口，配置文件驱动。

> 完整的架构决策讨论见 `/Users/liyixin/.claude/plans/mellow-skipping-cascade.md`（本次修订的原始计划文件）。

## 0. 已就绪的种子数据

`seed-docs/en/` 和 `seed-docs/zh/`（与本文件同级）放着 Pi 文档的 29 篇源文件，作为**内置的 Seed 来源**在应用首次启动时导入。`seed-docs/zh/README.md` 是分类导航清单（6 个分类 + 有序链接列表），被直接解析为 Pi 来源的 `manifest.json`，保留原有顺序。

```
pi-docs-desktop/
├── DEVELOPMENT_PLAN.md   ← 本文件
└── seed-docs/
    ├── en/*.md            ← 29 篇英文原文
    └── zh/*.md            ← 29 篇中文翻译 + README.md（分类索引，即 manifest 来源）
```

## 1. 总体架构

```
┌─────────────────────────────┐
│         WebView 前端          │  index.html / main.ts / *-view.ts
│  （来源选择 + 侧边栏 + 内容）  │
└───────────────┬──────────────┘
                 │ tauri invoke()
┌───────────────▼──────────────┐
│        Rust 后端（src-tauri）  │
│  commands / sources / nav /   │
│  search / translate / state   │
└───────────────┬──────────────┘
                 │
     ┌───────────┼────────────────┐
     ▼           ▼                ▼
 磁盘 md 文件   state.json      大模型 API
(sources/<id>) (来源+文档+日志) (Anthropic / OpenAI 兼容)
```

选择 Tauri 而不是"本地起 HTTP 服务"的原因不变：Rust 后端天然有文件读写/网络能力，API Key 只存在于 Rust 进程里，WebView 侧的 JS 永远拿不到明文 key。

## 2. 目录结构

```
pi-docs-desktop/
├── src/                        # 前端（vanilla TypeScript + Vite）
│   ├── index.html
│   ├── main.ts                  # 入口：来源选择、导入按钮、语言切换、导航、搜索、内容面板的拼装
│   ├── api.ts                   # invoke() 的类型化封装
│   ├── nav-view.ts               # 侧边栏渲染
│   ├── content-view.ts           # 内容面板渲染（loading/error/markdown）
│   ├── search-view.ts            # wiki 式搜索结果页渲染
│   ├── markdown.ts                # marked 封装（本地打包，离线可渲染）
│   ├── lang.ts                     # 语言标签小工具
│   └── styles.css
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/default.json  # 含 dialog:default（本地文件夹选择器权限）
│   ├── icons/
│   └── src/
│       ├── main.rs / lib.rs
│       ├── commands.rs         # #[tauri::command] 入口，前端唯一可调用的接口面
│       ├── state.rs            # Source / DocMeta / LogEntry 数据模型 + state.json 原子写入
│       ├── sources.rs          # seed 导入 + 跨来源共用的 manifest/doc-content 路径helper
│       ├── local_import.rs     # 本地文件夹导入：语言探测、manifest/文件系统兜底排序、拷贝落盘
│       ├── remote_import.rs    # 远程 Git 导入：Git Trees API 列文件 + raw 内容下载
│       ├── sync.rs             # check_updates/apply_update：按来源类型（远程/本地）核对并拉取更新
│       ├── nav.rs              # 分层排序解析：manifest 链接列表 / 手动 override / 自然排序 / prettify_id
│       ├── docs.rs             # 按来源+语言读文件
│       ├── search.rs           # 内存全文索引 + 查询
│       ├── hashing.rs          # sha256 封装
│       ├── config.rs           # config.json 读写、key 解析（$ENV_VAR/!command/字面量）、脱敏
│       ├── provider.rs         # TranslationProvider trait + Anthropic/OpenAI 兼容实现
│       ├── translate.rs        # 分块、代码块占位保护、结构自查、translate_doc/translate_all_pending 的核心逻辑
│       └── backup.rs           # export_backup/import_backup：state.json + config.json 打包/还原
├── seed-docs/                  # 编译期通过 include_dir! 嵌入二进制，首次启动时展开
└── DEVELOPMENT_PLAN.md
```

运行时的用户数据目录（Tauri `app_data_dir()`，macOS 上是 `~/Library/Application Support/com.liyixin.docswiki/`）：

```
sources/<source_id>/
  docs/<lang>/<id>.md
  manifest.json        # 该来源的导航树（分类 + 有序文档列表）
state.json              # { sources: Source[], docs: DocMeta[], log: LogEntry[] }
config.json             # Provider / Key / 翻译行为配置（P5）
```

首次启动时：`sources.rs::ensure_seed_source` 检测 `state.json` 里是否已有 `pi` 来源；没有就把编译期嵌入的 `seed-docs/{en,zh}` 内容写入 `sources/pi/docs/{en,zh}`，解析 `zh/README.md` 生成 `manifest.json`，为每篇英文原文计算 sha256 写入 `DocMeta.primaryHash`，写 `state.json` 基线（`translationStatus = Translated`，因为种子内容本来就是配套翻译好的）。**此逻辑已实现并有单元测试覆盖**（见 `src-tauri/src/sources.rs` 的 `#[cfg(test)]` 模块）。

## 3. 数据模型（已实现，见 `state.rs`）

```rust
enum SourceKind { Seed, LocalFolder, RemoteGit }

struct RemoteSpec { owner: String, repo: String, branch: String, path: String }

struct Source {
    id: String,
    name: String,
    kind: SourceKind,
    languages: Vec<String>,       // ["en","zh"] 或 ["default"]（单语来源，"default" 表示无法/无需识别具体语种）
    primary_language: String,
    remote: Option<RemoteSpec>,
    local_path: Option<String>,   // LocalFolder 来源的原始导入路径，为 P4 重新扫描预留
    order_override: Vec<String>,  // 用户手动拖动排序，优先级最高
    created_at: String,
    updated_at: String,
}

enum CheckStatus { Same, Changed, Error, Unknown }
enum TranslationStatus { Translated, Pending, NeverTranslated, NotApplicable } // 单语来源为 NotApplicable

struct DocMeta {
    source_id: String,
    id: String,
    title: String,
    category: String,
    primary_hash: String,          // sha256(主语言内容)，用于检测原文变化
    last_checked_at: Option<String>,
    last_check_status: CheckStatus,
    translation_status: TranslationStatus,
    translated_at: Option<String>,
    translated_by: Option<String>,
}

enum LogKind { Import, Check, ApplyUpdate, Translate, TranslateError }
struct LogEntry { id: String, ts: String, source_id: String, doc_id: Option<String>, kind: LogKind, detail: String }

struct AppStateData { sources: Vec<Source>, docs: Vec<DocMeta>, log: Vec<LogEntry> }
```

`state.json` 写入统一走"写临时文件 + rename"的原子写模式（`state::save_state_atomic`），避免中途崩溃损坏文件。

## 4. 文档顺序 / 分类 — 分层解析（已实现核心，见 `nav.rs`）

每个来源的 `manifest.json`（分类 → 有序文档列表）按优先级分层生成：

1. **用户手动顺序**（`Source.order_override`）— 若存在，最高优先，覆盖一切。已实现：`nav::apply_order_override`。（前端拖动重排 UI 仍待做。）
2. **显式 manifest 文件** — 来源根目录的 `README.md`/`SUMMARY.md`/`_sidebar.md`（大小写不敏感，按此优先级）等链接列表文件（`## 分类` + `- [text](id.md) - 描述`），已实现：`nav::parse_link_manifest`。Pi 种子的 `zh/README.md`、本地导入的 `SUMMARY.md` 都走这条路径。
3. **front-matter**（待做）— 文档头部 `order:`/`category:` 字段。
4. **文件系统结构兜底**（已实现，见 `local_import.rs::build_fallback_categories` + `nav::natural_cmp`）— 根目录下的文件归入"未分类"；每个一级子目录（含其下任意深度的文件）各自成为一个分类；分类名与文档 id 均按数字前缀感知的自然排序（`1-intro` < `2-setup` < `10-appendix`，而非字典序）。本地文件夹导入没有 manifest 文件时走这条路径。

前端（P3）将支持拖动重排，写入 `order_override`。

## 5. 本地文件夹导入（P3，已实现，见 `local_import.rs`）

`add_local_source(path, name?)` 命令，前端通过 `@tauri-apps/plugin-dialog` 的原生文件夹选择器调用（点击顶部"+ 导入文件夹"按钮）：

- **语言探测**：扫描根目录的一级子目录，若其中 ≥2 个匹配已知语言代码（`en`/`zh`/`ja`/`ko`/`fr`/`de`/`es`/`pt`/`ru`/`it`/`ar`/`hi`）且各自内部（任意深度）确实含有 `.md` 文件，判定为多语言来源，`languages` = 匹配到的代码；否则整个文件夹当作单语言来源导入，`languages = ["default"]`（不冒充某个具体语种）。
- **导航顺序**：优先用主语言目录下的 `SUMMARY.md`/`_sidebar.md`/`README.md`（按此优先级，大小写不敏感）解析出分类和顺序（复用 `nav::parse_link_manifest`）；找不到就用文件系统兜底（`build_fallback_categories`：根目录文件 → "未分类"，一级子目录 → 各自一个分类，自然排序）。
- **文档 id**：用相对路径（正斜杠分隔、去掉 `.md`）而非裸文件名，如 `guides/setup`，天然避免同名文件跨目录冲突；`docs/<lang>/` 下按同样的相对路径落盘，无需扁平化。
- **翻译状态推断**：多语言来源里，一篇文档在除主语言外的所有语言目录下都存在对应文件 → `Translated`；缺任意一个 → `NeverTranslated`（为 P6 的"翻译此文档"入口做好准备）；单语言来源一律 `NotApplicable`。
- **来源 id 去重**：来源名转小写 ASCII slug（非 ASCII 字符会被丢弃，folder 显示名不受影响，只影响内部目录名），与现有来源 id 冲突时自动加 `-2`/`-3` 后缀。
- **单元测试**（`local_import.rs` 内 6 个）：单语言 + manifest 导入、双语言探测 + 缺失翻译标记、无 manifest 时的文件系统兜底排序（含自然排序验证）、来源 id 冲突去重、非目录路径报错、无 Markdown 文件报错。
- 前端：[index.html](index.html) 顶部"+ 导入文件夹"按钮 → `main.ts::addLocalFolderSource` 调用系统原生文件夹选择器 → `api.ts::addLocalSource` → 成功后刷新来源列表并自动切换到新来源。

尚未实现（后续跟进）：front-matter 排序策略。"重新扫描本地变更"已在 P4 实现（见第 7 节 `sync.rs`，复用 `Source.local_path`）；`remove_source` 与拖动重排 UI 见下方 5.6 节——均已完成。

## 5.5 远程 Git 来源导入（P3，已实现，见 `remote_import.rs`）

`add_remote_source(owner, repo, branch, path, name?, lang?)` 命令，前端"+ 远程仓库"按钮打开一个原生 `<dialog>` 表单（[remote-source-dialog.ts](src/remote-source-dialog.ts)）填写 owner/repo/branch/路径：

- **文件列表**：用 GitHub Git Trees API 的 `?recursive=1` 一次性拉取整个仓库的文件树（而不是逐目录调用 Contents API），过滤出 `path` 前缀下的 `.md` 文件——**已用真实的 `earendil-works/pi` 仓库端到端验证**（`remote_import.rs` 的 `imports_the_real_pi_docs_repo_end_to_end` 测试，实际发起网络请求，非 mock）。
- **导航顺序**：同样优先找 `path` 根目录下的 `SUMMARY.md`/`_sidebar.md`/`README.md` 并复用 `parse_link_manifest`；找不到就用与本地导入相同的分类/自然排序兜底规则（`build_fallback_categories_from_ids`）。**已验证**：Pi 仓库实际用的是 Mintlify 风格的 `docs.json`（非我们识别的清单格式），所以走的是兜底路径，29 篇文档全部正确落盘并归入"未分类"一个分类，按文件名自然排序。
- **单语言起点**：新导入的远程来源永远从单语言开始（`languages = [lang]`，默认 `"en"`）——上游仓库本身没有"配套翻译"这回事，第二语言要等 P6 的翻译流程产出后才会出现。
- `DocMeta.lastCheckedAt`/`lastCheckStatus` 在导入时就设为"刚检查过、与原文一致"，为 P4 的 `check_updates` 提供基线。
- **单元测试**（`remote_import.rs` 内 5 个，含 1 个真实网络测试）：manifest 优先级选择、忽略嵌套 README、文件系统兜底分类+自然排序、raw URL 拼接（根路径/嵌套路径两种情况）、真实仓库端到端导入。
- 前端：[remote-source-dialog.ts](src/remote-source-dialog.ts) 独立模块管理对话框的显示/校验/提交/错误展示；`main.ts::applyNewSource` 抽出本地导入和远程导入共用的"导入成功后刷新来源列表并切换过去"逻辑。

尚未验证（需要在真实桌面窗口里点击确认，见下方"需要你验证的部分"）：原生对话框在实际 Tauri 窗口（而非普通浏览器）里的完整点击流程——对话框本身的显示/隐藏/表单校验已经在浏览器里验证过，`add_remote_source` 后端命令也已用真实网络请求验证过，但两者结合的完整 GUI 操作没有在真机窗口里跑过一遍。

## 5.6 来源管理与拖动重排（P3，已实现）

- `remove_source(id)`：删除该来源在磁盘上的私有目录（`sources/<id>/` 整个删掉）+ 从 `state.json` 里移除对应的 `Source`/`DocMeta`/`LogEntry`。**顺序很关键**：先删磁盘文件、成功后才修改内存状态并落盘——这样万一删除文件失败，来源记录保持完整（用户可以重试），而不会留下一个指向已消失文件的"僵尸来源"。实现在 `sources.rs`（而不是 `local_import.rs`/`remote_import.rs`），因为它对所有来源类型都通用。2 个单元测试：正常删除、未知 id 报错。
  - 前端：顶部"🗑 移除来源"按钮，`window.confirm()` 二次确认后调用，成功后自动切换到剩余的第一个来源（如果一个都不剩，显示"没有可用的文档来源"占位提示）。**已在浏览器里验证按钮渲染正常**；实际点击删除的完整流程需要真机窗口验证。
- `set_nav_override(source_id, ordered_ids)`：直接覆盖 `Source.orderOverride` 并更新 `updatedAt`。排序应用逻辑（`nav::apply_order_override`）在 P2 就已实现并测试过，这里只是新增了"从前端写入"的入口。
  - 前端：[nav-view.ts](src/nav-view.ts) 用原生 HTML5 拖放 API 实现——每个文档项左侧有个"⠿"拖动把手，`dragover`/`drop` 用经典的"比较指针 Y 坐标与兄弟节点中点"算法决定插入位置，**限制只能在同一分类内拖动**（跨分类拖拽没有意义，因为分类由 manifest/兜底规则决定，不是任意的）；`dragend` 时把 DOM 里展平后的完整顺序发给 `set_nav_override` 持久化。失败时只 `console.error`，不打断用户——这是一个纯 UX 增强功能，偶发的持久化失败不值得用弹窗打断。**已在浏览器里验证按钮/DOM 结构渲染正常**；实际拖拽交互需要真机窗口里、加载了真实数据后才能验证（浏览器里因为没有 Tauri 后端，侧边栏是空的）。

## 6. wiki 式全文关键字搜索（P2，已实现，见 `search.rs`）

- **索引**：`InMemorySearch::build` 每次搜索时现场遍历所有来源所有语言的 md 构建索引（当前数据规模下现建现查足够快，且避免了在还没有任何命令会修改磁盘文档的阶段就引入缓存失效逻辑——待 P3/P4/P6 引入会修改文档的命令后可以再加缓存）。
- **查询**：`search_docs(query, source_id?, lang?, limit?)` 命令；多个关键词之间是 AND 关系（每个词必须在标题/标题层级/正文之一命中）；按 标题命中(+100) > 章节标题命中(+50) > 正文频次 分级打分排序；返回 `SearchHit { source_id, doc_id, lang, title, score, snippets }`，`snippets` 含命中词上下文 + 高亮字节范围（char-boundary 安全，中英文混排不会 panic）。
- **接口抽象**：`trait SearchBackend`，当前实现 `InMemorySearch`；后续来源量大可加 `SqliteFts5Search`，前端接口不变。
- **单元测试**（`search.rs` 内 6 个）：正文命中定位、AND 语义、标题优先排序、按来源/语言过滤、limit 截断、多字节 UTF-8（中文）边界安全。
- 前端：[api.ts](src/api.ts) 的 `searchDocs`、[search-view.ts](src/search-view.ts) 渲染 wiki 式结果页（跨来源分组、`<mark>` 高亮摘要、点击跳转），[main.ts](src/main.ts) 顶部搜索框（防抖 250ms，输入清空自动恢复原文档视图，跳转到其它来源的结果会自动切换来源+语言）。

## 7. 同步检查泛化（P4，已实现，见 `sync.rs`）

`check_updates(source_id)` / `apply_update(source_id, id)`，按来源类型分派"上游"的含义：

- **RemoteGit（以及带 `remote` 的 Seed 来源，如 Pi）**：`fetch_upstream_content` 对每篇文档重新请求 `raw.githubusercontent.com`，与 `DocMeta.primaryHash` 比对。**开发过程中真实抓到一次上游变化**：GitHub 上 `earendil-works/pi` 仓库在这次开发期间新增了一篇 `environment-variables.md`——这直接说明测试在打真实的网，也说明了这个功能本身要解决的问题是真实存在的（外部文档确实会变）。
- **LocalFolder**：`fetch_upstream_content` 改为从 `Source.localPath` 重新读取对应语言目录下的文件（复用 `local_import::lang_root`），不发网络请求。
- **没有 `remote` 也没有 `localPath` 的来源**（理论上不应出现）：直接报错"无法检查更新"。
- `check_updates` 更新每篇文档的 `lastCheckStatus`（`Same`/`Changed`/`Error`）+ `lastCheckedAt`，并写一条 `Check` 日志（汇总"共检查 N 篇，M 篇有更新，K 篇失败"）。
- `apply_update` 重新拉取内容覆盖本地主语言文件、更新哈希，若该文档原本 `translationStatus = Translated` 则降级为 `Pending`（提示"翻译已过期，需要重新翻译"，为 P6 埋钩子）；写一条 `ApplyUpdate` 日志。
- `get_history(source_id?, doc_id?)`：读取 `state.json` 里的 `log`，按来源/文档过滤，最新的排在最前面。
- **单元测试**（`sync.rs` 内 3 个）：用本地文件夹来源做"确定性变更检测"（直接改 upstream 文件夹里的文件、验证状态从 `Same` 翻转到 `Changed`、`apply_update` 后落盘内容确实更新且状态翻回 `Same`）、未知来源报错、针对 Pi 种子来源的真实网络检查（断言 `checked == 29`——这是我们内置的固定文档数，不受上游新增文件影响，因为 `check_updates` 只核对已登记的文档）。
- 前端：[api.ts](src/api.ts) 新增 `listDocs`/`checkUpdates`/`applyUpdate`/`getHistory`；[nav-view.ts](src/nav-view.ts) 侧边栏为 `lastCheckStatus === "changed"` 的文档加橙色圆点角标；[content-view.ts](src/content-view.ts) 的 `showDoc` 支持一个"上游内容已更新 [应用更新]"横幅；[history-view.ts](src/history-view.ts) 渲染历史记录列表；`main.ts` 顶部"🔄 检查更新"（仅当来源有 `remote`/`localPath` 时可点）+"📜 历史"按钮。**已在浏览器里验证 5 个头部按钮渲染正常**；实际点击检查/应用更新的完整 GUI 流程需要真机窗口验证。

## 7.5 翻译 — 每来源可选双语（P6，已实现，见 `translate.rs`）

- 单语来源（`languages.len()==1`）：`translation_status = NotApplicable`，不显示语言切换、不参与翻译。
- 双语来源：`translate_doc`/`translate_all_pending` 把 `Pending`/`NeverTranslated` 的文档变回 `Translated`；P4 的 `apply_update` 已经会把受影响文档降级为 `Pending`，两者衔接良好——原文一更新，对应译文自动标"需要重新翻译"。
- **目标语言**：取 `source.languages` 里除 `primary_language` 外的第一个语言（当前只有意义处理"恰好两种语言"的常见情形；理论上 3 种语言的来源只会翻译出第一个非主语言，够用但不是通用多语言方案）。
- **`TranslationStatus` 新增 `NeedsReview` 变体**：翻译完成但结构自查没通过时用这个状态，而不是静默标记 `Translated`——这是原计划"标记为需要人工检查"的具体落地方式。
- **Provider 可插拔适配层**（`provider.rs`）：

```rust
#[async_trait]
pub trait TranslationProvider: Send + Sync {
    async fn translate_chunk(&self, markdown: &str) -> Result<String>;
    async fn test_connection(&self) -> Result<()>;
}
```

`AnthropicProvider` 走 Messages API（`x-api-key` + `anthropic-version` 头）；`OpenAiCompatProvider` 走 chat completions（Bearer token），`baseUrl` 可指向任意 OpenAI 兼容端点。`build_provider(config)` 按 `ProviderConfig.kind` 路由，构造前就用 `resolve_key` 解析好 API key——key 解析失败会在这一步就快速报错。

统一翻译 system prompt（沿用已验证有效的规则，见 `provider::TRANSLATE_SYSTEM_PROMPT`）：

> 翻译成简体中文技术文档风格；代码块、行内代码、文件路径、环境变量名、命令/参数名、JSON 字段名、URL 一律保留原文；标题/表格/代码围栏结构与原文一一对应，不得增删内容；表格只翻译描述性单元格，内部链接的 `.md` 相对路径保持不变。

翻译流程三个可靠性改进（均已实现）：
1. **代码块占位保护**（`protect_code_blocks`/`restore_code_blocks`）：逐行状态机识别 fenced code block（\`\`\` 或 ~~~），替换成 `[[CODEBLOCK_n]]` 占位符只翻正文，回填时换回原始代码——精确到字符级还原，不依赖模型"答应"不改代码。
2. **超长文档分块**（`split_into_chunks`）：超过 `chunkThresholdChars`（`config.rs` 的 `TranslationSettings`）就按 `##` 二级标题切块分别翻译再拼接。
3. **翻译后结构自查**（`structural_check_passes`）：对比标题行数、代码围栏行数、表格分隔行数（`|---|---|` 这种），任一不一致就把状态设为 `NeedsReview` 而不是 `Translated`。

**测试策略**：`translate.rs` 的 6 个单元测试里，核心管道逻辑（占位保护往返、分块边界、结构自查、`translate_document` 端到端）用一个内建的 `UppercaseProvider`（把输入转大写当"翻译"）注入测试，完全不需要真实 API key 或网络；只有 `provider.rs` 里"用无法解析的 key 应该快速失败"/"字面量 key 应该能构造成功"这两个不发请求的测试。**真正调用 Anthropic/OpenAI API 的路径（`translate_chunk`/`test_connection` 实际发请求）没有、也不可能在这个环境里验证**——需要你自己的 API key，见第 15 节。

**Command**：`translate_doc(source_id, id) -> DocMeta`（单篇）、`translate_all_pending(source_id) -> ()`（批量，通过 `app.emit("translate-progress", {done, total, currentTitle, success, error})` 推送进度，逐篇成功就逐篇保存状态——中途失败不会丢掉已完成的翻译）。

**前端**：[content-view.ts](src/content-view.ts) 的 `showDoc` 改成接受 `DocBanner[]` 数组（原来只支持"应用更新"一个横幅，现在同一篇文档可以同时显示"应用更新"和"翻译"两个横幅）；`main.ts::buildDocBanners` 根据 `translationStatus` 决定显示"翻译"还是"重新翻译"（`needsReview` 时）；顶部"🌐 翻译全部待翻译"按钮点击后先 `window.confirm` 展示预计翻译篇数，再调用批量翻译并订阅 `onTranslateProgress` 事件更新内联进度文字。

## 8. 打磨：离线降级、备份导入导出、跨平台打包（P7，已实现）

- **`checkOnStartup` 真正接了线**：这个配置项在 P5 就已经存在于 `TranslationSettings` 里、设置页也有对应的勾选框，但一直没有代码真的读它——这是开发过程中发现的一个真实缺口，不是凭空加的功能。现在 `lib.rs` 的 `setup` 钩子里用 `tauri::async_runtime::spawn` 启动一个后台任务，调用 `sync::run_startup_checks`：读配置，`checkOnStartup` 为 `false`（用户在设置里关掉）就直接跳过；为 `true`（默认值，与 `config.json` 示例一致）就对每个"有 `remote` 或 `local_path`"的来源跑一次 `check_updates`。全程不阻塞应用启动，单个来源检查失败（比如没网）只打日志、跳过，继续处理下一个来源——这就是"离线降级"的具体体现：没网不会让应用起不来或卡住，只是那次检查没结果。2 个单元测试：默认启用时真的会检查、显式关闭时是空操作。
- **备份导入导出**（`backup.rs`）：`export_backup`/`import_backup` 把 `state.json` + `config.json`（元数据：来源列表、文档状态、翻译设置——**不包含文档正文**，正文在 `sources/<id>/docs/` 下不属于备份范围）打包成一个 JSON 文件。已知的取舍：
  - 如果用户在设置页 API Key 输入框里直接填了明文（没用 `$ENV_VAR` 表达式），导出的备份文件会包含这个明文——前端在备份区域用一段提示文字明确警告这一点，而不是静默导出。
  - 在没有对应 `sources/<id>/` 磁盘内容的机器上导入备份，来源会出现在列表里但打开文档会报错——这是"备份"和"完整迁移"的本质区别，已在前端确认弹窗和计划里写清楚，不是缺陷。
  - 3 个单元测试：跨目录导出再导入的往返一致性、格式错误的备份文件报错、`state.json`/`config.json` 都还不存在时导出不出错（用默认值兜底）。
  - 前端：入口放在"⚙ 设置"对话框底部（而不是顶部再加两个按钮——头部已经有 8 个按钮，备份是维护性操作，跟设置放一起更合理），导入前有二次确认弹窗，导入成功后复用 `refreshSourcesAfterExternalChange`（与移除来源共享的"重新拉取来源列表并选中/清空"逻辑，避免了重复代码）。
- **`tauri build` 打包验证**：已实际跑通并产出可用的 macOS 安装包——
  - `DocsWiki.app`（16MB，arm64 Mach-O）+ `DocsWiki_0.1.0_aarch64.dmg`（约 6MB，`hdiutil verify` 校验通过）。
  - 过程中遇到并诊断了两个环境问题（均已解决，不是代码 bug）：一是 create-dmg 的临时读写镜像自动计算的大小偶尔不够导致"设备上无剩余空间"；二是给 DMG 做 Finder 图标排版时的 AppleScript 偶发因为 Finder 响应慢而超时（`AppleEvent已超时`）。两次都是重跑就好，属于本机自动化脚本与 Finder/AppleScript 交互时的已知不稳定点，跟应用代码无关。
- **一处已知但暂不修的性能缺口**（诚实记录，不掩盖）：`translate_all_pending` 目前是**顺序**翻译，没有使用 `TranslationSettings.concurrency` 做并发限流——正确但不是最快。原因：批量翻译当前是"翻一篇成功就存一篇"的增量保存模式（中途失败不丢已完成的），改成并发执行需要协调多个并发任务对同一个 `AppStateData` 快照的写入合并，复杂度会显著上升；在验证不到真实 API 性能瓶颈之前，先保证正确性和崩溃安全，不做这个优化。

## 9. `config.json` 设计（P5，已实现，见 `config.rs`）

```jsonc
{
  "activeProvider": "anthropic",
  "providers": {
    "anthropic": { "kind": "anthropicMessages", "baseUrl": "https://api.anthropic.com", "apiKey": "$ANTHROPIC_API_KEY", "model": "claude-sonnet-4-5", "maxOutputTokens": 8192 },
    "openai": { "kind": "openAiCompatible", "baseUrl": "https://api.openai.com/v1", "apiKey": "$OPENAI_API_KEY", "model": "gpt-4o", "maxOutputTokens": 8192 }
  },
  "translation": { "autoTranslateOnChange": false, "checkOnStartup": true, "concurrency": 2, "chunkThresholdChars": 12000 }
}
```

（`kind` 序列化为 camelCase 而不是原计划的 kebab-case，跟项目里其余所有字段的命名约定保持一致。）

- **Key 解析规则**（`config::resolve_key`）：`$ENV_VAR` 读环境变量；`!command` 用 `sh -c` 执行命令取 stdout 并去除首尾空白（如读系统钥匙串 `security find-generic-password`）；其余按字面量处理。环境变量不存在或命令执行失败都会返回明确错误（而不是静默用空字符串）。
- **脱敏**：`get_config` 返回 `PublicConfig`，其 `PublicProviderConfig` 类型本身就没有 `apiKey` 字段（只有 `hasApiKey: bool`）——这是类型系统保证的，不是靠"记得脱敏"这种约定。
- **只增量覆盖 key，不清空**：`save_config` 把前端传来的 `ConfigInput` 与磁盘上现有配置合并（`apply_config_input`）——某个 provider 的 `apiKey` 字段是 `None`/空字符串时，沿用已存的 key；只有传了非空新值才会覆盖。这样前端设置页的 key 输入框永远可以留空（表示"不改"），不需要也不能回显旧值。
- **原子写入**：`save_config_atomic` 与 `state::save_state_atomic` 同样的"写临时文件 + rename"模式。
- **单元测试**（`config.rs` 内 9 个）：三种 key 解析路径（环境变量/命令/字面量）及各自的失败情况、脱敏后 JSON 里确实找不到原始 key 字符串、合并语义（留空保留旧 key / 提供新值覆盖）、配置文件读写往返、配置文件不存在时的默认值。

## 10. Tauri Command 接口面

```rust
// 已实现（P0/P1）
list_sources() -> Vec<Source>
list_docs(source_id: String) -> Vec<DocMeta>
get_nav(source_id: String) -> NavTree              // 已应用 order_override
get_doc_content(source_id: String, id: String, lang: String) -> String  // 每次从磁盘现读

// 已实现（P2）
search_docs(query: String, source_id: Option<String>, lang: Option<String>, limit: Option<usize>) -> Vec<SearchHit>

// 已实现（P3）
add_local_source(path: String, name: Option<String>) -> Source
add_remote_source(owner: String, repo: String, branch: String, path: String, name: Option<String>, lang: Option<String>) -> Source
remove_source(source_id: String) -> ()
set_nav_override(source_id: String, ordered_ids: Vec<String>) -> ()

// 已实现（P4）
check_updates(source_id: String) -> CheckSummary
apply_update(source_id: String, id: String) -> DocMeta
get_history(source_id: Option<String>, doc_id: Option<String>) -> Vec<LogEntry>

// 已实现（P5）
get_config() -> PublicConfig
save_config(input: ConfigInput) -> ()
test_provider_connection(provider_id: String) -> ()   // 抛错即为失败，成功不返回内容

// 已实现（P6）
translate_doc(source_id: String, id: String) -> DocMeta
translate_all_pending(source_id: String) -> ()      // emit 'translate-progress' 事件

// 已实现（P7）
export_backup(dest_path: String) -> ()
import_backup(src_path: String) -> ()
```

## 11. 前端

- 已实现（P1）：`api.ts`（invoke 类型化封装）、`nav-view.ts`（侧边栏渲染）、`content-view.ts`（loading/error/markdown 渲染）、`markdown.ts`（本地打包的 `marked`，离线渲染）、`main.ts`（来源选择 + 语言切换 + 导航 + 内容面板拼装）。
- 已实现（P2）：`search-view.ts`（wiki 式结果页渲染，`<mark>` 高亮）、`lang.ts`（语言标签共用小工具）、`main.ts` 顶部搜索框（防抖、跨来源跳转）。
- 已实现（P3 本地导入部分）：顶部"+ 导入文件夹"按钮（`@tauri-apps/plugin-dialog` 原生文件夹选择器）、`main.ts::addLocalFolderSource`（导入中禁用按钮+文案反馈，成功后刷新来源列表并自动切换）、`api.ts::addLocalSource`。
- 数据源全部来自 `invoke()`，不使用 `localStorage`——单一数据源是 Rust 侧的 `state.json`。
- 双语来源默认优先展示中文（`main.ts::selectSource` 中的语言选择逻辑），单语来源自动隐藏语言切换按钮。
- 已实现（P3）：`main.ts` 顶部"🗑 移除来源"按钮（`removeCurrentSource`，二次确认）；[nav-view.ts](src/nav-view.ts) 侧边栏 HTML5 拖放重排（同分类内），`main.ts::persistNavOrder` 写入 `set_nav_override`。
- 已实现（P4）：[history-view.ts](src/history-view.ts)（历史记录列表渲染）；`main.ts` 顶部"🔄 检查更新"（按来源类型自动启用/禁用）+"📜 历史"按钮；[nav-view.ts](src/nav-view.ts) 为待更新文档加橙色角标；[content-view.ts](src/content-view.ts) 的 `showDoc` 支持"应用更新"横幅。
- 已实现（P5）：[settings-dialog.ts](src/settings-dialog.ts) 独立模块——固定的 Anthropic / OpenAI 兼容两个 provider 卡片（而非任意动态增删的 provider 列表，匹配 `config.json` 示例里就是这两个固定 id）+ 翻译行为设置；API key 输入框永远留空展示（`placeholder` 提示"已配置"与否），"测试连接"按钮会先静默保存当前表单再调用 `test_provider_connection`（避免测试用的是磁盘上过期的配置）。
- 已实现（P6）：[content-view.ts](src/content-view.ts) 的 `showDoc` 从单个"应用更新"横幅参数改成 `DocBanner[]` 数组（一篇文档可以同时显示"应用更新"+"翻译"两个横幅）；`main.ts::buildDocBanners` 按 `translationStatus` 决定显示"翻译"/"重新翻译"（`needsReview` 时）；顶部"🌐 翻译全部待翻译"按钮：点击先 `confirm` 展示预计翻译篇数，再调用批量翻译并通过 `api.ts::onTranslateProgress`（封装 `@tauri-apps/api/event` 的 `listen`）订阅进度、更新头部内联进度文字。
- 已实现（P7）：备份导入导出的入口放进"⚙ 设置"对话框（[settings-dialog.ts](src/settings-dialog.ts) 新增 `runExportBackup`/`runImportBackup`，用 `@tauri-apps/plugin-dialog` 的 `save`/`open` 选文件路径）；`main.ts::refreshSourcesAfterExternalChange` 是从 `removeCurrentSource` 里抽出来的共用逻辑，导入备份后同样需要"重新拉取来源列表 + 选中第一个或显示空状态"。
- 后续新增：来源重命名（未在任何 P0-P7 范围内规划，可按需补）。

## 12. 安全性要点

- `config.json` 存 key 的"取值表达式"，推荐 `$ENV_VAR` 或 `!command`；明文 key 只在 Rust 进程内存里短暂存在，不通过 `invoke` 返回值传给前端 JS。
- `sources/`、`state.json`、`config.json` 都在应用私有数据目录下。

## 13. 分阶段实施计划

| 阶段 | 目标 | 状态 |
|---|---|---|
| **P0** | `create-tauri-app` 初始化；多来源存储布局；Pi 种子作为内置 Seed 来源导入（`sources.rs::ensure_seed_source`） | ✅ 已完成，单元测试通过 |
| **P1** | 只读浏览：`list_sources`/`get_nav`/`get_doc_content` + 来源选择/侧边栏/中英切换/Markdown 渲染 | ✅ 已完成 |
| **P2** | 全文关键字搜索：`search_docs` + 内存索引 + 顶部搜索框 + wiki 式结果页 | ✅ 已完成，单元测试通过 |
| **P3** | 新增来源：本地文件夹导入 + 远程 Git 抓取 + 分层排序解析完善 + 手动拖动重排 | ✅ 已完成（单元测试通过，远程部分含真实网络端到端测试）；来源重命名、front-matter 排序策略仍是后续小项 |
| **P4** | 同步检查泛化：按来源 `RemoteSpec`/`local_path` 的 `check_updates`/`apply_update` + 历史日志 + 更新角标 | ✅ 已完成（单元测试通过，含真实网络检测到的一次上游变化） |
| **P5** | 配置管理：`config.json` + 设置页 + Provider trait（Anthropic/OpenAI 兼容）+ 连接测试 | ✅ 已完成（单元测试通过；`test_provider_connection` 对真实 API 的验证需要你自己的 key，见下方"需要你验证的部分"） |
| **P6** | 翻译流程（仅双语来源）：代码块占位保护 + 分块 + 单篇/批量 + 进度事件 + 结构自查 | ✅ 已完成（管道逻辑单元测试通过；真实 API 调用需要你自己的 key 验证，见第 15 节） |
| **P7** | 打磨：错误提示、离线降级、备份导入导出、跨平台打包 | ✅ 已完成（`checkOnStartup` 接线、备份导入导出均有单元测试；`tauri build` 已实测产出 `.app` + `.dmg`） |

依赖顺序：P2 只依赖 P1；P3 的排序解析被 P0/P1 的 manifest 生成复用；P4 依赖 P3 的 `RemoteSpec`；P6 依赖 P5 的 Provider。

## 14. 风险与注意事项

- **超长原文抓取**：`extensions.md` 原文约 97KB/2600 行；Rust `reqwest` 不会有截断问题，翻译阶段的分块逻辑（第 7.5 节，已实现）解决了发给模型时的长度问题。
- **速率与成本控制**：`concurrency` 配置项已定义在 `TranslationSettings` 里，但 `translate_all_pending` 目前是**顺序执行**、还没有真正做并发限流（做批量翻译时是一篇一篇串行翻译，不会因为并发把 API 打爆，但也没有利用 `concurrency` 加速——这是一个待改进点，不算 bug，只是还没用上这个配置项）；批量翻译前会给用户看"预计翻译 N 篇"确认弹窗（已实现，`main.ts::runTranslateAllPending`）。
- **WebView 兼容性**：Tauri 在 Windows 用 WebView2、macOS/Linux 用系统 WebKit，本项目前端只用基础 DOM/Fetch/CSS，跨平台风险低。
- **状态文件损坏**：`state.json` 原子写入，避免崩溃或断电导致文件半写坏掉。

---

## 15. 需要你验证的部分

以下几项是自动化验证在这个开发环境里做不到、需要你在真机上确认的：

1. **真实 LLM Provider 连接与翻译质量**：`test_provider_connection`、`translate_doc`、`translate_all_pending` 的实际网络调用都需要你自己的 Anthropic 或 OpenAI（兼容）API key——这个开发环境没有任何 key，`provider.rs`/`translate.rs` 的测试只验证了不发请求的部分（key 解析失败快速报错、管道逻辑用假 provider 跑通）。在"⚙ 设置"里填好 key（推荐用 `$ANTHROPIC_API_KEY` 这种环境变量表达式，不要把明文 key 直接存进配置文件）：
   - 点"测试连接"验证 API 能不能连上。
   - 找一篇双语来源里状态是"待翻译"的文档，点"翻译"，检查译文质量、代码块是否被完整保留、遇到表格/多级标题的文档是否语义连贯。
   - 试一次"翻译全部待翻译"，留意进度条数字是否符合预期、批量过程中断网或 key 失效时是否会中止在正确的地方（而不是丢失已经翻译成功的部分——设计上是逐篇保存，但没有真实 key 没法实测这个"失败恢复"路径）。
2. **原生窗口里的完整 GUI 交互**：本地文件夹导入的原生文件夹选择器、远程仓库导入表单、拖动重排侧边栏文档、检查更新/应用更新/翻译的按钮点击流程——这些的**后端命令**都已经过单元测试（部分含真实网络请求）验证，**前端 DOM 结构/校验/开合逻辑**也已经在普通浏览器里验证过，但两者结合的完整点击流程需要在真实的 Tauri 窗口里跑一遍才算数（这个环境里的截图工具无法附加到未打包的开发二进制上，见前面章节的说明）。
3. **`tauri build` 产出的安装包在真机上双击安装/打开是否正常**：已经在这个开发环境里验证过 `pnpm tauri build` 能成功产出 `DocsWiki.app` + `DocsWiki_0.1.0_aarch64.dmg`（`hdiutil verify` 通过、`.app` 内可执行文件是合法的 arm64 Mach-O），但"双击 dmg → 拖到 Applications → 打开应用"这个最终用户视角的安装体验没有跑过——尤其要注意**这个 .app 没有签名/公证**，macOS Gatekeeper 大概率会拦截，需要右键"打开"或在系统设置里放行，这在自动化环境里没法测。
4. **备份导入导出的实际文件对话框交互**：`export_backup`/`import_backup` 后端命令 + `state.json`/`config.json` 往返一致性已有单元测试覆盖，但设置页里"导出备份…"/"导入备份…"两个按钮触发的原生文件保存/打开对话框，同样受限于"截图工具无法附加到未打包开发二进制"，需要真机点击验证。

除此之外的功能（多来源导入、排序、搜索、同步检查、配置持久化、翻译管道逻辑、启动时后台检查更新、备份读写、release 打包）都已经过自动化测试或直接命令验证，包括好几个真正发起网络请求命中 GitHub 真实仓库的测试，以及一次成功产出真实安装包的 `tauri build`。

## 给 Claude Code 的启动建议

P0-P7 已经全部落地（`cargo test` 54/54 通过，`pnpm exec tsc --noEmit` 无报错，`tauri build` 已验证能产出可用的 macOS 安装包）。DEVELOPMENT_PLAN.md 里规划的核心功能全部完成：多来源导入（本地/远程/内置种子）+ 分层排序、全文搜索、来源管理、同步检查、配置管理、翻译管道、启动时后台检查、备份导入导出、release 打包。

剩余是一些明确记录在案、非阻塞的小尾巴，可以按需捡起来做：
- 来源重命名（从未被任何 Pn 阶段规划过，是一个自然的后续小功能）。
- `front-matter` 排序策略（`nav.rs` 分层排序的第 3 层，第 4 层文件系统兜底已经实现，front-matter 这层一直没做，实际使用中文件系统兜底已经够用）。
- `translate_all_pending` 的并发限流（`concurrency` 配置项已存在但未使用，见第 8 节的说明）。
- 第 15 节列出的、需要你亲自动手验证的几项（真实 API key、真机 GUI 交互、安装包的 Gatekeeper 体验）。

一个阶段做完、验收通过后，再继续下一个 Phase 即可。
