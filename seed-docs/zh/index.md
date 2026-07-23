# Pi 文档

Pi 是一个极简的终端编码代理（coding harness）。它的设计目标是保持核心精简，同时通过 TypeScript 扩展、技能（skills）、提示词模板、主题和 pi 包（packages）进行扩展。

## 快速开始

使用 npm 安装 Pi：

```bash
npm install -g --ignore-scripts @earendil-works/pi-coding-agent
```

`--ignore-scripts` 会在安装期间禁用依赖生命周期脚本。正常的 npm 安装并不需要 Pi 的安装脚本。

在 Linux 或 macOS 上，你也可以使用安装脚本：

```bash
curl -fsSL https://pi.dev/install.sh | sh
```

要卸载 pi 本身，无论是通过 curl 还是 npm 安装的，都使用 npm 卸载：

```bash
npm uninstall -g @earendil-works/pi-coding-agent
```

对于 pnpm、Yarn 或 Bun 安装的情况，请使用相应的全局移除命令：`pnpm remove -g @earendil-works/pi-coding-agent`、`yarn global remove @earendil-works/pi-coding-agent`，或 `bun uninstall -g @earendil-works/pi-coding-agent`。

然后在项目目录中运行它：

```bash
pi
```

对于订阅制提供方，使用 `/login` 进行身份验证；对于其他方式，可在启动 pi 之前设置 API 密钥，例如 `ANTHROPIC_API_KEY`。

关于完整的首次运行流程，请参阅[快速开始](quickstart.md)。

## 从这里开始

- [快速开始](quickstart.md) - 安装、身份验证并运行第一个会话。
- [使用 Pi](usage.md) - 交互模式、斜杠命令、上下文文件和 CLI 参考。
- [提供方](providers.md) - 内置提供方的订阅和 API 密钥设置。
- [llama.cpp](llama-cpp.md) - 运行本地路由器并通过 `/llama` 管理模型。
- [安全](security.md) - 项目信任、沙箱边界和漏洞报告。
- [容器化](containerization.md) - 使用 Gondolin、Docker 或 OpenShell 对 pi 进行沙箱化。
- [设置](settings.md) - 全局和项目设置。
- [按键绑定](keybindings.md) - 默认快捷键和自定义按键绑定。
- [会话](sessions.md) - 会话管理、分支和树形导航。
- [压缩](compaction.md) - 上下文压缩和分支摘要。

## 定制化

- [扩展](extensions.md) - 用于工具、命令、事件和自定义 UI 的 TypeScript 模块。
- [技能](skills.md) - 可复用的按需能力（Agent Skills）。
- [提示词模板](prompt-templates.md) - 通过斜杠命令展开的可复用提示词。
- [主题](themes.md) - 内置和自定义终端主题。
- [Pi 包](packages.md) - 打包并共享扩展、技能、提示词和主题。
- [自定义模型](models.md) - 为受支持的提供方 API 添加模型条目。
- [自定义提供方](custom-provider.md) - 实现自定义 API 和 OAuth 流程。

## 编程化使用

- [SDK](sdk.md) - 在 Node.js 应用中嵌入 pi。
- [RPC 模式](rpc.md) - 通过 stdin/stdout JSONL 进行集成。
- [JSON 事件流模式](json.md) - 带结构化事件的打印模式。
- [TUI 组件](tui.md) - 为扩展构建自定义终端 UI。

## 参考

- [会话格式](session-format.md) - JSONL 会话文件格式、条目类型和 SessionManager API。

## 平台设置

- [Windows](windows.md)
- [Android 上的 Termux](termux.md)
- [tmux](tmux.md)
- [终端设置](terminal-setup.md)
- [Shell 别名](shell-aliases.md)

## 开发

- [开发](development.md) - 本地环境搭建、项目结构和调试。
