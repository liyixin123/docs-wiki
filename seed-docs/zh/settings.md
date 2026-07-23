# 设置

Pi 使用 JSON 设置文件，项目设置会覆盖全局设置。

| Location | Scope |
|----------|-------|
| `~/.pi/agent/settings.json` | 全局（所有项目） |
| `.pi/settings.json` | 项目（当前目录） |

直接编辑，或使用 `/settings` 来配置常用选项。

## 所有设置

### 模型与思考

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `defaultProvider` | string | - | 默认提供商（例如 `"anthropic"`、`"openai"`） |
| `defaultModel` | string | - | 默认模型 ID |
| `defaultThinkingLevel` | string | - | `"off"`、`"minimal"`、`"low"`、`"medium"`、`"high"`、`"xhigh"` |
| `hideThinkingBlock` | boolean | `false` | 在输出中隐藏思考块 |
| `thinkingBudgets` | object | - | 每个思考级别的自定义 token 预算 |

#### thinkingBudgets

```json
{
  "thinkingBudgets": {
    "minimal": 1024,
    "low": 4096,
    "medium": 10240,
    "high": 32768
  }
}
```

### 界面与显示

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `theme` | string | `"dark"` | 主题名称（`"dark"`、`"light"` 或自定义主题） |
| `quietStartup` | boolean | `false` | 隐藏启动标题 |
| `collapseChangelog` | boolean | `false` | 更新后显示精简版更新日志 |
| `enableInstallTelemetry` | boolean | `true` | 在首次安装或检测到更新日志的更新后，发送一次匿名的安装/更新版本 ping。此设置不控制更新检查 |
| `doubleEscapeAction` | string | `"tree"` | 双击 Escape 的行为：`"tree"`、`"fork"` 或 `"none"` |
| `treeFilterMode` | string | `"default"` | `/tree` 的默认过滤器：`"default"`、`"no-tools"`、`"user-only"`、`"labeled-only"`、`"all"` |
| `editorPaddingX` | number | `0` | 输入编辑器的水平内边距（0-3） |
| `autocompleteMaxVisible` | number | `5` | 自动补全下拉列表中最大可见条目数（3-20） |
| `showHardwareCursor` | boolean | `false` | 显示终端光标 |

### 遥测与更新检查

`enableInstallTelemetry` 只控制发送到 `https://pi.dev/api/report-install` 的匿名安装/更新 ping。选择退出遥测不会禁用更新检查；Pi 仍然可以获取 `https://pi.dev/api/latest-version` 来检查最新版本。

设置 `PI_SKIP_VERSION_CHECK=1` 可以禁用 Pi 的版本更新检查。使用 `--offline` 或 `PI_OFFLINE=1` 可以禁用此处描述的所有启动时网络操作，包括更新检查、包更新检查和安装/更新遥测。

### 警告

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `warnings.anthropicExtraUsage` | boolean | `true` | 当 Anthropic 订阅认证可能产生额外付费用量时显示警告 |

```json
{
  "warnings": {
    "anthropicExtraUsage": false
  }
}
```

### 压缩

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `compaction.enabled` | boolean | `true` | 启用自动压缩 |
| `compaction.reserveTokens` | number | `16384` | 为 LLM 响应保留的 token 数 |
| `compaction.keepRecentTokens` | number | `20000` | 保留的最近 token 数（不会被摘要） |

```json
{
  "compaction": {
    "enabled": true,
    "reserveTokens": 16384,
    "keepRecentTokens": 20000
  }
}
```

### 分支摘要

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `branchSummary.reserveTokens` | number | `16384` | 为分支摘要保留的 token 数 |
| `branchSummary.skipPrompt` | boolean | `false` | 在 `/tree` 导航时跳过“Summarize branch？”提示（默认不生成摘要） |

### 重试

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `retry.enabled` | boolean | `true` | 启用代理级别的瞬态错误自动重试 |
| `retry.maxRetries` | number | `3` | 代理级别的最大重试次数 |
| `retry.baseDelayMs` | number | `2000` | 代理级别指数退避的基础延迟（2 秒、4 秒、8 秒） |
| `retry.provider.timeoutMs` | number | SDK default | 提供商/SDK 请求超时时间（毫秒） |
| `retry.provider.maxRetries` | number | `0` | 提供商/SDK 重试次数 |
| `retry.provider.maxRetryDelayMs` | number | `60000` | 失败前允许的最大服务器请求延迟（60 秒） |

当提供商请求的重试延迟超过 `retry.provider.maxRetryDelayMs`（例如 Google 提示的“配额将在 5 小时后重置”）时，请求会立即失败并返回相应错误信息，而不是静默等待。设置为 `0` 可以取消该上限。

除非明确需要提供商级别的重试，否则请将 `retry.provider.maxRetries` 保持为 `0`。将其设置为大于 `0` 的值可能会导致 SDK/提供商重试在 Pi 看到超出使用限制的错误之前先行处理它们，这在某些情况下可能会阻塞代理，直到提供商配额重置。

```json
{
  "retry": {
    "enabled": true,
    "maxRetries": 3,
    "baseDelayMs": 2000,
    "provider": {
      "timeoutMs": 3600000,
      "maxRetries": 0,
      "maxRetryDelayMs": 60000
    }
  }
}
```

### 消息投递

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `steeringMode` | string | `"one-at-a-time"` | 引导消息的发送方式：`"all"` 或 `"one-at-a-time"` |
| `followUpMode` | string | `"one-at-a-time"` | 后续消息的发送方式：`"all"` 或 `"one-at-a-time"` |
| `transport` | string | `"auto"` | 支持多种传输方式的提供商所使用的首选传输方式：`"sse"`、`"websocket"`、`"websocket-cached"` 或 `"auto"` |
| `httpIdleTimeoutMs` | number | `300000` | HTTP 头/正文空闲超时时间（毫秒），也用于具有显式流空闲超时设置的提供商。设置为 `0` 可禁用。 |
| `websocketConnectTimeoutMs` | number | `15000` | 支持 WebSocket 传输的提供商的 WebSocket 连接/握手超时时间（毫秒）。设置为 `0` 可禁用。 |

### 终端与图像

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `terminal.showImages` | boolean | `true` | 在终端中显示图像（如果支持） |
| `terminal.imageWidthCells` | number | `60` | 终端中内联图像的首选宽度（以单元格计） |
| `terminal.clearOnShrink` | boolean | `false` | 内容收缩时清空空白行（可能导致闪烁） |
| `images.autoResize` | boolean | `true` | 将图像调整为最大 2000x2000 |
| `images.blockImages` | boolean | `false` | 阻止所有图像发送给 LLM |

### Shell

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `shellPath` | string | - | 自定义 shell 路径（例如 Windows 上的 Cygwin） |
| `shellCommandPrefix` | string | - | 每个 bash 命令的前缀（例如 `"shopt -s expand_aliases"`） |
| `npmCommand` | string[] | - | 用于 npm 包查找/安装操作的命令 argv（例如 `["mise", "exec", "node@20", "--", "npm"]`） |

```json
{
  "npmCommand": ["mise", "exec", "node@20", "--", "npm"]
}
```

`npmCommand` 用于所有 npm 包管理器操作，包括安装、卸载以及 git 包内的依赖安装。用户范围的 npm 包安装在 `~/.pi/agent/npm/` 下；项目范围的 npm 包安装在 `.pi/npm/` 下。请使用 argv 风格的条目，与进程实际启动方式完全一致。配置了 `npmCommand` 后，git 包依赖安装会使用普通的 `install`，以避免在包装器或替代包管理器中出现 npm 专用标志。

### 会话

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `sessionDir` | string | - | 存储会话文件的目录。支持绝对路径、相对路径以及 `~`。 |

```json
{ "sessionDir": ".pi/sessions" }
```

当多个来源指定了会话目录时，优先级为 `--session-dir`、`PI_CODING_AGENT_SESSION_DIR`，然后是 settings.json 中的 `sessionDir`。

### 模型循环切换

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `enabledModels` | string[] | - | 用于 Ctrl+P 循环切换的模型模式（格式与 `--models` CLI 标志相同） |

```json
{
  "enabledModels": ["claude-*", "gpt-4o", "gemini-2*"]
}
```

### Markdown

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `markdown.codeBlockIndent` | string | `"  "` | 代码块的缩进 |

### 资源

这些设置定义了从何处加载扩展、技能、提示和主题。

`~/.pi/agent/settings.json` 中的路径相对于 `~/.pi/agent` 解析。`.pi/settings.json` 中的路径相对于 `.pi` 解析。支持绝对路径和 `~`。

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `packages` | array | `[]` | 用于加载资源的 npm/git 包 |
| `extensions` | string[] | `[]` | 本地扩展文件路径或目录 |
| `skills` | string[] | `[]` | 本地技能文件路径或目录 |
| `prompts` | string[] | `[]` | 本地提示模板路径或目录 |
| `themes` | string[] | `[]` | 本地主题文件路径或目录 |
| `enableSkillCommands` | boolean | `true` | 将技能注册为 `/skill:name` 命令 |

数组支持 glob 模式和排除项。使用 `!pattern` 进行排除。使用 `+path` 强制包含某个精确路径，使用 `-path` 强制排除某个精确路径。

#### packages

字符串形式会加载某个包中的所有资源：

```json
{
  "packages": ["pi-skills", "@org/my-extension"]
}
```

对象形式可以过滤要加载哪些资源：

```json
{
  "packages": [
    {
      "source": "pi-skills",
      "skills": ["brave-search", "transcribe"],
      "extensions": []
    }
  ]
}
```

有关包管理的详细信息，请参见 [packages.md](packages.md)。

## 示例

```json
{
  "defaultProvider": "anthropic",
  "defaultModel": "claude-sonnet-4-20250514",
  "defaultThinkingLevel": "medium",
  "theme": "dark",
  "compaction": {
    "enabled": true,
    "reserveTokens": 16384,
    "keepRecentTokens": 20000
  },
  "retry": {
    "enabled": true,
    "maxRetries": 3
  },
  "enabledModels": ["claude-*", "gpt-4o"],
  "warnings": {
    "anthropicExtraUsage": true
  },
  "packages": ["pi-skills"]
}
```

## 项目覆盖

项目设置（`.pi/settings.json`）会覆盖全局设置。嵌套对象会被合并：

```json
// ~/.pi/agent/settings.json (global)
{
  "theme": "dark",
  "compaction": { "enabled": true, "reserveTokens": 16384 }
}

// .pi/settings.json (project)
{
  "compaction": { "reserveTokens": 8192 }
}

// Result
{
  "theme": "dark",
  "compaction": { "enabled": true, "reserveTokens": 8192 }
}
```
