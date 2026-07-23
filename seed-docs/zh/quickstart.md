# 快速开始

本页将带你从安装到运行一个有用的首次 pi 会话。

## 安装

Pi 以 npm 包的形式分发：

```bash
npm install -g --ignore-scripts @earendil-works/pi-coding-agent
```

`--ignore-scripts` 会在安装期间禁用依赖生命周期脚本。正常的 npm 安装并不需要 Pi 的安装脚本。

### 卸载

请使用安装 pi 时所用的包管理器。curl 安装脚本在全局范围内使用 npm，因此通过 curl 和 npm 安装的版本都用 npm 卸载：

```bash
# curl 安装脚本或 npm install -g
npm uninstall -g @earendil-works/pi-coding-agent

# pnpm
pnpm remove -g @earendil-works/pi-coding-agent

# Yarn
yarn global remove @earendil-works/pi-coding-agent

# Bun
bun uninstall -g @earendil-works/pi-coding-agent
```

卸载 pi 后，设置、凭据、会话以及已安装的 pi 包仍会保留在 `~/.pi/agent/` 中。

然后在你想让 pi 工作的项目目录中启动它：

```bash
cd /path/to/project
pi
```

## 身份验证

Pi 可以通过 `/login` 使用订阅制提供方，也可以通过环境变量或身份验证文件使用基于 API 密钥的提供方。

### 选项 1：订阅登录

启动 pi 并运行：

```text
/login
```

然后选择一个提供方。内置的订阅登录包括 Claude Pro/Max、ChatGPT Plus/Pro（Codex）和 GitHub Copilot。

### 选项 2：API 密钥

在启动 pi 之前设置 API 密钥：

```bash
export ANTHROPIC_API_KEY=sk-ant-...
pi
```

你也可以运行 `/login` 并选择一个 API 密钥提供方，将密钥存储在 `~/.pi/agent/auth.json` 中。

有关所有受支持的提供方、环境变量和云提供方设置，请参阅[提供方](providers.md)。

## 首次会话

pi 启动后，输入请求并按 Enter：

```text
Summarize this repository and tell me how to run its checks.
```

默认情况下，pi 会为模型提供四个工具：

- `read` - 读取文件
- `write` - 创建或覆盖文件
- `edit` - 修补文件
- `bash` - 运行 shell 命令

其他内置的只读工具（`grep`、`find`、`ls`）可以通过工具选项启用。Pi 在你当前的工作目录中运行，并可以修改该目录下的文件。如果你想方便地回滚更改，请使用 git 或其他检查点（checkpointing）工作流。

## 为 pi 提供项目指令

Pi 会在启动时加载上下文文件。添加一个 `AGENTS.md` 文件来告诉它如何在项目中工作：

```markdown
# Project Instructions

- Run `npm run check` after code changes.
- Do not run production migrations locally.
- Keep responses concise.
```

Pi 会加载：

- `~/.pi/agent/AGENTS.md`，用于全局指令
- 从父目录及当前目录中的 `AGENTS.md` 或 `CLAUDE.md`

更改上下文文件后，请重启 pi，或运行 `/reload`。

## 常见操作

### 引用文件

在编辑器中输入 `@` 以模糊搜索文件，或在命令行中传入文件：

```bash
pi @README.md "Summarize this"
pi @src/app.ts @src/app.test.ts "Review these together"
```

可以使用 Ctrl+V（Windows 上为 Alt+V）粘贴图片，或将图片拖入受支持的终端中。

### 运行 shell 命令

在交互模式下：

```text
!npm run lint
```

命令输出会发送给模型。使用 `!!command` 可以在不将输出添加到模型上下文的情况下运行命令。

### 切换模型

使用 `/model` 或 Ctrl+L 选择模型。使用 Shift+Tab 循环切换思考等级。使用 Ctrl+P / Shift+Ctrl+P 在范围限定的模型间循环切换。

### 稍后继续

会话会自动保存：

```bash
pi -c                  # 继续最近的会话
pi -r                  # 浏览之前的会话
pi --session <path|id> # 打开指定会话
```

在 pi 内部，使用 `/resume`、`/new`、`/tree`、`/fork` 和 `/clone` 来管理会话。

### 非交互模式

用于一次性提示词：

```bash
pi -p "Summarize this codebase"
cat README.md | pi -p "Summarize this text"
pi -p @screenshot.png "What's in this image?"
```

使用 `--mode json` 获取 JSON 事件输出，或使用 `--mode rpc` 进行进程集成。

## 后续步骤

- [使用 Pi](usage.md) - 交互模式、斜杠命令、会话、上下文文件和 CLI 参考。
- [提供方](providers.md) - 身份验证和模型设置。
- [设置](settings.md) - 全局和项目配置。
- [按键绑定](keybindings.md) - 快捷键和自定义。
- [Pi 包](packages.md) - 安装共享的扩展、技能、提示词和主题。

平台说明：[Windows](windows.md)、[Termux](termux.md)、[tmux](tmux.md)、[终端设置](terminal-setup.md)、[Shell 别名](shell-aliases.md)。
