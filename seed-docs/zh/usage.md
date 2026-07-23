# 使用 Pi

本页汇总了快速开始页面未涵盖的日常使用细节。

## 交互模式

界面有四个主要区域：

- **启动头部** - 快捷键、已加载的上下文文件、提示词模板、技能和扩展
- **消息区** - 用户消息、助手回复、工具调用、工具结果、通知、错误以及扩展 UI
- **编辑器** - 你输入内容的地方；边框颜色表示当前的思考等级
- **页脚** - 工作目录、会话名称、token/缓存使用情况、费用、上下文使用情况以及当前模型

编辑器可以被内置 UI（例如 `/settings`）或自定义扩展 UI 临时替换。

### 编辑器功能

| 功能 | 方式 |
|---------|-----|
| 文件引用 | 输入 `@` 以模糊搜索项目文件 |
| 路径补全 | 按 Tab 补全路径 |
| 多行输入 | Shift+Enter，或在 Windows Terminal 中使用 Ctrl+Enter |
| 图片 | 使用 Ctrl+V 粘贴，Windows 上为 Alt+V，或拖入终端 |
| Shell 命令 | `!command` 运行命令并将输出发送给模型 |
| 隐藏 shell 命令 | `!!command` 运行命令但不将输出发送给模型 |
| 外部编辑器 | Ctrl+G 打开 `$VISUAL` 或 `$EDITOR` |

有关所有快捷键和自定义方式，请参阅[按键绑定](keybindings.md)。

## 斜杠命令

在编辑器中输入 `/` 打开命令补全。扩展可以注册自定义命令，技能可以通过 `/skill:name` 使用，提示词模板则通过 `/templatename` 展开。

| 命令 | 描述 |
|---------|-------------|
| `/login`、`/logout` | 管理 OAuth 或 API 密钥凭据 |
| `/model` | 切换模型 |
| `/scoped-models` | 启用/禁用用于 Ctrl+P 循环切换的模型 |
| `/settings` | 思考等级、主题、消息投递方式、传输方式 |
| `/resume` | 从之前的会话中选择 |
| `/new` | 开始一个新会话 |
| `/name <name>` | 设置会话显示名称 |
| `/session` | 显示会话文件、ID、消息、token 和费用 |
| `/tree` | 跳转到会话中的任意节点并从那里继续 |
| `/fork` | 从之前的用户消息创建一个新会话 |
| `/clone` | 将当前活动分支复制到一个新会话中 |
| `/compact [prompt]` | 手动压缩上下文，可选附带自定义指令 |
| `/copy` | 将最后一条助手消息复制到剪贴板 |
| `/export [file]` | 将会话导出为 HTML |
| `/share` | 以私有 GitHub gist 的形式上传，并生成可分享的 HTML 链接 |
| `/reload` | 重新加载按键绑定、扩展、技能、提示词和上下文文件 |
| `/hotkeys` | 显示所有键盘快捷键 |
| `/changelog` | 显示版本历史 |
| `/quit` | 退出 pi |

## 消息队列

你可以在代理仍在工作时提交消息：

- **Enter** 会将消息加入引导（steering）队列，在当前助手轮次执行完其工具调用后投递。
- **Alt+Enter** 会将消息加入后续（follow-up）队列，在代理完成所有工作后投递。
- **Escape** 会中止操作，并将排队的消息恢复到编辑器中。
- **Alt+Up** 会将已排队的消息取回到编辑器中。

在 Windows Terminal 中，Alt+Enter 默认用于全屏切换。如果你希望 pi 能接收到该快捷键，请按照[终端设置](terminal-setup.md)中的说明重新映射它。

可以在[设置](settings.md)中通过 `steeringMode` 和 `followUpMode` 配置投递方式。

## 会话

会话会自动保存到 `~/.pi/agent/sessions/`，并按工作目录进行组织。

```bash
pi -c                  # 继续最近的会话
pi -r                  # 浏览并选择一个会话
pi --no-session        # 临时模式；不保存
pi --session <path|id> # 使用指定的会话文件或会话 ID
pi --fork <path|id>    # 将一个会话分叉（fork）为新的会话文件
```

常用的会话命令：

- `/session` 显示当前会话文件和 ID。
- `/tree` 在文件内的会话树中导航，并可以对已放弃的分支进行摘要。
- `/fork` 从较早的用户消息创建一个新会话。
- `/clone` 将当前活动分支复制到一个新会话文件中。
- `/compact` 对较旧的消息进行摘要以释放上下文。

详情请参阅[会话](sessions.md)和[压缩](compaction.md)。

## 上下文文件

Pi 在启动时会从以下位置加载 `AGENTS.md` 或 `CLAUDE.md`：

- `~/.pi/agent/AGENTS.md`，用于全局指令
- 从当前工作目录向上遍历的父目录
- 当前目录

使用上下文文件来定义项目约定、命令、安全规则和偏好设置。使用 `--no-context-files` 或 `-nc` 可禁用加载。

### 系统提示词文件

使用以下文件替换默认系统提示词：

- 项目级别的 `.pi/SYSTEM.md`
- 全局级别的 `~/.pi/agent/SYSTEM.md`

使用上述任一位置中的 `APPEND_SYSTEM.md`，可以在不替换默认提示词的情况下对其进行追加。

## 导出和分享会话

使用 `/export [file]` 将会话写出为 HTML。

使用 `/share` 上传为私有 GitHub gist，并生成可分享的 HTML 链接。

如果你将 pi 用于开源工作，并希望为模型、提示词、工具和评估研究发布会话，请参阅 [`badlogic/pi-share-hf`](https://github.com/badlogic/pi-share-hf)。它会将会话发布到 Hugging Face 数据集。

## CLI 参考

```bash
pi [options] [@files...] [messages...]
```

### 包命令

```bash
pi install <source> [-l]     # 安装包，-l 表示项目本地安装
pi remove <source> [-l]      # 移除包
pi uninstall <source> [-l]   # remove 的别名
pi update [source|self|pi]   # 更新 pi 和各个包；协调已固定的 git 引用
pi update --extensions       # 仅更新包；协调已固定的 git 引用
pi update --self             # 仅更新 pi
pi update --extension <src>  # 更新单个包
pi list                      # 列出已安装的包
pi config                    # 启用/禁用包资源
```

这些命令管理的是 pi 包，而不是 pi CLI 本身的安装。要卸载 pi 本身，请参阅[快速开始](quickstart.md#uninstall)。

有关包来源和安全说明，请参阅 [Pi 包](packages.md)。

### 模式

| 标志 | 描述 |
|------|-------------|
| 默认 | 交互模式 |
| `-p`、`--print` | 打印回复并退出 |
| `--mode json` | 以 JSON 行形式输出所有事件；参见 [JSON 模式](json.md) |
| `--mode rpc` | 通过 stdin/stdout 的 RPC 模式；参见 [RPC 模式](rpc.md) |
| `--export <in> [out]` | 将会话导出为 HTML |

在打印模式下，pi 还会读取通过管道传入的 stdin，并将其合并到初始提示词中：

```bash
cat README.md | pi -p "Summarize this text"
```

### 模型选项

| 选项 | 描述 |
|--------|-------------|
| `--provider <name>` | 提供方，例如 `anthropic`、`openai` 或 `google` |
| `--model <pattern>` | 模型模式或 ID；支持 `provider/id` 以及可选的 `:<thinking>` |
| `--api-key <key>` | API 密钥，覆盖环境变量 |
| `--thinking <level>` | `off`、`minimal`、`low`、`medium`、`high`、`xhigh` |
| `--models <patterns>` | 用于 Ctrl+P 循环切换的、以逗号分隔的模式列表 |
| `--list-models [search]` | 列出可用模型 |

### 会话选项

| 选项 | 描述 |
|--------|-------------|
| `-c`、`--continue` | 继续最近的会话 |
| `-r`、`--resume` | 浏览并选择一个会话 |
| `--session <path\|id>` | 使用指定的会话文件或部分 UUID |
| `--fork <path\|id>` | 将一个会话文件或部分 UUID 分叉为新会话 |
| `--session-dir <dir>` | 自定义会话存储目录 |
| `--no-session` | 临时模式；不保存 |

### 工具选项

| 选项 | 描述 |
|--------|-------------|
| `--tools <list>`、`-t <list>` | 将特定内置、扩展和自定义工具列入允许列表 |
| `--exclude-tools <list>`、`-xt <list>` | 禁用特定内置、扩展和自定义工具 |
| `--no-builtin-tools`、`-nbt` | 禁用内置工具，但保留扩展/自定义工具启用状态 |
| `--no-tools`、`-nt` | 禁用所有工具 |

内置工具：`read`、`bash`、`edit`、`write`、`grep`、`find`、`ls`。

### 资源选项

| 选项 | 描述 |
|--------|-------------|
| `-e`、`--extension <source>` | 从路径、npm 或 git 加载扩展；可重复使用 |
| `--no-extensions` | 禁用扩展发现 |
| `--skill <path>` | 加载一个技能；可重复使用 |
| `--no-skills` | 禁用技能发现 |
| `--prompt-template <path>` | 加载一个提示词模板；可重复使用 |
| `--no-prompt-templates` | 禁用提示词模板发现 |
| `--theme <path>` | 加载一个主题；可重复使用 |
| `--no-themes` | 禁用主题发现 |
| `--no-context-files`、`-nc` | 禁用 `AGENTS.md` 和 `CLAUDE.md` 的发现 |

将 `--no-*` 与显式标志组合使用，可以精确加载所需内容，忽略设置。示例：

```bash
pi --no-extensions -e ./my-extension.ts
```

### 其他选项

| 选项 | 描述 |
|--------|-------------|
| `--system-prompt <text>` | 替换默认提示词；上下文文件和技能仍会被追加 |
| `--append-system-prompt <text>` | 追加到系统提示词 |
| `--verbose` | 强制启用详细启动信息 |
| `-h`、`--help` | 显示帮助 |
| `-v`、`--version` | 显示版本 |

### 文件参数

在文件前加上 `@` 前缀，将其包含到消息中：

```bash
pi @prompt.md "Answer this"
pi -p @screenshot.png "What's in this image?"
pi @code.ts @test.ts "Review these files"
```

### 示例

```bash
# Interactive with initial prompt
pi "List all .ts files in src/"

# Non-interactive
pi -p "Summarize this codebase"

# Non-interactive with piped stdin
cat README.md | pi -p "Summarize this text"

# Different model
pi --provider openai --model gpt-4o "Help me refactor"

# Model with provider prefix
pi --model openai/gpt-4o "Help me refactor"

# Model with thinking level shorthand
pi --model sonnet:high "Solve this complex problem"

# Limit model cycling
pi --models "claude-*,gpt-4o"

# Read-only mode
pi --tools read,grep,find,ls -p "Review the code"

# Disable one extension or built-in tool while keeping the rest available
pi --exclude-tools ask_question
```

### 环境变量

| 变量 | 描述 |
|----------|-------------|
| `PI_CODING_AGENT_DIR` | 覆盖配置目录；默认值为 `~/.pi/agent` |
| `PI_CODING_AGENT_SESSION_DIR` | 覆盖会话存储目录；会被 `--session-dir` 覆盖 |
| `PI_PACKAGE_DIR` | 覆盖包目录，对 Nix/Guix store 路径很有用 |
| `PI_OFFLINE` | 禁用启动时的网络操作，包括更新检查、包更新检查以及安装/更新遥测 |
| `PI_SKIP_VERSION_CHECK` | 跳过启动时的 Pi 版本更新检查。这可以避免向 `pi.dev` 发送最新版本请求 |
| `PI_TELEMETRY` | 覆盖安装/更新遥测设置：`1`/`true`/`yes` 或 `0`/`false`/`no`。这不会禁用更新检查 |
| `PI_CACHE_RETENTION` | 设为 `long`，可在受支持的场景下延长提示词缓存时间 |
| `VISUAL`、`EDITOR` | Ctrl+G 使用的外部编辑器 |

## 设计原则

Pi 让核心保持精简，并将特定于工作流的行为下放到扩展、技能、提示词模板和包中。

它有意不内置 MCP、子代理（sub-agents）、权限弹窗、计划模式（plan mode）、待办事项（to-dos）或后台 bash。你可以将这些工作流构建或安装为扩展或包，也可以使用容器和 tmux 等外部工具。

有关完整的设计理念，请阅读[这篇博客文章](https://mariozechner.at/posts/2025-11-30-pi-coding-agent/)。
