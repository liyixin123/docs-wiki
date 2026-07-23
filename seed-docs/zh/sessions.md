# 会话

Pi 将对话保存为会话，这样你可以继续之前的工作、从早先的轮次分支，并回顾之前的路径。

## 会话存储

会话会自动保存到 `~/.pi/agent/sessions/`，按工作目录组织。每个会话都是一个具有树结构的 JSONL 文件。

```bash
pi -c                  # Continue most recent session
pi -r                  # Browse and select from past sessions
pi --no-session        # Ephemeral mode; do not save
pi --name "my task"    # Set session display name at startup
pi --session <path|id> # Use a specific session file or partial session ID
pi --fork <path|id>    # Fork a session file or partial session ID into a new session
```

在交互模式下使用 `/session` 可以查看当前会话文件、会话 ID、消息数量、token 数和费用。

有关 JSONL 文件格式和 SessionManager API，请参见[会话格式](session-format.md)。

## 会话命令

| Command | Description |
|---------|-------------|
| `/resume` | 浏览并选择之前的会话 |
| `/new` | 开始新会话 |
| `/name <name>` | 设置当前会话的显示名称 |
| `/session` | 显示会话信息 |
| `/tree` | 浏览当前会话树 |
| `/fork` | 从之前的用户消息创建新会话 |
| `/clone` | 将当前活动分支复制到新会话中 |
| `/compact [prompt]` | 总结较早的上下文；参见[压缩](compaction.md) |
| `/export [file]` | 将会话导出为 HTML |
| `/share` | 以私有 GitHub gist 的形式上传，并生成可分享的 HTML 链接 |

## 恢复与删除会话

`/resume` 会为当前项目打开一个交互式会话选择器。`pi -r` 会在启动时打开相同的选择器。

在选择器中，你可以：

- 通过输入进行搜索
- 使用 Ctrl+P 切换路径显示
- 使用 Ctrl+S 切换排序模式
- 使用 Ctrl+N 过滤为仅显示已命名的会话
- 使用 Ctrl+R 重命名
- 使用 Ctrl+D 删除，然后确认

在可用的情况下，pi 会使用 `trash` CLI 进行删除，而不是永久移除文件。

## 命名会话

使用 `/name <name>` 设置一个易读的会话名称：

```text
/name Refactor auth module
```

在启动时使用 `--name` 或 `-n` 设置名称：

```bash
pi --name "Refactor auth module"
pi --name "CI audit" -p "Review this build failure"
```

已命名的会话在 `/resume` 和 `pi -r` 中更容易查找。

## 使用 `/tree` 进行分支管理

会话以树的形式存储。每个条目都有一个 `id` 和 `parentId`，当前位置是活动叶子节点。`/tree` 可以让你跳转到之前的任意一点，并从那里继续，而不会创建新文件。

示例结构：

```text
├─ user: "Hello, can you help..."
│  └─ assistant: "Of course! I can..."
│     ├─ user: "Let's try approach A..."
│     │  └─ assistant: "For approach A..."
│     │     └─ user: "That worked..."  ← active
│     └─ user: "Actually, approach B..."
│        └─ assistant: "For approach B..."
```

### 树控制

| Key | Action |
|-----|--------|
| ↑/↓ | 浏览可见条目 |
| ←/→ | 向上/向下翻页 |
| Ctrl+←/Ctrl+→ 或 Alt+←/Alt+→ | 折叠/展开或在分支段之间跳转 |
| Shift+L | 设置或清除所选条目上的标签 |
| Shift+T | 切换标签时间戳 |
| Enter | 选择条目 |
| Escape/Ctrl+C | 取消 |
| Ctrl+O | 循环切换过滤模式 |

过滤模式包括：default、no-tools、user-only、labeled-only 和 all。可以在[设置](settings.md)中通过 `treeFilterMode` 配置默认值。

### 选择行为

选择用户消息或自定义消息时：

1. 将叶子节点移动到所选消息的父节点。
2. 将所选消息文本放入编辑器中。
3. 允许你编辑并重新提交，从而创建一个新分支。

选择助手、工具、压缩或其他非用户条目时：

1. 将叶子节点移动到该条目。
2. 编辑器保持为空。
3. 允许你从该点继续。

选择根用户消息会将叶子节点重置为空对话，并将原始提示放入编辑器中。

## `/tree`、`/fork` 和 `/clone`

| Feature | `/tree` | `/fork` | `/clone` |
|---------|---------|---------|----------|
| 输出 | 同一个会话文件 | 新会话文件 | 新会话文件 |
| 视图 | 完整树 | 用户消息选择器 | 当前活动分支 |
| 典型用途 | 就地探索不同方案 | 从早先的提示开始新会话 | 在继续之前复制当前工作 |
| 摘要 | 可选的分支摘要 | 无 | 无 |

当你想将不同方案保留在一起时，使用 `/tree`。当你想要一个独立的会话文件时，使用 `/fork` 或 `/clone`。

## 分支摘要

当 `/tree` 从一个分支切换到另一个分支时，pi 可以对被放弃的分支进行摘要，并将该摘要附加到新位置。这样可以在不重放整个分支的情况下，保留你离开的路径中的重要上下文。

出现提示时，可以选择以下之一：

1. 不生成摘要
2. 使用默认提示生成摘要
3. 使用自定义关注点说明生成摘要

有关分支摘要的内部实现和扩展钩子，请参见[压缩](compaction.md)。

## 会话格式

会话文件是 JSONL 格式，包含消息条目、模型变更、思考级别变更、标签、压缩、分支摘要以及扩展条目。

有关解析器、扩展、SDK 用法以及完整的 SessionManager API，请参见[会话格式](session-format.md)。
