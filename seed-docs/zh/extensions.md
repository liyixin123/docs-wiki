> pi 可以创建扩展。让 pi 为你的用例构建一个扩展吧。

# 扩展(Extensions)

扩展是用于扩展 pi 行为的 TypeScript 模块。它们可以订阅生命周期事件、注册可被 LLM 调用的自定义工具、添加命令等等。

> **`/reload` 的存放位置:** 将扩展放在 `~/.pi/agent/extensions/`(全局)或 `.pi/extensions/`(项目本地)以便自动发现。`pi -e ./path.ts` 仅用于快速测试。位于自动发现路径下的扩展可以通过 `/reload` 进行热重载。

**关键能力:**
- **自定义工具** - 通过 `pi.registerTool()` 注册 LLM 可调用的工具
- **事件拦截** - 阻止或修改工具调用、注入上下文、自定义压缩逻辑
- **用户交互** - 通过 `ctx.ui`(select、confirm、input、notify)提示用户
- **自定义 UI 组件** - 通过 `ctx.ui.custom()` 使用带键盘输入的完整 TUI 组件,实现复杂交互
- **自定义命令** - 通过 `pi.registerCommand()` 注册类似 `/mycommand` 的命令
- **会话持久化** - 通过 `pi.appendEntry()` 存储可在重启后保留的状态
- **自定义渲染** - 控制工具调用/结果以及消息在 TUI 中的呈现方式

**示例用例:**
- 权限门控(在执行 `rm -rf`、`sudo` 等命令前进行确认)
- Git 检查点(每一轮暂存(stash),在分支切换时恢复)
- 路径保护(阻止对 `.env`、`node_modules/` 的写入)
- 自定义压缩(按你自己的方式总结对话)
- 对话摘要(参见 `summarize.ts` 示例)
- 交互式工具(提问、向导、自定义对话框)
- 有状态工具(待办列表、连接池)
- 外部集成(文件监听、webhook、CI 触发器)
- 等待时的小游戏(参见 `snake.ts` 示例)

参见 [examples/extensions/](../examples/extensions/) 获取可运行的实现示例。

## 目录

- [快速开始](#quick-start)
- [扩展的存放位置](#extension-locations)
- [可用的导入](#available-imports)
- [编写扩展](#writing-an-extension)
  - [扩展的组织方式](#extension-styles)
- [事件](#events)
  - [生命周期概览](#lifecycle-overview)
  - [资源事件](#resource-events)
  - [会话事件](#session-events)
  - [Agent 事件](#agent-events)
  - [模型事件](#model-events)
  - [工具事件](#tool-events)
- [ExtensionContext](#extensioncontext)
- [ExtensionCommandContext](#extensioncommandcontext)
- [ExtensionAPI 方法](#extensionapi-methods)
- [状态管理](#state-management)
- [自定义工具](#custom-tools)
- [自定义 UI](#custom-ui)
- [错误处理](#error-handling)
- [模式行为](#mode-behavior)
- [示例参考](#examples-reference)

## 快速开始

创建 `~/.pi/agent/extensions/my-extension.ts`:

```typescript
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";

export default function (pi: ExtensionAPI) {
  // React to events
  pi.on("session_start", async (_event, ctx) => {
    ctx.ui.notify("Extension loaded!", "info");
  });

  pi.on("tool_call", async (event, ctx) => {
    if (event.toolName === "bash" && event.input.command?.includes("rm -rf")) {
      const ok = await ctx.ui.confirm("Dangerous!", "Allow rm -rf?");
      if (!ok) return { block: true, reason: "Blocked by user" };
    }
  });

  // Register a custom tool
  pi.registerTool({
    name: "greet",
    label: "Greet",
    description: "Greet someone by name",
    parameters: Type.Object({
      name: Type.String({ description: "Name to greet" }),
    }),
    async execute(toolCallId, params, signal, onUpdate, ctx) {
      return {
        content: [{ type: "text", text: `Hello, ${params.name}!` }],
        details: {},
      };
    },
  });

  // Register a command
  pi.registerCommand("hello", {
    description: "Say hello",
    handler: async (args, ctx) => {
      ctx.ui.notify(`Hello ${args || "world"}!`, "info");
    },
  });
}
```

使用 `--extension`(或 `-e`)参数进行测试:

```bash
pi -e ./my-extension.ts
```

## 扩展的存放位置

> **安全提示:** 扩展会以你的完整系统权限运行,并可以执行任意代码。只安装来自可信来源的扩展。

扩展会从以下位置自动发现:

| 位置 | 作用范围 |
|----------|-------|
| `~/.pi/agent/extensions/*.ts` | 全局(所有项目) |
| `~/.pi/agent/extensions/*/index.ts` | 全局(子目录形式) |
| `.pi/extensions/*.ts` | 项目本地 |
| `.pi/extensions/*/index.ts` | 项目本地(子目录形式) |

也可以通过 `settings.json` 添加额外路径:

```json
{
  "packages": [
    "npm:@foo/bar@1.0.0",
    "git:github.com/user/repo@v1"
  ],
  "extensions": [
    "/path/to/local/extension.ts",
    "/path/to/local/extension/dir"
  ]
}
```

要将扩展作为 pi 包通过 npm 或 git 分享,请参见 [packages.md](packages.md)。

## 可用的导入

| 包 | 用途 |
|---------|-------|
| `@earendil-works/pi-coding-agent` | 扩展相关类型(`ExtensionAPI`、`ExtensionContext`、事件) |
| `typebox` | 用于工具参数的 schema 定义 |
| `@earendil-works/pi-ai` | AI 相关工具函数(`StringEnum` 用于兼容 Google 的枚举) |
| `@earendil-works/pi-tui` | 用于自定义渲染的 TUI 组件 |

npm 依赖同样可用。在你的扩展旁边(或上层目录)添加一个 `package.json`,运行 `npm install`,`node_modules/` 中的导入就会被自动解析。

对于通过 `pi install`(npm 或 git)安装的已分发 pi 包,运行期依赖必须放在 `dependencies` 中。包安装默认使用生产安装方式(`npm install --omit=dev`),因此运行时无法使用 `devDependencies`;当配置了 `npmCommand` 时,git 包会使用普通的 `install` 以兼容各种包装器(wrapper)。

Node.js 内置模块(`node:fs`、`node:path` 等)同样可用。

## 编写扩展

一个扩展会导出一个默认的工厂函数,该函数接收 `ExtensionAPI`。该工厂函数可以是同步的,也可以是异步的:

```typescript
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

export default function (pi: ExtensionAPI) {
  // Subscribe to events
  pi.on("event_name", async (event, ctx) => {
    // ctx.ui for user interaction
    const ok = await ctx.ui.confirm("Title", "Are you sure?");
    ctx.ui.notify("Done!", "info");
    ctx.ui.setStatus("my-ext", "Processing...");  // Footer status
    ctx.ui.setWidget("my-ext", ["Line 1", "Line 2"]);  // Widget above editor (default)
  });

  // Register tools, commands, shortcuts, flags
  pi.registerTool({ ... });
  pi.registerCommand("name", { ... });
  pi.registerShortcut("ctrl+x", { ... });
  pi.registerFlag("my-flag", { ... });
}
```

扩展通过 [jiti](https://github.com/unjs/jiti) 加载,因此无需编译即可直接使用 TypeScript。

如果工厂函数返回一个 `Promise`,pi 会在继续启动之前等待它完成。这意味着异步初始化会在 `session_start` 之前、`resources_discover` 之前,以及通过 `pi.registerProvider()` 排队的 provider 注册被刷新(flush)之前完成。

### 异步工厂函数

对于一次性的启动工作(例如获取远程配置或动态发现可用模型),请使用异步工厂函数。

```typescript
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

export default async function (pi: ExtensionAPI) {
  const response = await fetch("http://localhost:1234/v1/models");
  const payload = (await response.json()) as {
    data: Array<{
      id: string;
      name?: string;
      context_window?: number;
      max_tokens?: number;
    }>;
  };

  pi.registerProvider("local-openai", {
    baseUrl: "http://localhost:1234/v1",
    apiKey: "LOCAL_OPENAI_API_KEY",
    api: "openai-completions",
    models: payload.data.map((model) => ({
      id: model.id,
      name: model.name ?? model.id,
      reasoning: false,
      input: ["text"],
      cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
      contextWindow: model.context_window ?? 128000,
      maxTokens: model.max_tokens ?? 4096,
    })),
  });
}
```

这种模式使获取到的模型在正常启动过程中以及 `pi --list-models` 中都可用。

### 扩展的组织方式

**单文件** - 最简单,适合小型扩展:

```
~/.pi/agent/extensions/
└── my-extension.ts
```

**带 index.ts 的目录** - 适合多文件扩展:

```
~/.pi/agent/extensions/
└── my-extension/
    ├── index.ts        # Entry point (exports default function)
    ├── tools.ts        # Helper module
    └── utils.ts        # Helper module
```

**带依赖的包** - 适合需要 npm 包的扩展:

```
~/.pi/agent/extensions/
└── my-extension/
    ├── package.json    # Declares dependencies and entry points
    ├── package-lock.json
    ├── node_modules/   # After npm install
    └── src/
        └── index.ts
```

```json
// package.json
{
  "name": "my-extension",
  "dependencies": {
    "zod": "^3.0.0",
    "chalk": "^5.0.0"
  },
  "pi": {
    "extensions": ["./src/index.ts"]
  }
}
```

在扩展目录中运行 `npm install`,之后 `node_modules/` 中的导入就会自动生效。

## 事件

### 生命周期概览

```
pi starts
  │
  ├─► session_start { reason: "startup" }
  └─► resources_discover { reason: "startup" }
      │
      ▼
user sends prompt ─────────────────────────────────────────┐
  │                                                        │
  ├─► (extension commands checked first, bypass if found)  │
  ├─► input (can intercept, transform, or handle)          │
  ├─► (skill/template expansion if not handled)            │
  ├─► before_agent_start (can inject message, modify system prompt)
  ├─► agent_start                                          │
  ├─► message_start / message_update / message_end         │
  │                                                        │
  │   ┌─── turn (repeats while LLM calls tools) ───┐       │
  │   │                                            │       │
  │   ├─► turn_start                               │       │
  │   ├─► context (can modify messages)            │       │
  │   ├─► before_provider_request (can inspect or replace payload)
  │   ├─► after_provider_response (status + headers, before stream consume)
  │   │                                            │       │
  │   │   LLM responds, may call tools:            │       │
  │   │     ├─► tool_execution_start               │       │
  │   │     ├─► tool_call (can block)               │       │
  │   │     ├─► tool_execution_update               │       │
  │   │     ├─► tool_result (can modify)            │       │
  │   │     └─► tool_execution_end                  │       │
  │   │                                            │       │
  │   └─► turn_end                                 │       │
  │                                                        │
  └─► agent_end                                            │
                                                           │
user sends another prompt ◄────────────────────────────────┘

/new (new session) or /resume (switch session)
  ├─► session_before_switch (can cancel)
  ├─► session_shutdown
  ├─► session_start { reason: "new" | "resume", previousSessionFile? }
  └─► resources_discover { reason: "startup" }

/fork or /clone
  ├─► session_before_fork (can cancel)
  ├─► session_shutdown
  ├─► session_start { reason: "fork", previousSessionFile }
  └─► resources_discover { reason: "startup" }

/compact or auto-compaction
  ├─► session_before_compact (can cancel or customize)
  └─► session_compact

/tree navigation
  ├─► session_before_tree (can cancel or customize)
  └─► session_tree

/model or Ctrl+P (model selection/cycling)
  ├─► thinking_level_select (if model change changes/clamps thinking level)
  └─► model_select

thinking level changes (settings, keybinding, pi.setThinkingLevel())
  └─► thinking_level_select

exit (Ctrl+C, Ctrl+D, SIGHUP, SIGTERM)
  └─► session_shutdown
```

### 资源事件

#### resources_discover

在 `session_start` 之后触发,以便扩展可以贡献额外的 skill、prompt 和主题(theme)路径。
启动路径使用 `reason: "startup"`。重新加载使用 `reason: "reload"`。

```typescript
pi.on("resources_discover", async (event, _ctx) => {
  // event.cwd - current working directory
  // event.reason - "startup" | "reload"
  return {
    skillPaths: ["/path/to/skills"],
    promptPaths: ["/path/to/prompts"],
    themePaths: ["/path/to/themes"],
  };
});
```

### 会话事件

关于会话存储的内部机制以及 SessionManager API,请参见 [Session Format](session-format.md)。

#### session_start

在会话启动、加载或重新加载时触发。

```typescript
pi.on("session_start", async (event, ctx) => {
  // event.reason - "startup" | "reload" | "new" | "resume" | "fork"
  // event.previousSessionFile - present for "new", "resume", and "fork"
  ctx.ui.notify(`Session: ${ctx.sessionManager.getSessionFile() ?? "ephemeral"}`, "info");
});
```

#### session_before_switch

在开始新会话(`/new`)或切换会话(`/resume`)之前触发。

```typescript
pi.on("session_before_switch", async (event, ctx) => {
  // event.reason - "new" or "resume"
  // event.targetSessionFile - session we're switching to (only for "resume")

  if (event.reason === "new") {
    const ok = await ctx.ui.confirm("Clear?", "Delete all messages?");
    if (!ok) return { cancel: true };
  }
});
```

切换或新建会话成功后,pi 会为旧的扩展实例触发 `session_shutdown`,重新加载并为新会话重新绑定扩展,然后携带 `reason: "new" | "resume"` 和 `previousSessionFile` 触发 `session_start`。
请在 `session_shutdown` 中做清理工作,然后在 `session_start` 中重新建立内存状态。

#### session_before_fork

在通过 `/fork` 分叉或通过 `/clone` 克隆时触发。

```typescript
pi.on("session_before_fork", async (event, ctx) => {
  // event.entryId - ID of the selected entry
  // event.position - "before" for /fork, "at" for /clone
  return { cancel: true }; // Cancel fork/clone
  // OR
  return { skipConversationRestore: true }; // Reserved for future conversation restore control
});
```

分叉或克隆成功后,pi 会为旧的扩展实例触发 `session_shutdown`,重新加载并为新会话重新绑定扩展,然后携带 `reason: "fork"` 和 `previousSessionFile` 触发 `session_start`。
请在 `session_shutdown` 中做清理工作,然后在 `session_start` 中重新建立内存状态。

#### session_before_compact / session_compact

在压缩(compaction)时触发。详情参见 [compaction.md](compaction.md)。

```typescript
pi.on("session_before_compact", async (event, ctx) => {
  const { preparation, branchEntries, customInstructions, signal } = event;

  // Cancel:
  return { cancel: true };

  // Custom summary:
  return {
    compaction: {
      summary: "...",
      firstKeptEntryId: preparation.firstKeptEntryId,
      tokensBefore: preparation.tokensBefore,
    }
  };
});

pi.on("session_compact", async (event, ctx) => {
  // event.compactionEntry - the saved compaction
  // event.fromExtension - whether extension provided it
});
```

#### session_before_tree / session_tree

在 `/tree` 导航时触发。树导航的相关概念参见 [Sessions](sessions.md)。

```typescript
pi.on("session_before_tree", async (event, ctx) => {
  const { preparation, signal } = event;
  return { cancel: true };
  // OR provide custom summary:
  return { summary: { summary: "...", details: {} } };
});

pi.on("session_tree", async (event, ctx) => {
  // event.newLeafId, oldLeafId, summaryEntry, fromExtension
});
```

#### session_shutdown

在扩展运行时被销毁之前触发。

```typescript
pi.on("session_shutdown", async (event, ctx) => {
  // event.reason - "quit" | "reload" | "new" | "resume" | "fork"
  // event.targetSessionFile - destination session for session replacement flows
  // Cleanup, save state, etc.
});
```

### Agent 事件

#### before_agent_start

在用户提交 prompt 之后、agent 循环开始之前触发。可以注入一条消息和/或修改系统提示词。

```typescript
pi.on("before_agent_start", async (event, ctx) => {
  // event.prompt - user's prompt text
  // event.images - attached images (if any)
  // event.systemPrompt - current chained system prompt for this handler
  //   (includes changes from earlier before_agent_start handlers)
  // event.systemPromptOptions - structured options used to build the system prompt
  //   .customPrompt - any custom system prompt (from --system-prompt, SYSTEM.md, or custom templates)
  //   .selectedTools - tools currently active in the prompt
  //   .toolSnippets - one-line descriptions for each tool
  //   .promptGuidelines - custom guideline bullets
  //   .appendSystemPrompt - text from --append-system-prompt flags
  //   .cwd - working directory
  //   .contextFiles - AGENTS.md files and other loaded context files
  //   .skills - loaded skills

  return {
    // Inject a persistent message (stored in session, sent to LLM)
    message: {
      customType: "my-extension",
      content: "Additional context for the LLM",
      display: true,
    },
    // Replace the system prompt for this turn (chained across extensions)
    systemPrompt: event.systemPrompt + "\n\nExtra instructions for this turn...",
  };
});
```

`systemPromptOptions` 字段让扩展能够访问 Pi 用于构建系统提示词的同一份结构化数据。这让你可以查看 Pi 已加载的内容——自定义提示词、指导原则、工具摘要、上下文文件、skill——而无需重新发现资源或重新解析各种参数。当你的扩展需要在尊重用户已有配置的前提下,对系统提示词进行深入、明智的修改时,请使用它。

在 `before_agent_start` 内部,`event.systemPrompt` 和 `ctx.getSystemPrompt()` 都反映了截至当前处理程序为止已被串联(chained)修改的系统提示词。之后的 `before_agent_start` 处理程序仍然可以继续修改它。

#### agent_start / agent_end

每个用户 prompt 触发一次。

```typescript
pi.on("agent_start", async (_event, ctx) => {});

pi.on("agent_end", async (event, ctx) => {
  // event.messages - messages from this prompt
});
```

#### turn_start / turn_end

每一轮(一次 LLM 响应加上其工具调用)触发一次。

```typescript
pi.on("turn_start", async (event, ctx) => {
  // event.turnIndex, event.timestamp
});

pi.on("turn_end", async (event, ctx) => {
  // event.turnIndex, event.message, event.toolResults
});
```

#### message_start / message_update / message_end

在消息生命周期更新时触发。

- `message_start` 和 `message_end` 会针对 user、assistant 和 toolResult 消息触发。
- `message_update` 会针对 assistant 的流式更新触发。
- `message_end` 处理程序可以返回 `{ message }` 来替换最终确定的消息。替换后的消息必须保持相同的 `role`。

```typescript
pi.on("message_start", async (event, ctx) => {
  // event.message
});

pi.on("message_update", async (event, ctx) => {
  // event.message
  // event.assistantMessageEvent (token-by-token stream event)
});

pi.on("message_end", async (event, ctx) => {
  if (event.message.role !== "assistant") return;

  return {
    message: {
      ...event.message,
      usage: {
        ...event.message.usage,
        cost: {
          ...event.message.usage.cost,
          total: 0.123,
        },
      },
    },
  };
});
```

#### tool_execution_start / tool_execution_update / tool_execution_end

在工具执行生命周期更新时触发。

在并行工具模式下:
- `tool_execution_start` 会在预检(preflight)阶段按 assistant 消息中的源顺序发出
- `tool_execution_update` 事件可能会在多个工具之间交错发生
- `tool_execution_end` 会在每个工具最终确定后,按工具完成的顺序发出
- 最终的 `toolResult` 消息事件仍然会稍后按 assistant 消息中的源顺序发出

```typescript
pi.on("tool_execution_start", async (event, ctx) => {
  // event.toolCallId, event.toolName, event.args
});

pi.on("tool_execution_update", async (event, ctx) => {
  // event.toolCallId, event.toolName, event.args, event.partialResult
});

pi.on("tool_execution_end", async (event, ctx) => {
  // event.toolCallId, event.toolName, event.result, event.isError
});
```

#### context

在每次 LLM 调用之前触发。可以非破坏性地修改消息。消息类型详情参见 [Session Format](session-format.md)。

```typescript
pi.on("context", async (event, ctx) => {
  // event.messages - deep copy, safe to modify
  const filtered = event.messages.filter(m => !shouldPrune(m));
  return { messages: filtered };
});
```

#### before_provider_request

在构建好特定于 provider 的负载(payload)之后、发送请求之前触发。处理程序按扩展的加载顺序运行。返回 `undefined` 会保持负载不变。返回其他任何值都会替换后续处理程序以及实际请求所使用的负载。

这个钩子可以重写或彻底移除 provider 级别的系统指令(system instructions)。这些负载层面的更改不会体现在 `ctx.getSystemPrompt()` 中,因为它报告的是 Pi 的系统提示词字符串,而不是最终序列化后的 provider 负载。

```typescript
pi.on("before_provider_request", (event, ctx) => {
  console.log(JSON.stringify(event.payload, null, 2));

  // Optional: replace payload
  // return { ...event.payload, temperature: 0 };
});
```

这主要用于调试 provider 的序列化过程和缓存行为。

#### after_provider_response

在收到 HTTP 响应之后、消费其流式响应体之前触发。处理程序按扩展的加载顺序运行。

```typescript
pi.on("after_provider_response", (event, ctx) => {
  // event.status - HTTP status code
  // event.headers - normalized response headers
  if (event.status === 429) {
    console.log("rate limited", event.headers["retry-after"]);
  }
});
```

请求头的可用性取决于 provider 和传输方式。抽象了 HTTP 响应的 provider 可能不会暴露请求头。

### 模型事件

#### model_select

在模型通过 `/model` 命令、模型循环切换(`Ctrl+P`)或会话恢复而发生变化时触发。

```typescript
pi.on("model_select", async (event, ctx) => {
  // event.model - newly selected model
  // event.previousModel - previous model (undefined if first selection)
  // event.source - "set" | "cycle" | "restore"

  const prev = event.previousModel
    ? `${event.previousModel.provider}/${event.previousModel.id}`
    : "none";
  const next = `${event.model.provider}/${event.model.id}`;

  ctx.ui.notify(`Model changed (${event.source}): ${prev} -> ${next}`, "info");
});
```

可以用它来更新 UI 元素(状态栏、footer),或在活动模型发生变化时执行模型特定的初始化操作。

#### thinking_level_select

在思维等级(thinking level)发生变化时触发。这是一个仅通知性质的事件;处理程序的返回值会被忽略。

```typescript
pi.on("thinking_level_select", async (event, ctx) => {
  // event.level - newly selected thinking level
  // event.previousLevel - previous thinking level

  ctx.ui.setStatus("thinking", `thinking: ${event.level}`);
});
```

当 `pi.setThinkingLevel()`、模型切换或内置的思维等级控件改变了当前活动的思维等级时,可用它来更新扩展的 UI。

### 工具事件

#### tool_call

在 `tool_execution_start` 之后、工具执行之前触发。**可以阻止执行。** 使用 `isToolCallEventType` 进行类型收窄并获得带类型的输入。

在 `tool_call` 运行之前,pi 会等待此前发出的 Agent 事件通过 `AgentSession` 完全处理完毕。这意味着 `ctx.sessionManager` 会更新到当前这条 assistant 工具调用消息为止的最新状态。

在默认的并行工具执行模式下,来自同一条 assistant 消息的兄弟工具调用会先依次完成预检,然后并发执行。`tool_call` 并不保证能在 `ctx.sessionManager` 中看到同一条 assistant 消息中兄弟工具调用的结果。

`event.input` 是可变的。可以原地修改它来在执行前修补工具参数。

行为保证:
- 对 `event.input` 的修改会影响实际的工具执行
- 后触发的 `tool_call` 处理程序能看到之前处理程序所做的修改
- 修改之后不会重新进行校验(validation)
- `tool_call` 的返回值只能通过 `{ block: true, reason?: string }` 来控制是否阻止执行

```typescript
import { isToolCallEventType } from "@earendil-works/pi-coding-agent";

pi.on("tool_call", async (event, ctx) => {
  // event.toolName - "bash", "read", "write", "edit", etc.
  // event.toolCallId
  // event.input - tool parameters (mutable)

  // Built-in tools: no type params needed
  if (isToolCallEventType("bash", event)) {
    // event.input is { command: string; timeout?: number }
    event.input.command = `source ~/.profile\n${event.input.command}`;

    if (event.input.command.includes("rm -rf")) {
      return { block: true, reason: "Dangerous command" };
    }
  }

  if (isToolCallEventType("read", event)) {
    // event.input is { path: string; offset?: number; limit?: number }
    console.log(`Reading: ${event.input.path}`);
  }
});
```

#### 为自定义工具输入添加类型

自定义工具应该导出自己的输入类型:

```typescript
// my-extension.ts
export type MyToolInput = Static<typeof myToolSchema>;
```

使用带有显式类型参数的 `isToolCallEventType`:

```typescript
import { isToolCallEventType } from "@earendil-works/pi-coding-agent";
import type { MyToolInput } from "my-extension";

pi.on("tool_call", (event) => {
  if (isToolCallEventType<"my_tool", MyToolInput>("my_tool", event)) {
    event.input.action;  // typed
  }
});
```

#### tool_result

在工具执行完成之后、`tool_execution_end` 以及最终的工具结果消息事件发出之前触发。**可以修改结果。**

在并行工具模式下,`tool_result` 和 `tool_execution_end` 可能按工具完成的顺序交错发生,而最终的 `toolResult` 消息事件仍然会稍后按 assistant 消息中的源顺序发出。

`tool_result` 处理程序像中间件一样链式运行:
- 处理程序按扩展的加载顺序运行
- 每个处理程序看到的都是前一个处理程序修改后的最新结果
- 处理程序可以返回部分补丁(`content`、`details` 或 `isError`);省略的字段会保留其当前值

在处理程序内部的嵌套异步工作中使用 `ctx.signal`。这样可以让 Esc 键取消由该扩展发起的模型调用、`fetch()` 以及其他支持中止(abort-aware)的操作。

```typescript
import { isBashToolResult } from "@earendil-works/pi-coding-agent";

pi.on("tool_result", async (event, ctx) => {
  // event.toolName, event.toolCallId, event.input
  // event.content, event.details, event.isError

  if (isBashToolResult(event)) {
    // event.details is typed as BashToolDetails
  }

  const response = await fetch("https://example.com/summarize", {
    method: "POST",
    body: JSON.stringify({ content: event.content }),
    signal: ctx.signal,
  });

  // Modify result:
  return { content: [...], details: {...}, isError: false };
});
```

### 用户 Bash 事件

#### user_bash

在用户执行 `!` 或 `!!` 命令时触发。**可以拦截。**

```typescript
import { createLocalBashOperations } from "@earendil-works/pi-coding-agent";

pi.on("user_bash", (event, ctx) => {
  // event.command - the bash command
  // event.excludeFromContext - true if !! prefix
  // event.cwd - working directory

  // Option 1: Provide custom operations (e.g., SSH)
  return { operations: remoteBashOps };

  // Option 2: Wrap pi's built-in local bash backend
  const local = createLocalBashOperations();
  return {
    operations: {
      exec(command, cwd, options) {
        return local.exec(`source ~/.profile\n${command}`, cwd, options);
      }
    }
  };

  // Option 3: Full replacement - return result directly
  return { result: { output: "...", exitCode: 0, cancelled: false, truncated: false } };
});
```

### 输入事件

#### input

在收到用户输入时触发,发生在扩展命令检查之后、skill 与模板(template)展开之前。该事件看到的是原始输入文本,因此 `/skill:foo` 和 `/template` 此时尚未展开。

**处理顺序:**
1. 首先检查扩展命令(`/cmd`) - 如果匹配到,则运行对应处理程序,并跳过 input 事件
2. 触发 `input` 事件 - 可以拦截、转换或处理该输入
3. 若未被处理:skill 命令(`/skill:name`)会被展开为 skill 内容
4. 若未被处理:prompt 模板(`/template`)会被展开为模板内容
5. 开始 agent 处理流程(`before_agent_start` 等)

```typescript
pi.on("input", async (event, ctx) => {
  // event.text - raw input (before skill/template expansion)
  // event.images - attached images, if any
  // event.source - "interactive" (typed), "rpc" (API), or "extension" (via sendUserMessage)

  // Transform: rewrite input before expansion
  if (event.text.startsWith("?quick "))
    return { action: "transform", text: `Respond briefly: ${event.text.slice(7)}` };

  // Handle: respond without LLM (extension shows its own feedback)
  if (event.text === "ping") {
    ctx.ui.notify("pong", "info");
    return { action: "handled" };
  }

  // Route by source: skip processing for extension-injected messages
  if (event.source === "extension") return { action: "continue" };

  // Intercept skill commands before expansion
  if (event.text.startsWith("/skill:")) {
    // Could transform, block, or let pass through
  }

  return { action: "continue" };  // Default: pass through to expansion
});
```

**结果:**
- `continue` - 原样透传(如果处理程序未返回任何内容,则为默认行为)
- `transform` - 修改文本/图片,然后继续进行展开
- `handled` - 完全跳过 agent 处理(第一个返回该值的处理程序生效)

多个处理程序的 transform 会链式串联。参见 [input-transform.ts](../examples/extensions/input-transform.ts)。

## ExtensionContext

所有处理程序都会接收到 `ctx: ExtensionContext`。

### ctx.ui

用于用户交互的 UI 方法。完整详情参见 [自定义 UI](#custom-ui)。

### ctx.hasUI

在 print 模式(`-p`)和 JSON 模式下为 `false`。在交互模式和 RPC 模式下为 `true`。在 RPC 模式下,对话框类方法(`select`、`confirm`、`input`、`editor`)通过扩展 UI 子协议工作,而即发即弃(fire-and-forget)类方法(`notify`、`setStatus`、`setWidget`、`setTitle`、`setEditorText`)会向客户端发出请求。一些 TUI 特有的方法是空操作或返回默认值(参见 [rpc.md](rpc.md#extension-ui-protocol))。

### ctx.cwd

当前工作目录。

### ctx.sessionManager

对会话状态的只读访问。完整的 SessionManager API 及条目类型参见 [Session Format](session-format.md)。

对于 `tool_call`,该状态会在处理程序运行之前同步到当前的 assistant 消息为止。在并行工具执行模式下,仍然不保证包含同一条 assistant 消息中兄弟工具调用的结果。

```typescript
ctx.sessionManager.getEntries()       // All entries
ctx.sessionManager.getBranch()        // Current branch
ctx.sessionManager.getLeafId()        // Current leaf entry ID
```

### ctx.modelRegistry / ctx.model

访问模型与 API key。

### ctx.signal

当前 agent 的中止信号(abort signal),如果没有正在进行的 agent 轮次则为 `undefined`。

在扩展处理程序发起的、支持中止的嵌套异步工作中使用它,例如:
- `fetch(..., { signal: ctx.signal })`
- 接受 `signal` 的模型调用
- 接受 `AbortSignal` 的文件或进程相关辅助函数

`ctx.signal` 通常在活动轮次相关事件(如 `tool_call`、`tool_result`、`message_update` 和 `turn_end`)中是有值的。
在空闲(idle)或非轮次上下文中(例如会话事件、扩展命令,以及 pi 空闲时触发的快捷键),它通常为 `undefined`。

```typescript
pi.on("tool_result", async (event, ctx) => {
  const response = await fetch("https://example.com/api", {
    method: "POST",
    body: JSON.stringify(event),
    signal: ctx.signal,
  });

  const data = await response.json();
  return { details: data };
});
```

### ctx.isIdle() / ctx.abort() / ctx.hasPendingMessages()

控制流相关的辅助方法。

### ctx.shutdown()

请求优雅地关闭 pi。

- **交互模式:** 会推迟到 agent 变为空闲状态之后(即处理完所有已排队的 steer 消息和 follow-up 消息之后)。
- **RPC 模式:** 会推迟到下一次空闲状态(即完成当前命令的响应、等待下一个命令时)。
- **Print 模式:** 空操作。当所有 prompt 处理完毕后进程会自动退出。

在退出之前会向所有扩展发出 `session_shutdown` 事件。可在所有上下文中使用(事件处理程序、工具、命令、快捷键)。

```typescript
pi.on("tool_call", (event, ctx) => {
  if (isFatal(event.input)) {
    ctx.shutdown();
  }
});
```

### ctx.getContextUsage()

返回当前活动模型的上下文用量。在可用时使用最后一条 assistant 消息的用量,否则会为末尾的消息估算 token 数。

```typescript
const usage = ctx.getContextUsage();
if (usage && usage.tokens > 100_000) {
  // ...
}
```

### ctx.compact()

触发压缩(compaction)但不等待其完成。使用 `onComplete` 和 `onError` 进行后续操作。

```typescript
ctx.compact({
  customInstructions: "Focus on recent changes",
  onComplete: (result) => {
    ctx.ui.notify("Compaction completed", "info");
  },
  onError: (error) => {
    ctx.ui.notify(`Compaction failed: ${error.message}`, "error");
  },
});
```

### ctx.getSystemPrompt()

返回 Pi 当前的系统提示词字符串。

- 在 `before_agent_start` 期间,该值反映的是截至当前轮次已被串联(chained)的系统提示词修改。
- 它不包含之后 `context` 事件对消息所做的修改。
- 它不包含 `before_provider_request` 对负载(payload)所做的重写。
- 如果在你的扩展之后加载的其他扩展也运行了,它们仍然可以进一步修改最终发送的内容。

```typescript
pi.on("before_agent_start", (event, ctx) => {
  const prompt = ctx.getSystemPrompt();
  console.log(`System prompt length: ${prompt.length}`);
});
```

## ExtensionCommandContext

命令处理程序接收的是 `ExtensionCommandContext`,它在 `ExtensionContext` 的基础上扩展了会话控制方法。这些方法只在命令中可用,因为如果在事件处理程序中调用可能会导致死锁。

### ctx.waitForIdle()

等待 agent 完成流式输出:

```typescript
pi.registerCommand("my-cmd", {
  handler: async (args, ctx) => {
    await ctx.waitForIdle();
    // Agent is now idle, safe to modify session
  },
});
```

### ctx.newSession(options?)

创建一个新会话:

```typescript
const parentSession = ctx.sessionManager.getSessionFile();
const kickoff = "Continue in the replacement session";

const result = await ctx.newSession({
  parentSession,
  setup: async (sm) => {
    sm.appendMessage({
      role: "user",
      content: [{ type: "text", text: "Context from previous session..." }],
      timestamp: Date.now(),
    });
  },
  withSession: async (ctx) => {
    // Use only the replacement-session ctx here.
    await ctx.sendUserMessage(kickoff);
  },
});

if (result.cancelled) {
  // An extension cancelled the new session
}
```

选项:
- `parentSession`:要记录在新会话头部信息中的父会话文件
- `setup`:在 `withSession` 运行之前,修改新会话的 `SessionManager`
- `withSession`:针对全新的替换会话上下文运行切换后的工作。不要使用捕获的旧 `pi` / 命令 `ctx`;参见 [会话替换的生命周期与陷阱](#session-replacement-lifecycle-and-footguns)。

### ctx.fork(entryId, options?)

从指定条目进行分叉(fork),创建一个新的会话文件:

```typescript
const result = await ctx.fork("entry-id-123", {
  withSession: async (ctx) => {
    // Use only the replacement-session ctx here.
    ctx.ui.notify("Now in the forked session", "info");
  },
});
if (result.cancelled) {
  // An extension cancelled the fork
}

const cloneResult = await ctx.fork("entry-id-456", { position: "at" });
if (cloneResult.cancelled) {
  // An extension cancelled the clone
}
```

选项:
- `position`:`"before"`(默认)在被选中的用户消息之前分叉,并将该 prompt 恢复到编辑器中
- `position`:`"at"` 会复制经过所选条目的当前活动路径,但不恢复编辑器文本
- `withSession`:针对全新的替换会话上下文运行切换后的工作。不要使用捕获的旧 `pi` / 命令 `ctx`;参见 [会话替换的生命周期与陷阱](#session-replacement-lifecycle-and-footguns)。

### ctx.navigateTree(targetId, options?)

导航到会话树中的另一个位置:

```typescript
const result = await ctx.navigateTree("entry-id-456", {
  summarize: true,
  customInstructions: "Focus on error handling changes",
  replaceInstructions: false, // true = replace default prompt entirely
  label: "review-checkpoint",
});
```

选项:
- `summarize`:是否为被放弃的分支生成摘要
- `customInstructions`:提供给摘要生成器的自定义指令
- `replaceInstructions`:如果为 true,`customInstructions` 会替换默认 prompt,而不是追加在其后
- `label`:附加到分支摘要条目(如果不生成摘要,则附加到目标条目)上的标签

### ctx.switchSession(sessionPath, options?)

切换到另一个会话文件:

```typescript
const result = await ctx.switchSession("/path/to/session.jsonl", {
  withSession: async (ctx) => {
    await ctx.sendUserMessage("Resume work in the replacement session");
  },
});
if (result.cancelled) {
  // An extension cancelled the switch via session_before_switch
}
```

选项:
- `withSession`:针对全新的替换会话上下文运行切换后的工作。不要使用捕获的旧 `pi` / 命令 `ctx`;参见 [会话替换的生命周期与陷阱](#session-replacement-lifecycle-and-footguns)。

要发现可用的会话,可使用静态方法 `SessionManager.list()` 或 `SessionManager.listAll()`:

```typescript
import { SessionManager } from "@earendil-works/pi-coding-agent";

pi.registerCommand("switch", {
  description: "Switch to another session",
  handler: async (args, ctx) => {
    const sessions = await SessionManager.list(ctx.cwd);
    if (sessions.length === 0) return;
    const choice = await ctx.ui.select(
      "Pick session:",
      sessions.map(s => s.file),
    );
    if (choice) {
      await ctx.switchSession(choice, {
        withSession: async (ctx) => {
          ctx.ui.notify("Switched session", "info");
        },
      });
    }
  },
});
```

### 会话替换的生命周期与陷阱

`withSession` 接收一个全新的 `ReplacedSessionContext`,它在 `ExtensionCommandContext` 的基础上扩展了绑定到替换会话上的异步方法 `sendMessage()` 和 `sendUserMessage()`。

生命周期与陷阱:
- `withSession` 只有在旧会话已经触发了 `session_shutdown`、旧的运行时已被销毁、替换会话已重新绑定、且新的扩展实例已经收到 `session_start` 之后才会运行。
- 该回调仍然在原来的闭包中执行,而不是在新的扩展实例内部执行。这意味着你的旧扩展实例可能在 `withSession` 开始之前就已经执行完了它的关闭清理逻辑。
- 被替换之后,捕获的旧 `pi` / 旧命令 `ctx` 中与会话绑定的对象已经失效,使用会抛出错误。会话相关工作只应使用传给 `withSession` 的那个 `ctx`。
- 之前提取出的原始对象仍然由你自己负责管理。例如,如果你在替换之前捕获了 `const sm = ctx.sessionManager`,那么 `sm` 仍然是旧的 `SessionManager` 对象。替换之后不要再复用它。
- `withSession` 中的代码应当假设任何被你的 `session_shutdown` 处理程序失效的状态都已经不存在了。只捕获能够安全经受住关闭清理的纯数据,例如字符串、ID 和已序列化的配置。

安全的写法:

```typescript
pi.registerCommand("handoff", {
  handler: async (_args, ctx) => {
    const kickoff = "Continue from the replacement session";
    await ctx.newSession({
      withSession: async (ctx) => {
        await ctx.sendUserMessage(kickoff);
      },
    });
  },
});
```

不安全的写法:

```typescript
pi.registerCommand("handoff", {
  handler: async (_args, ctx) => {
    const oldSessionManager = ctx.sessionManager;
    await ctx.newSession({
      withSession: async (_ctx) => {
        // stale old objects: do not do this
        oldSessionManager.getSessionFile();
        pi.sendUserMessage("wrong");
      },
    });
  },
});
```

### ctx.reload()

运行与 `/reload` 相同的重新加载流程。

```typescript
pi.registerCommand("reload-runtime", {
  description: "Reload extensions, skills, prompts, and themes",
  handler: async (_args, ctx) => {
    await ctx.reload();
    return;
  },
});
```

重要行为说明:
- `await ctx.reload()` 会为当前的扩展运行时触发 `session_shutdown`
- 之后它会重新加载资源,并触发 `session_start`(`reason: "reload"`)和 `resources_discover`(reason 为 `"reload"`)
- 当前正在运行的命令处理程序仍然会在旧的调用帧(call frame)中继续执行
- `await ctx.reload()` 之后的代码仍然运行在重新加载之前的版本中
- `await ctx.reload()` 之后的代码不应假设旧的内存中扩展状态仍然有效
- 处理程序返回之后,后续的命令/事件/工具调用会使用新版本的扩展

为了行为可预测,应将 reload 视为该处理程序的终点(`await ctx.reload(); return;`)。

工具是在 `ExtensionContext` 下运行的,因此它们不能直接调用 `ctx.reload()`。请使用一个命令作为 reload 的入口点,再暴露一个工具,让它把该命令作为 follow-up 用户消息排队执行。

以下是一个可供 LLM 调用以触发 reload 的工具示例:

```typescript
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";

export default function (pi: ExtensionAPI) {
  pi.registerCommand("reload-runtime", {
    description: "Reload extensions, skills, prompts, and themes",
    handler: async (_args, ctx) => {
      await ctx.reload();
      return;
    },
  });

  pi.registerTool({
    name: "reload_runtime",
    label: "Reload Runtime",
    description: "Reload extensions, skills, prompts, and themes",
    parameters: Type.Object({}),
    async execute() {
      pi.sendUserMessage("/reload-runtime", { deliverAs: "followUp" });
      return {
        content: [{ type: "text", text: "Queued /reload-runtime as a follow-up command." }],
      };
    },
  });
}
```

## ExtensionAPI 方法

### pi.on(event, handler)

订阅事件。事件类型及返回值参见[事件](#events)。

### pi.registerTool(definition)

注册一个可被 LLM 调用的自定义工具。完整详情参见[自定义工具](#custom-tools)。

`pi.registerTool()` 在扩展加载期间和启动之后都可以使用。你可以在 `session_start`、命令处理程序或其他事件处理程序中调用它。新工具会在同一会话中立即刷新生效,因此它们会出现在 `pi.getAllTools()` 中,并且无需 `/reload` 即可被 LLM 调用。

使用 `pi.setActiveTools()` 可以在运行时启用或禁用工具(包括动态添加的工具)。

使用 `promptSnippet` 可以让自定义工具在 `Available tools` 中拥有一行简要条目,使用 `promptGuidelines` 可以在该工具处于活动状态时,向默认的 `Guidelines` 部分追加工具特定的要点。

**重要提示:** `promptGuidelines` 中的要点会被平铺追加到 `Guidelines` 部分,不带工具名称前缀。每条准则都必须点明它所指的工具——避免使用"Use this tool when..."这样的表述,因为 LLM 无法判断"this"指的是哪个工具。应写成"Use my_tool when..."。

完整示例参见 [dynamic-tools.ts](../examples/extensions/dynamic-tools.ts)。

```typescript
import { Type } from "typebox";
import { StringEnum } from "@earendil-works/pi-ai";

pi.registerTool({
  name: "my_tool",
  label: "My Tool",
  description: "What this tool does",
  promptSnippet: "Summarize or transform text according to action",
  promptGuidelines: ["Use my_tool when the user asks to summarize previously generated text."],
  parameters: Type.Object({
    action: StringEnum(["list", "add"] as const),
    text: Type.Optional(Type.String()),
  }),
  prepareArguments(args) {
    // Optional compatibility shim. Runs before schema validation.
    // Return the current schema shape, for example to fold legacy fields
    // into the modern parameter object.
    return args;
  },

  async execute(toolCallId, params, signal, onUpdate, ctx) {
    // Stream progress
    onUpdate?.({ content: [{ type: "text", text: "Working..." }] });

    return {
      content: [{ type: "text", text: "Done" }],
      details: { result: "..." },
    };
  },

  // Optional: Custom rendering
  renderCall(args, theme, context) { ... },
  renderResult(result, options, theme, context) { ... },
});
```

### pi.sendMessage(message, options?)

向会话中注入一条自定义消息。

```typescript
pi.sendMessage({
  customType: "my-extension",
  content: "Message text",
  display: true,
  details: { ... },
}, {
  triggerTurn: true,
  deliverAs: "steer",
});
```

**选项:**
- `deliverAs` - 投递模式:
  - `"steer"`(默认)- 在流式输出期间将消息排队。会在当前 assistant 轮次执行完其工具调用之后、下一次 LLM 调用之前投递。
  - `"followUp"` - 等待 agent 完成。仅当 agent 没有更多工具调用时才会投递。
  - `"nextTurn"` - 排队等待下一次用户 prompt。不会打断或触发任何操作。
- `triggerTurn: true` - 如果 agent 处于空闲状态,则立即触发一次 LLM 响应。仅适用于 `"steer"` 和 `"followUp"` 模式(在 `"nextTurn"` 模式下会被忽略)。

### pi.sendUserMessage(content, options?)

向 agent 发送一条用户消息。与发送自定义消息的 `sendMessage()` 不同,这会发送一条看起来像是用户手动输入的真实用户消息。总是会触发一轮新的处理。

```typescript
// Simple text message
pi.sendUserMessage("What is 2+2?");

// With content array (text + images)
pi.sendUserMessage([
  { type: "text", text: "Describe this image:" },
  { type: "image", source: { type: "base64", mediaType: "image/png", data: "..." } },
]);

// During streaming - must specify delivery mode
pi.sendUserMessage("Focus on error handling", { deliverAs: "steer" });
pi.sendUserMessage("And then summarize", { deliverAs: "followUp" });
```

**选项:**
- `deliverAs` - 当 agent 正在流式输出时为必填项:
  - `"steer"` - 将消息排队,在当前 assistant 轮次执行完其工具调用之后投递
  - `"followUp"` - 等待 agent 完成所有工具调用

当没有在进行流式输出时,消息会立即发送并触发新的一轮处理。在流式输出期间若未指定 `deliverAs`,则会抛出错误。

完整示例参见 [send-user-message.ts](../examples/extensions/send-user-message.ts)。

### pi.appendEntry(customType, data?)

持久化保存扩展状态(不会参与 LLM 上下文)。

```typescript
pi.appendEntry("my-state", { count: 42 });

// Restore on reload
pi.on("session_start", async (_event, ctx) => {
  for (const entry of ctx.sessionManager.getEntries()) {
    if (entry.type === "custom" && entry.customType === "my-state") {
      // Reconstruct from entry.data
    }
  }
});
```

### pi.setSessionName(name)

设置会话的显示名称(在会话选择器中显示,替代第一条消息)。

```typescript
pi.setSessionName("Refactor auth module");
```

### pi.getSessionName()

获取当前会话名称(如果已设置)。

```typescript
const name = pi.getSessionName();
if (name) {
  console.log(`Session: ${name}`);
}
```

### pi.setLabel(entryId, label)

为某个条目设置或清除标签。标签是用户自定义的标记,用于书签和导航(会在 `/tree` 选择器中显示)。

```typescript
// Set a label
pi.setLabel(entryId, "checkpoint-before-refactor");

// Clear a label
pi.setLabel(entryId, undefined);

// Read labels via sessionManager
const label = ctx.sessionManager.getLabel(entryId);
```

标签会持久化在会话中,并在重启后保留。可以用它们来标记对话树中的重要节点(轮次、检查点)。

### pi.registerCommand(name, options)

注册一个命令。

如果多个扩展注册了相同的命令名称,pi 会保留所有这些命令,并按加载顺序为它们分配数字调用后缀,例如 `/review:1` 和 `/review:2`。

```typescript
pi.registerCommand("stats", {
  description: "Show session statistics",
  handler: async (args, ctx) => {
    const count = ctx.sessionManager.getEntries().length;
    ctx.ui.notify(`${count} entries`, "info");
  }
});
```

可选:为 `/command ...` 添加参数自动补全:

```typescript
import type { AutocompleteItem } from "@earendil-works/pi-tui";

pi.registerCommand("deploy", {
  description: "Deploy to an environment",
  getArgumentCompletions: (prefix: string): AutocompleteItem[] | null => {
    const envs = ["dev", "staging", "prod"];
    const items = envs.map((e) => ({ value: e, label: e }));
    const filtered = items.filter((i) => i.value.startsWith(prefix));
    return filtered.length > 0 ? filtered : null;
  },
  handler: async (args, ctx) => {
    ctx.ui.notify(`Deploying: ${args}`, "info");
  },
});
```

### pi.getCommands()

获取在当前会话中可通过 `prompt` 调用的斜杠命令。包括扩展命令、prompt 模板和 skill 命令。
该列表的顺序与 RPC 的 `get_commands` 一致:先是扩展命令,然后是模板,最后是 skill。

```typescript
const commands = pi.getCommands();
const bySource = commands.filter((command) => command.source === "extension");
const userScoped = commands.filter((command) => command.sourceInfo.scope === "user");
```

每个条目的结构如下:

```typescript
{
  name: string; // Invokable command name without the leading slash. May be suffixed like "review:1"
  description?: string;
  source: "extension" | "prompt" | "skill";
  sourceInfo: {
    path: string;
    source: string;
    scope: "user" | "project" | "temporary";
    origin: "package" | "top-level";
    baseDir?: string;
  };
}
```

请将 `sourceInfo` 作为权威的来源(provenance)字段使用。不要通过命令名称推断归属,也不要通过临时的路径解析来推断归属。

内置的交互式命令(如 `/model` 和 `/settings`)不包含在此列表中。它们仅在交互模式下处理,如果通过 `prompt` 发送则不会执行。

### pi.registerMessageRenderer(customType, renderer)

为带有你的 `customType` 的消息注册自定义 TUI 渲染器。参见[自定义 UI](#custom-ui)。

### pi.registerShortcut(shortcut, options)

注册一个键盘快捷键。快捷键格式及内置键位绑定参见 [keybindings.md](keybindings.md)。

```typescript
pi.registerShortcut("ctrl+shift+p", {
  description: "Toggle plan mode",
  handler: async (ctx) => {
    ctx.ui.notify("Toggled!");
  },
});
```

### pi.registerFlag(name, options)

注册一个 CLI 参数(flag)。

```typescript
pi.registerFlag("plan", {
  description: "Start in plan mode",
  type: "boolean",
  default: false,
});

// Check value
if (pi.getFlag("plan")) {
  // Plan mode enabled
}
```

### pi.exec(command, args, options?)

执行一条 shell 命令。

```typescript
const result = await pi.exec("git", ["status"], { signal, timeout: 5000 });
// result.stdout, result.stderr, result.code, result.killed
```

### pi.getActiveTools() / pi.getAllTools() / pi.setActiveTools(names)

管理活动工具。这既适用于内置工具,也适用于动态注册的工具。

```typescript
const active = pi.getActiveTools();
const all = pi.getAllTools();
// [{
//   name: "read",
//   description: "Read file contents...",
//   parameters: ..., 
//   sourceInfo: { path: "<builtin:read>", source: "builtin", scope: "temporary", origin: "top-level" }
// }, ...]
const names = all.map(t => t.name);
const builtinTools = all.filter((t) => t.sourceInfo.source === "builtin");
const extensionTools = all.filter((t) => t.sourceInfo.source !== "builtin" && t.sourceInfo.source !== "sdk");
pi.setActiveTools(["read", "bash"]); // Switch to read-only
```

`pi.getAllTools()` 返回 `name`、`description`、`parameters` 和 `sourceInfo`。

典型的 `sourceInfo.source` 取值:
- `builtin` 表示内置工具
- `sdk` 表示通过 `createAgentSession({ customTools })` 传入的工具
- 扩展注册的工具会带有对应的扩展来源元数据

### pi.setModel(model)

设置当前模型。如果该模型没有可用的 API key,则返回 `false`。配置自定义模型的方法参见 [models.md](models.md)。

```typescript
const model = ctx.modelRegistry.find("anthropic", "claude-sonnet-4-5");
if (model) {
  const success = await pi.setModel(model);
  if (!success) {
    ctx.ui.notify("No API key for this model", "error");
  }
}
```

### pi.getThinkingLevel() / pi.setThinkingLevel(level)

获取或设置思维等级。等级会被限制在模型能力范围内(非推理模型始终使用 "off")。变更会触发 `thinking_level_select`。

```typescript
const current = pi.getThinkingLevel();  // "off" | "minimal" | "low" | "medium" | "high" | "xhigh"
pi.setThinkingLevel("high");
```

### pi.events

用于扩展之间通信的共享事件总线:

```typescript
pi.events.on("my:event", (data) => { ... });
pi.events.emit("my:event", { ... });
```

### pi.registerProvider(name, config)

动态注册或覆盖一个模型 provider。适用于代理、自定义端点或团队级的模型配置。

在扩展工厂函数执行期间发起的调用会被排队,并在运行器(runner)初始化完成后统一生效。之后发起的调用——例如在用户完成设置流程后,由命令处理程序发起的调用——会立即生效,无需 `/reload`。

如果你需要从远程端点发现模型,建议使用异步扩展工厂函数,而不是把获取过程推迟到 `session_start` 中。pi 会在启动继续之前等待工厂函数完成,因此注册的模型会立即可用,包括在 `pi --list-models` 中。

```typescript
// Register a new provider with custom models
pi.registerProvider("my-proxy", {
  name: "My Proxy",
  baseUrl: "https://proxy.example.com",
  apiKey: "PROXY_API_KEY",  // env var name or literal
  api: "anthropic-messages",
  models: [
    {
      id: "claude-sonnet-4-20250514",
      name: "Claude 4 Sonnet (proxy)",
      reasoning: false,
      input: ["text", "image"],
      cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
      contextWindow: 200000,
      maxTokens: 16384
    }
  ]
});

// Override baseUrl for an existing provider (keeps all models)
pi.registerProvider("anthropic", {
  baseUrl: "https://proxy.example.com"
});

// Register provider with OAuth support for /login
pi.registerProvider("corporate-ai", {
  baseUrl: "https://ai.corp.com",
  api: "openai-responses",
  models: [...],
  oauth: {
    name: "Corporate AI (SSO)",
    async login(callbacks) {
      // Custom OAuth flow
      callbacks.onAuth({ url: "https://sso.corp.com/..." });
      const code = await callbacks.onPrompt({ message: "Enter code:" });
      return { refresh: code, access: code, expires: Date.now() + 3600000 };
    },
    async refreshToken(credentials) {
      // Refresh logic
      return credentials;
    },
    getApiKey(credentials) {
      return credentials.access;
    }
  }
});
```

**配置选项:**
- `name` - provider 在 UI(如 `/login`)中的显示名称。
- `baseUrl` - API 端点 URL。定义模型时必填。
- `apiKey` - API key 或环境变量名称。定义模型时必填(除非提供了 `oauth`)。
- `api` - API 类型:`"anthropic-messages"`、`"openai-completions"`、`"openai-responses"` 等。
- `headers` - 请求中包含的自定义请求头。
- `authHeader` - 如果为 true,会自动添加 `Authorization: Bearer` 请求头。
- `models` - 模型定义数组。如果提供,会替换该 provider 现有的所有模型。模型定义可以设置 `baseUrl` 来为该模型覆盖 provider 的端点。
- `oauth` - 用于支持 `/login` 的 OAuth provider 配置。提供该字段后,该 provider 会出现在登录菜单中。
- `streamSimple` - 针对非标准 API 的自定义流式实现。

关于自定义流式 API、OAuth 细节、模型定义参考等进阶主题,参见 [custom-provider.md](custom-provider.md)。

### pi.unregisterProvider(name)

移除之前注册的 provider 及其模型。被该 provider 覆盖的内置模型会被恢复。如果该 provider 未被注册过,则此操作无效果。

与 `registerProvider` 一样,在初始加载阶段之后调用会立即生效,不需要 `/reload`。

```typescript
pi.registerCommand("my-setup-teardown", {
  description: "Remove the custom proxy provider",
  handler: async (_args, _ctx) => {
    pi.unregisterProvider("my-proxy");
  },
});
```

## 状态管理

有状态的扩展应当将状态存储在工具结果的 `details` 中,以便正确支持分支(branching):

```typescript
export default function (pi: ExtensionAPI) {
  let items: string[] = [];

  // Reconstruct state from session
  pi.on("session_start", async (_event, ctx) => {
    items = [];
    for (const entry of ctx.sessionManager.getBranch()) {
      if (entry.type === "message" && entry.message.role === "toolResult") {
        if (entry.message.toolName === "my_tool") {
          items = entry.message.details?.items ?? [];
        }
      }
    }
  });

  pi.registerTool({
    name: "my_tool",
    // ...
    async execute(toolCallId, params, signal, onUpdate, ctx) {
      items.push("new item");
      return {
        content: [{ type: "text", text: "Added" }],
        details: { items: [...items] },  // Store for reconstruction
      };
    },
  });
}
```

## 自定义工具

通过 `pi.registerTool()` 注册 LLM 可调用的工具。工具会出现在系统提示词中,并且可以拥有自定义渲染。

使用 `promptSnippet` 可以在默认系统提示词的 `Available tools` 部分中添加一行简短条目。如果省略,自定义工具就不会出现在该部分中。

使用 `promptGuidelines` 可以在默认系统提示词的 `Guidelines` 部分中添加工具特定的要点。这些要点只有在该工具处于活动状态时才会包含在内(例如,在调用 `pi.setActiveTools([...])` 之后)。

**重要提示:** `promptGuidelines` 中的要点会被平铺追加到 `Guidelines` 部分,没有工具名称前缀或分组。每条准则都必须点明它所指的工具——避免使用"Use this tool when..."这样的表述,因为 LLM 无法判断"this"指的是哪个工具。应写成"Use my_tool when..."。

注意:有些模型比较笨,会在工具的路径参数中包含 @ 前缀。内置工具在解析路径之前会去除开头的 @。如果你的自定义工具接受路径参数,也应当对开头的 @ 做同样的归一化处理。

如果你的自定义工具会修改文件,请使用 `withFileMutationQueue()`,使其加入与内置 `edit` 和 `write` 相同的按文件队列。这一点很重要,因为工具调用默认是并行运行的。如果不使用该队列,两个工具可能会读取同一份旧的文件内容,分别计算出不同的更新,然后无论哪个写入后完成,都会覆盖掉另一个的结果。

失败场景示例:你的自定义工具在编辑 `foo.ts` 的同时,内置的 `edit` 也在同一个 assistant 轮次中修改了 `foo.ts`。如果你的工具没有加入这个队列,两者都可能读取到原始的 `foo.ts`,各自应用不同的修改,其中一个修改就会丢失。

请将真实的目标文件路径传给 `withFileMutationQueue()`,而不是原始的用户参数。先将其解析为绝对路径,相对于 `ctx.cwd` 或你工具的工作目录。对于已存在的文件,该辅助函数会通过 `realpath()` 对路径进行规范化,因此指向同一文件的符号链接别名会共享同一个队列。对于新文件,由于此时还没有东西可以 `realpath()`,它会回退使用解析后的绝对路径。

需要将整个变更窗口都纳入该目标路径的队列中,这包括读取-修改-写入的逻辑,而不仅仅是最后的写入操作。

```typescript
import { withFileMutationQueue } from "@earendil-works/pi-coding-agent";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";

async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
  const absolutePath = resolve(ctx.cwd, params.path);

  return withFileMutationQueue(absolutePath, async () => {
    await mkdir(dirname(absolutePath), { recursive: true });
    const current = await readFile(absolutePath, "utf8");
    const next = current.replace(params.oldText, params.newText);
    await writeFile(absolutePath, next, "utf8");

    return {
      content: [{ type: "text", text: `Updated ${params.path}` }],
      details: {},
    };
  });
}
```

### 工具定义

```typescript
import { Type } from "typebox";
import { StringEnum } from "@earendil-works/pi-ai";
import { Text } from "@earendil-works/pi-tui";

pi.registerTool({
  name: "my_tool",
  label: "My Tool",
  description: "What this tool does (shown to LLM)",
  promptSnippet: "List or add items in the project todo list",
  promptGuidelines: [
    "Use my_tool for todo planning instead of direct file edits when the user asks for a task list."
  ],
  parameters: Type.Object({
    action: StringEnum(["list", "add"] as const),  // Use StringEnum for Google compatibility
    text: Type.Optional(Type.String()),
  }),
  prepareArguments(args) {
    if (!args || typeof args !== "object") return args;
    const input = args as { action?: string; oldAction?: string };
    if (typeof input.oldAction === "string" && input.action === undefined) {
      return { ...input, action: input.oldAction };
    }
    return args;
  },

  async execute(toolCallId, params, signal, onUpdate, ctx) {
    // Check for cancellation
    if (signal?.aborted) {
      return { content: [{ type: "text", text: "Cancelled" }] };
    }

    // Stream progress updates
    onUpdate?.({
      content: [{ type: "text", text: "Working..." }],
      details: { progress: 50 },
    });

    // Run commands via pi.exec (captured from extension closure)
    const result = await pi.exec("some-command", [], { signal });

    // Return result
    return {
      content: [{ type: "text", text: "Done" }],  // Sent to LLM
      details: { data: result },                   // For rendering & state
      // Optional: stop after this tool batch when every finalized tool result
      // in the batch also returns terminate: true.
      terminate: true,
    };
  },

  // Optional: Custom rendering
  renderCall(args, theme, context) { ... },
  renderResult(result, options, theme, context) { ... },
});
```

**发出错误信号:** 要将某次工具执行标记为失败(会在结果上设置 `isError: true` 并报告给 LLM),需要从 `execute` 中抛出一个错误。无论返回对象中包含哪些属性,直接返回值都不会设置错误标志。

**提前终止:** 从 `execute()` 中返回 `terminate: true`,用于提示应跳过当前工具批次之后自动进行的后续 LLM 调用。只有当该批次中每一个最终确定的工具结果都返回了 terminate 时,该提示才会生效。一个 agent 在最终结构化输出工具调用后结束的极简示例参见 [examples/extensions/structured-output.ts](../examples/extensions/structured-output.ts)。

```typescript
// Correct: throw to signal an error
async execute(toolCallId, params) {
  if (!isValid(params.input)) {
    throw new Error(`Invalid input: ${params.input}`);
  }
  return { content: [{ type: "text", text: "OK" }], details: {} };
}
```

**重要提示:** 字符串枚举请使用 `@earendil-works/pi-ai` 中的 `StringEnum`。`Type.Union`/`Type.Literal` 在 Google 的 API 中不适用。

**参数预处理:** `prepareArguments(args)` 是可选的。如果定义了它,会在 schema 校验之前、`execute()` 之前运行。当 pi 恢复一个旧会话,而该会话中存储的工具调用参数已经不再匹配当前 schema 时,可以用它来模拟旧版本所接受的输入形态。返回你希望针对 `parameters` 进行校验的对象。请保持公开 schema 的严格性,不要仅仅为了让旧的、被恢复的会话能继续工作,就在 `parameters` 中添加已废弃的兼容字段。

举例来说:旧会话中可能包含一个 `edit` 工具调用,其顶层带有 `oldText` 和 `newText`,而当前 schema 只接受 `edits: [{ oldText, newText }]`。

```typescript
pi.registerTool({
  name: "edit",
  label: "Edit",
  description: "Edit a single file using exact text replacement",
  parameters: Type.Object({
    path: Type.String(),
    edits: Type.Array(
      Type.Object({
        oldText: Type.String(),
        newText: Type.String(),
      }),
    ),
  }),
  prepareArguments(args) {
    if (!args || typeof args !== "object") return args;

    const input = args as {
      path?: string;
      edits?: Array<{ oldText: string; newText: string }>;
      oldText?: unknown;
      newText?: unknown;
    };

    if (typeof input.oldText !== "string" || typeof input.newText !== "string") {
      return args;
    }

    return {
      ...input,
      edits: [...(input.edits ?? []), { oldText: input.oldText, newText: input.newText }],
    };
  },
  async execute(toolCallId, params, signal, onUpdate, ctx) {
    // params now matches the current schema
    return {
      content: [{ type: "text", text: `Applying ${params.edits.length} edit block(s)` }],
      details: {},
    };
  },
});
```

### 覆盖内置工具

扩展可以通过注册同名工具来覆盖内置工具(`read`、`bash`、`edit`、`write`、`grep`、`find`、`ls`)。这种情况发生时,交互模式会显示一条警告。

```bash
# Extension's read tool replaces built-in read
pi -e ./tool-override.ts
```

或者,使用 `--no-builtin-tools` 可以在不启用任何内置工具的情况下启动,同时保留扩展工具:
```bash
# No built-in tools, only extension tools
pi --no-builtin-tools -e ./my-extension.ts
```

一个覆盖 `read` 并加入日志记录与访问控制的完整示例参见 [examples/extensions/tool-override.ts](../examples/extensions/tool-override.ts)。

**渲染:** 内置渲染器的继承是按插槽(slot)分别解析的。执行覆盖与渲染覆盖是相互独立的。如果你的覆盖省略了 `renderCall`,则会使用内置的 `renderCall`。如果你的覆盖省略了 `renderResult`,则会使用内置的 `renderResult`。如果两者都省略,则会自动使用内置渲染器(语法高亮、diff 等)。这使你可以在不重新实现 UI 的情况下,为内置工具包装日志记录或访问控制逻辑。

**Prompt 元数据:** `promptSnippet` 和 `promptGuidelines` 不会从内置工具继承。如果你的覆盖希望保留这些 prompt 指令,需要在覆盖中显式定义它们。

**你的实现必须匹配完全一致的结果形态**,包括 `details` 的类型。UI 和会话逻辑都依赖这些形态来进行渲染和状态追踪。

内置工具实现:
- [read.ts](https://github.com/earendil-works/pi-mono/blob/main/packages/coding-agent/src/core/tools/read.ts) - `ReadToolDetails`
- [bash.ts](https://github.com/earendil-works/pi-mono/blob/main/packages/coding-agent/src/core/tools/bash.ts) - `BashToolDetails`
- [edit.ts](https://github.com/earendil-works/pi-mono/blob/main/packages/coding-agent/src/core/tools/edit.ts)
- [write.ts](https://github.com/earendil-works/pi-mono/blob/main/packages/coding-agent/src/core/tools/write.ts)
- [grep.ts](https://github.com/earendil-works/pi-mono/blob/main/packages/coding-agent/src/core/tools/grep.ts) - `GrepToolDetails`
- [find.ts](https://github.com/earendil-works/pi-mono/blob/main/packages/coding-agent/src/core/tools/find.ts) - `FindToolDetails`
- [ls.ts](https://github.com/earendil-works/pi-mono/blob/main/packages/coding-agent/src/core/tools/ls.ts) - `LsToolDetails`

### 远程执行

内置工具支持可插拔的操作(operations),用于委托给远程系统(SSH、容器等)执行:

```typescript
import { createReadTool, createBashTool, type ReadOperations } from "@earendil-works/pi-coding-agent";

// Create tool with custom operations
const remoteRead = createReadTool(cwd, {
  operations: {
    readFile: (path) => sshExec(remote, `cat ${path}`),
    access: (path) => sshExec(remote, `test -r ${path}`).then(() => {}),
  }
});

// Register, checking flag at execution time
pi.registerTool({
  ...remoteRead,
  async execute(id, params, signal, onUpdate, _ctx) {
    const ssh = getSshConfig();
    if (ssh) {
      const tool = createReadTool(cwd, { operations: createRemoteOps(ssh) });
      return tool.execute(id, params, signal, onUpdate);
    }
    return localRead.execute(id, params, signal, onUpdate);
  },
});
```

**Operations 接口:** `ReadOperations`、`WriteOperations`、`EditOperations`、`BashOperations`、`LsOperations`、`GrepOperations`、`FindOperations`

对于 `user_bash`,扩展可以复用 pi 内置的本地 shell 后端 `createLocalBashOperations()`,而无需重新实现本地进程的生成、shell 解析以及进程树终止逻辑。

bash 工具还支持一个 spawn 钩子,可以在执行前调整命令、cwd 或环境变量:

```typescript
import { createBashTool } from "@earendil-works/pi-coding-agent";

const bashTool = createBashTool(cwd, {
  spawnHook: ({ command, cwd, env }) => ({
    command: `source ~/.profile\n${command}`,
    cwd: `/mnt/sandbox${cwd}`,
    env: { ...env, CI: "1" },
  }),
});
```

一个带 `--ssh` 参数的完整 SSH 示例参见 [examples/extensions/ssh.ts](../examples/extensions/ssh.ts)。

### 输出截断

**工具必须对输出进行截断**,以避免让 LLM 上下文不堪重负。过大的输出可能导致:
- 上下文溢出错误(prompt 过长)
- 压缩失败
- 模型性能下降

内置限制为 **50KB**(约 1 万 token)和 **2000 行**,以先达到者为准。请使用已导出的截断工具函数:

```typescript
import {
  truncateHead,      // Keep first N lines/bytes (good for file reads, search results)
  truncateTail,      // Keep last N lines/bytes (good for logs, command output)
  truncateLine,      // Truncate a single line to maxBytes with ellipsis
  formatSize,        // Human-readable size (e.g., "50KB", "1.5MB")
  DEFAULT_MAX_BYTES, // 50KB
  DEFAULT_MAX_LINES, // 2000
} from "@earendil-works/pi-coding-agent";

async execute(toolCallId, params, signal, onUpdate, ctx) {
  const output = await runCommand();

  // Apply truncation
  const truncation = truncateHead(output, {
    maxLines: DEFAULT_MAX_LINES,
    maxBytes: DEFAULT_MAX_BYTES,
  });

  let result = truncation.content;

  if (truncation.truncated) {
    // Write full output to temp file
    const tempFile = writeTempFile(output);

    // Inform the LLM where to find complete output
    result += `\n\n[Output truncated: ${truncation.outputLines} of ${truncation.totalLines} lines`;
    result += ` (${formatSize(truncation.outputBytes)} of ${formatSize(truncation.totalBytes)}).`;
    result += ` Full output saved to: ${tempFile}]`;
  }

  return { content: [{ type: "text", text: result }] };
}
```

**要点:**
- 对于开头内容更重要的场景(搜索结果、文件读取),使用 `truncateHead`
- 对于末尾内容更重要的场景(日志、命令输出),使用 `truncateTail`
- 输出被截断时务必告知 LLM,并说明去哪里能找到完整版本
- 在工具的描述中注明截断限制

一个包装 `rg`(ripgrep)并正确处理截断的完整示例参见 [examples/extensions/truncated-tool.ts](../examples/extensions/truncated-tool.ts)。

### 多个工具

一个扩展可以注册多个共享状态的工具:

```typescript
export default function (pi: ExtensionAPI) {
  let connection = null;

  pi.registerTool({ name: "db_connect", ... });
  pi.registerTool({ name: "db_query", ... });
  pi.registerTool({ name: "db_close", ... });

  pi.on("session_shutdown", async () => {
    connection?.close();
  });
}
```

### 自定义渲染

工具可以提供 `renderCall` 和 `renderResult` 以实现自定义 TUI 展示。完整组件 API 参见 [tui.md](tui.md),工具行是如何组合渲染的参见 [tool-execution.ts](https://github.com/earendil-works/pi-mono/blob/main/packages/coding-agent/src/modes/interactive/components/tool-execution.ts)。

默认情况下,工具输出会被包裹在一个处理内边距和背景的 `Box` 中。定义的 `renderCall` 或 `renderResult` 必须返回一个 `Component`。如果某个插槽没有定义渲染器,`tool-execution.ts` 会为该插槽使用回退渲染。

当工具需要渲染自己的外壳,而不是使用默认的 `Box` 时,设置 `renderShell: "self"`。这适用于需要完全控制边框(framing)或背景行为的工具,例如需要在工具执行完成后保持视觉稳定的大型预览。

```typescript
pi.registerTool({
  name: "my_tool",
  label: "My Tool",
  description: "Custom shell example",
  parameters: Type.Object({}),
  renderShell: "self",
  async execute() {
    return { content: [{ type: "text", text: "ok" }], details: undefined };
  },
  renderCall(args, theme, context) {
    return new Text(theme.fg("accent", "my custom shell"), 0, 0);
  },
});
```

`renderCall` 和 `renderResult` 各自接收一个 `context` 对象,其中包含:
- `args` - 当前的工具调用参数
- `state` - `renderCall` 和 `renderResult` 之间共享的行级(row-local)状态
- `lastComponent` - 该插槽上一次返回的组件(如果有)
- `invalidate()` - 请求重新渲染这一行工具输出
- `toolCallId`、`cwd`、`executionStarted`、`argsComplete`、`isPartial`、`expanded`、`showImages`、`isError`

使用 `context.state` 来存放需要跨插槽共享的状态。当你希望在多次渲染之间复用并原地修改同一个组件实例时,把插槽本地的缓存保存在返回的组件实例上。

#### renderCall

渲染工具调用或头部信息:

```typescript
import { Text } from "@earendil-works/pi-tui";

renderCall(args, theme, context) {
  const text = (context.lastComponent as Text | undefined) ?? new Text("", 0, 0);
  let content = theme.fg("toolTitle", theme.bold("my_tool "));
  content += theme.fg("muted", args.action);
  if (args.text) {
    content += " " + theme.fg("dim", `"${args.text}"`);
  }
  text.setText(content);
  return text;
}
```

#### renderResult

渲染工具结果或输出:

```typescript
renderResult(result, { expanded, isPartial }, theme, context) {
  if (isPartial) {
    return new Text(theme.fg("warning", "Processing..."), 0, 0);
  }

  if (result.details?.error) {
    return new Text(theme.fg("error", `Error: ${result.details.error}`), 0, 0);
  }

  let text = theme.fg("success", "✓ Done");
  if (expanded && result.details?.items) {
    for (const item of result.details.items) {
      text += "\n  " + theme.fg("dim", item);
    }
  }
  return new Text(text, 0, 0);
}
```

如果某个插槽本身就没有可显示的内容,返回一个空的 `Component`,例如一个空的 `Container`。

#### 键位提示(Keybinding Hints)

使用 `keyHint()` 显示与当前生效的键位绑定配置相匹配的快捷键提示:

```typescript
import { keyHint } from "@earendil-works/pi-coding-agent";

renderResult(result, { expanded }, theme, context) {
  let text = theme.fg("success", "✓ Done");
  if (!expanded) {
    text += ` (${keyHint("app.tools.expand", "to expand")})`;
  }
  return new Text(text, 0, 0);
}
```

可用函数:
- `keyHint(keybinding, description)` - 格式化一个已配置的键位绑定 ID,例如 `"app.tools.expand"` 或 `"tui.select.confirm"`
- `keyText(keybinding)` - 返回某个键位绑定 ID 对应的原始配置按键文本
- `rawKeyHint(key, description)` - 格式化一个原始按键字符串

使用带命名空间的键位绑定 ID:
- Coding-agent 相关 ID 使用 `app.*` 命名空间,例如 `app.tools.expand`、`app.editor.external`、`app.session.rename`
- 共享的 TUI 相关 ID 使用 `tui.*` 命名空间,例如 `tui.select.confirm`、`tui.select.cancel`、`tui.input.tab`

完整的键位绑定 ID 及默认值列表参见 [keybindings.md](keybindings.md)。`keybindings.json` 使用的是相同的带命名空间 ID。

自定义编辑器和 `ctx.ui.custom()` 组件会接收注入的 `keybindings: KeybindingsManager` 参数。它们应当直接使用这个被注入的管理器,而不是调用 `getKeybindings()` 或 `setKeybindings()`。

#### 最佳实践

- 使用内边距为 `(0, 0)` 的 `Text`。默认的 Box 会处理内边距。
- 使用 `\n` 表示多行内容。
- 为流式进度处理 `isPartial`。
- 支持 `expanded` 以按需展示详情。
- 保持默认视图的紧凑性。
- 在 `renderResult` 中读取 `context.args`,而不是把参数复制到 `context.state` 中。
- `context.state` 只用于存放需要在调用插槽和结果插槽之间共享的数据。
- 当同一个组件实例可以原地更新时,复用 `context.lastComponent`。
- 只有当默认的带边框外壳影响到展示效果时,才使用 `renderShell: "self"`。在自绘外壳(self-shell)模式下,工具需要自行负责边框、内边距和背景。

#### 回退行为

如果某个插槽渲染器未定义或抛出异常:
- `renderCall`:显示工具名称
- `renderResult`:显示 `content` 中的原始文本

## 自定义 UI

扩展可以通过 `ctx.ui` 方法与用户交互,并自定义消息/工具的渲染方式。

**关于自定义组件,请参见 [tui.md](tui.md)**,其中包含以下内容的可直接复用的模式:
- 选择对话框(SelectList)
- 带取消功能的异步操作(BorderedLoader)
- 设置开关(SettingsList)
- 状态指示器(setStatus)
- 流式输出期间的工作提示信息、可见性与指示器(`setWorkingMessage`、`setWorkingVisible`、`setWorkingIndicator`)
- 编辑器上方/下方的 widget(setWidget)
- 叠加在内置斜杠命令/路径补全之上的自动补全提供程序(addAutocompleteProvider)
- 自定义 footer(setFooter)

### 对话框

```typescript
// Select from options
const choice = await ctx.ui.select("Pick one:", ["A", "B", "C"]);

// Confirm dialog
const ok = await ctx.ui.confirm("Delete?", "This cannot be undone");

// Text input
const name = await ctx.ui.input("Name:", "placeholder");

// Multi-line editor
const text = await ctx.ui.editor("Edit:", "prefilled text");

// Notification (non-blocking)
ctx.ui.notify("Done!", "info");  // "info" | "warning" | "error"
```

#### 带倒计时的定时对话框

对话框支持 `timeout` 选项,可以带着实时倒计时显示并自动关闭:

```typescript
// Dialog shows "Title (5s)" → "Title (4s)" → ... → auto-dismisses at 0
const confirmed = await ctx.ui.confirm(
  "Timed Confirmation",
  "This dialog will auto-cancel in 5 seconds. Confirm?",
  { timeout: 5000 }
);

if (confirmed) {
  // User confirmed
} else {
  // User cancelled or timed out
}
```

**超时时的返回值:**
- `select()` 返回 `undefined`
- `confirm()` 返回 `false`
- `input()` 返回 `undefined`

#### 使用 AbortSignal 手动关闭

如需更精细的控制(例如区分超时和用户主动取消),可以使用 `AbortSignal`:

```typescript
const controller = new AbortController();
const timeoutId = setTimeout(() => controller.abort(), 5000);

const confirmed = await ctx.ui.confirm(
  "Timed Confirmation",
  "This dialog will auto-cancel in 5 seconds. Confirm?",
  { signal: controller.signal }
);

clearTimeout(timeoutId);

if (confirmed) {
  // User confirmed
} else if (controller.signal.aborted) {
  // Dialog timed out
} else {
  // User cancelled (pressed Escape or selected "No")
}
```

完整示例参见 [examples/extensions/timed-confirm.ts](../examples/extensions/timed-confirm.ts)。

### Widget、状态与 Footer

```typescript
// Status in footer (persistent until cleared)
ctx.ui.setStatus("my-ext", "Processing...");
ctx.ui.setStatus("my-ext", undefined);  // Clear

// Working loader (shown during streaming)
ctx.ui.setWorkingMessage("Thinking deeply...");
ctx.ui.setWorkingMessage();  // Restore default
ctx.ui.setWorkingVisible(false);  // Hide the built-in working loader row entirely
ctx.ui.setWorkingVisible(true);   // Show the built-in working loader row

// Working indicator (shown during streaming)
ctx.ui.setWorkingIndicator({ frames: [ctx.ui.theme.fg("accent", "●")] });  // Static dot
ctx.ui.setWorkingIndicator({
  frames: [
    ctx.ui.theme.fg("dim", "·"),
    ctx.ui.theme.fg("muted", "•"),
    ctx.ui.theme.fg("accent", "●"),
    ctx.ui.theme.fg("muted", "•"),
  ],
  intervalMs: 120,
});
ctx.ui.setWorkingIndicator({ frames: [] });  // Hide indicator
ctx.ui.setWorkingIndicator();  // Restore default spinner

// Widget above editor (default)
ctx.ui.setWidget("my-widget", ["Line 1", "Line 2"]);
// Widget below editor
ctx.ui.setWidget("my-widget", ["Line 1", "Line 2"], { placement: "belowEditor" });
ctx.ui.setWidget("my-widget", (tui, theme) => new Text(theme.fg("accent", "Custom"), 0, 0));
ctx.ui.setWidget("my-widget", undefined);  // Clear

// Custom footer (replaces built-in footer entirely)
ctx.ui.setFooter((tui, theme) => ({
  render(width) { return [theme.fg("dim", "Custom footer")]; },
  invalidate() {},
}));
ctx.ui.setFooter(undefined);  // Restore built-in footer

// Terminal title
ctx.ui.setTitle("pi - my-project");

// Editor text
ctx.ui.setEditorText("Prefill text");
const current = ctx.ui.getEditorText();

// Paste into editor (triggers paste handling, including collapse for large content)
ctx.ui.pasteToEditor("pasted content");

// Stack custom autocomplete behavior on top of the built-in provider
ctx.ui.addAutocompleteProvider((current) => ({
  async getSuggestions(lines, line, col, options) {
    const beforeCursor = (lines[line] ?? "").slice(0, col);
    const match = beforeCursor.match(/(?:^|[ \t])#([^\s#]*)$/);
    if (!match) {
      return current.getSuggestions(lines, line, col, options);
    }

    return {
      prefix: `#${match[1] ?? ""}`,
      items: [{ value: "#2983", label: "#2983", description: "Extension API for autocomplete" }],
    };
  },
  applyCompletion(lines, line, col, item, prefix) {
    return current.applyCompletion(lines, line, col, item, prefix);
  },
  shouldTriggerFileCompletion(lines, line, col) {
    return current.shouldTriggerFileCompletion?.(lines, line, col) ?? true;
  },
}));

// Tool output expansion
const wasExpanded = ctx.ui.getToolsExpanded();
ctx.ui.setToolsExpanded(true);
ctx.ui.setToolsExpanded(wasExpanded);

// Custom editor (vim mode, emacs mode, etc.)
ctx.ui.setEditorComponent((tui, theme, keybindings) => new VimEditor(tui, theme, keybindings));
const currentEditor = ctx.ui.getEditorComponent();
ctx.ui.setEditorComponent((tui, theme, keybindings) =>
  new WrappedEditor(tui, theme, keybindings, currentEditor?.(tui, theme, keybindings))
);
ctx.ui.setEditorComponent(undefined);  // Restore default editor

// Theme management (see themes.md for creating themes)
const themes = ctx.ui.getAllThemes();  // [{ name: "dark", path: "/..." | undefined }, ...]
const lightTheme = ctx.ui.getTheme("light");  // Load without switching
const result = ctx.ui.setTheme("light");  // Switch by name
if (!result.success) {
  ctx.ui.notify(`Failed: ${result.error}`, "error");
}
ctx.ui.setTheme(lightTheme!);  // Or switch by Theme object
ctx.ui.theme.fg("accent", "styled text");  // Access current theme
```

自定义的工作指示器(working-indicator)帧会被原样渲染。如果你想要颜色效果,需要自己给帧字符串添加,例如使用 `ctx.ui.theme.fg(...)`。

### 自动补全提供程序

使用 `ctx.ui.addAutocompleteProvider()` 可以在内置的斜杠命令与路径提供程序之上叠加自定义的自动补全逻辑。

典型模式:

- 检查光标之前的文本
- 当匹配到你的扩展特定语法时,返回你自己的建议
- 否则委托给 `current.getSuggestions(...)`
- 除非需要自定义插入行为,否则委托给 `applyCompletion(...)`

```typescript
pi.on("session_start", (_event, ctx) => {
  ctx.ui.addAutocompleteProvider((current) => ({
    async getSuggestions(lines, cursorLine, cursorCol, options) {
      const line = lines[cursorLine] ?? "";
      const beforeCursor = line.slice(0, cursorCol);
      const match = beforeCursor.match(/(?:^|[ \t])#([^\s#]*)$/);
      if (!match) {
        return current.getSuggestions(lines, cursorLine, cursorCol, options);
      }

      return {
        prefix: `#${match[1] ?? ""}`,
        items: [
          { value: "#2983", label: "#2983", description: "Extension API for registering custom @ autocomplete providers" },
          { value: "#2753", label: "#2753", description: "Reload stale resource settings" },
        ],
      };
    },

    applyCompletion(lines, cursorLine, cursorCol, item, prefix) {
      return current.applyCompletion(lines, cursorLine, cursorCol, item, prefix);
    },

    shouldTriggerFileCompletion(lines, cursorLine, cursorCol) {
      return current.shouldTriggerFileCompletion?.(lines, cursorLine, cursorCol) ?? true;
    },
  }));
});
```

一个完整示例参见 [github-issue-autocomplete.ts](../examples/extensions/github-issue-autocomplete.ts),它使用 `gh issue list` 预加载最新的未关闭 GitHub issue,并在本地对它们进行过滤,以实现快速的 `#...` 补全。该示例需要 GitHub CLI(`gh`)以及一个 GitHub 仓库检出目录。

### 自定义组件

对于复杂 UI,使用 `ctx.ui.custom()`。它会临时将编辑器替换为你的组件,直到调用 `done()` 为止:

```typescript
import { Text, Component } from "@earendil-works/pi-tui";

const result = await ctx.ui.custom<boolean>((tui, theme, keybindings, done) => {
  const text = new Text("Press Enter to confirm, Escape to cancel", 1, 1);

  text.onKey = (key) => {
    if (key === "return") done(true);
    if (key === "escape") done(false);
    return true;
  };

  return text;
});

if (result) {
  // User pressed Enter
}
```

回调函数接收以下参数:
- `tui` - TUI 实例(用于屏幕尺寸、焦点管理)
- `theme` - 当前主题(用于样式)
- `keybindings` - 应用键位绑定管理器(用于检查快捷键)
- `done(value)` - 调用以关闭组件并返回值

完整组件 API 参见 [tui.md](tui.md)。

#### 悬浮层模式(实验性)

传入 `{ overlay: true }` 可以将组件渲染为悬浮在现有内容之上的浮动模态框,而不清空屏幕:

```typescript
const result = await ctx.ui.custom<string | null>(
  (tui, theme, keybindings, done) => new MyOverlayComponent({ onClose: done }),
  { overlay: true }
);
```

对于更高级的定位需求(锚点、边距、百分比、响应式可见性),可以传入 `overlayOptions`。使用 `onHandle` 以编程方式控制可见性:

```typescript
const result = await ctx.ui.custom<string | null>(
  (tui, theme, keybindings, done) => new MyOverlayComponent({ onClose: done }),
  {
    overlay: true,
    overlayOptions: { anchor: "top-right", width: "50%", margin: 2 },
    onHandle: (handle) => { /* handle.setHidden(true/false) */ }
  }
);
```

完整的 `OverlayOptions` API 参见 [tui.md](tui.md),更多示例参见 [overlay-qa-tests.ts](../examples/extensions/overlay-qa-tests.ts)。

### 自定义编辑器

用自定义实现(vim 模式、emacs 模式等)替换主输入编辑器:

```typescript
import { CustomEditor, type ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { matchesKey } from "@earendil-works/pi-tui";

class VimEditor extends CustomEditor {
  private mode: "normal" | "insert" = "insert";

  handleInput(data: string): void {
    if (matchesKey(data, "escape") && this.mode === "insert") {
      this.mode = "normal";
      return;
    }
    if (this.mode === "normal" && data === "i") {
      this.mode = "insert";
      return;
    }
    super.handleInput(data);  // App keybindings + text editing
  }
}

export default function (pi: ExtensionAPI) {
  pi.on("session_start", (_event, ctx) => {
    ctx.ui.setEditorComponent((_tui, theme, keybindings) =>
      new VimEditor(theme, keybindings)
    );
  });
}
```

**要点:**
- 继承 `CustomEditor`(而不是基础的 `Editor`)以获得应用级键位绑定(escape 中止、ctrl+d、模型切换)
- 对于你不处理的按键,调用 `super.handleInput(data)`
- 工厂函数会从应用中接收 `theme` 和 `keybindings`
- 在调用 `setEditorComponent()` 之前先调用 `ctx.ui.getEditorComponent()`,以便包装之前配置好的自定义编辑器
- 传入 `undefined` 可恢复默认编辑器:`ctx.ui.setEditorComponent(undefined)`

要与另一个已经替换过编辑器的扩展组合使用,请在设置你自己的工厂函数之前,先捕获之前的工厂函数:

```typescript
const previous = ctx.ui.getEditorComponent();
ctx.ui.setEditorComponent((tui, theme, keybindings) =>
  new MyEditor(tui, theme, keybindings, { base: previous?.(tui, theme, keybindings) })
);
```

一个带模式指示器的完整示例参见 [tui.md](tui.md) 中的 Pattern 7。

### 消息渲染

为带有你的 `customType` 的消息注册自定义渲染器:

```typescript
import { Text } from "@earendil-works/pi-tui";

pi.registerMessageRenderer("my-extension", (message, options, theme) => {
  const { expanded } = options;
  let text = theme.fg("accent", `[${message.customType}] `);
  text += message.content;

  if (expanded && message.details) {
    text += "\n" + theme.fg("dim", JSON.stringify(message.details, null, 2));
  }

  return new Text(text, 0, 0);
});
```

消息通过 `pi.sendMessage()` 发送:

```typescript
pi.sendMessage({
  customType: "my-extension",  // Matches registerMessageRenderer
  content: "Status update",
  display: true,               // Show in TUI
  details: { ... },            // Available in renderer
});
```

### 主题颜色

所有渲染函数都会接收一个 `theme` 对象。关于创建自定义主题以及完整的调色板,参见 [themes.md](themes.md)。

```typescript
// Foreground colors
theme.fg("toolTitle", text)   // Tool names
theme.fg("accent", text)      // Highlights
theme.fg("success", text)     // Success (green)
theme.fg("error", text)       // Errors (red)
theme.fg("warning", text)     // Warnings (yellow)
theme.fg("muted", text)       // Secondary text
theme.fg("dim", text)         // Tertiary text

// Text styles
theme.bold(text)
theme.italic(text)
theme.strikethrough(text)
```

在自定义工具渲染器中实现语法高亮:

```typescript
import { highlightCode, getLanguageFromPath } from "@earendil-works/pi-coding-agent";

// Highlight code with explicit language
const highlighted = highlightCode("const x = 1;", "typescript", theme);

// Auto-detect language from file path
const lang = getLanguageFromPath("/path/to/file.rs");  // "rust"
const highlighted = highlightCode(code, lang, theme);
```

## 错误处理

- 扩展错误会被记录,agent 继续运行
- `tool_call` 错误会阻止工具执行(fail-safe)
- 工具的 `execute` 错误必须通过抛出异常来发出信号;抛出的错误会被捕获,并以 `isError: true` 报告给 LLM,然后继续执行

## 模式行为

| 模式 | UI 方法 | 说明 |
|------|-----------|-------|
| 交互式(Interactive) | 完整 TUI | 正常操作 |
| RPC(`--mode rpc`) | JSON 协议 | 宿主处理 UI,见 [rpc.md](rpc.md) |
| JSON(`--mode json`) | 无操作 | 事件流输出到 stdout,见 [json.md](json.md) |
| Print(`-p`) | 无操作 | 扩展会运行,但无法弹出提示 |

在非交互模式下,使用 UI 方法之前请检查 `ctx.hasUI`。

## 示例参考

所有示例都在 [examples/extensions/](../examples/extensions/) 中。

| 示例 | 说明 | 关键 API |
|---------|-------------|----------|
| **工具(Tools)** |||
| `hello.ts` | 最简工具注册 | `registerTool` |
| `question.ts` | 带用户交互的工具 | `registerTool`, `ui.select` |
| `questionnaire.ts` | 多步骤向导工具 | `registerTool`, `ui.custom` |
| `todo.ts` | 带持久化的有状态工具 | `registerTool`, `appendEntry`, `renderResult`, session events |
| `dynamic-tools.ts` | 启动后及命令执行期间注册工具 | `registerTool`, `session_start`, `registerCommand` |
| `structured-output.ts` | 带 `terminate: true` 的最终结构化输出工具 | `registerTool`, terminating tool results |
| `truncated-tool.ts` | 输出截断示例 | `registerTool`, `truncateHead` |
| `tool-override.ts` | 覆盖内置 read 工具 | `registerTool`(与内置工具同名) |
| **命令(Commands)** |||
| `pirate.ts` | 每回合修改系统提示词 | `registerCommand`, `before_agent_start` |
| `summarize.ts` | 对话摘要命令 | `registerCommand`, `ui.custom` |
| `handoff.ts` | 跨 provider 模型交接 | `registerCommand`, `ui.editor`, `ui.custom` |
| `qna.ts` | 带自定义 UI 的问答 | `registerCommand`, `ui.custom`, `setEditorText` |
| `send-user-message.ts` | 注入用户消息 | `registerCommand`, `sendUserMessage` |
| `reload-runtime.ts` | 重载命令与 LLM 工具交接 | `registerCommand`, `ctx.reload()`, `sendUserMessage` |
| `shutdown-command.ts` | 优雅关闭命令 | `registerCommand`, `shutdown()` |
| **事件与门控(Events & Gates)** |||
| `permission-gate.ts` | 阻止危险命令 | `on("tool_call")`, `ui.confirm` |
| `protected-paths.ts` | 阻止写入特定路径 | `on("tool_call")` |
| `confirm-destructive.ts` | 确认会话变更 | `on("session_before_switch")`, `on("session_before_fork")` |
| `dirty-repo-guard.ts` | 对未提交的 git 仓库发出警告 | `on("session_before_*")`, `exec` |
| `input-transform.ts` | 转换用户输入 | `on("input")` |
| `model-status.ts` | 响应模型变更 | `on("model_select")`, `setStatus` |
| `provider-payload.ts` | 检查请求负载与 provider 响应头 | `on("before_provider_request")`, `on("after_provider_response")` |
| `system-prompt-header.ts` | 显示系统提示词信息 | `on("agent_start")`, `getSystemPrompt` |
| `claude-rules.ts` | 从文件加载规则 | `on("session_start")`, `on("before_agent_start")` |
| `prompt-customizer.ts` | 使用 `systemPromptOptions` 添加上下文相关的工具指导 | `on("before_agent_start")`, `BuildSystemPromptOptions` |
| `file-trigger.ts` | 文件监听触发消息 | `sendMessage` |
| **压缩与会话(Compaction & Sessions)** |||
| `custom-compaction.ts` | 自定义压缩摘要 | `on("session_before_compact")` |
| `trigger-compact.ts` | 手动触发压缩 | `compact()` |
| `git-checkpoint.ts` | 回合时进行 git stash | `on("turn_start")`, `on("session_before_fork")`, `exec` |
| `auto-commit-on-exit.ts` | 退出时提交 | `on("session_shutdown")`, `exec` |
| **UI 组件** |||
| `status-line.ts` | 页脚状态指示器 | `setStatus`, session events |
| `working-indicator.ts` | 自定义流式工作指示器 | `setWorkingIndicator`, `registerCommand` |
| `github-issue-autocomplete.ts` | 通过预加载 `gh issue list` 中最近打开的 issue,在内置自动补全基础上添加 `#1234` issue 补全 | `addAutocompleteProvider`, `on("session_start")`, `exec` |
| `custom-footer.ts` | 完全替换页脚 | `registerCommand`, `setFooter` |
| `custom-header.ts` | 替换启动头部 | `on("session_start")`, `setHeader` |
| `modal-editor.ts` | Vim 风格模式编辑器 | `setEditorComponent`, `CustomEditor` |
| `rainbow-editor.ts` | 自定义编辑器样式 | `setEditorComponent` |
| `widget-placement.ts` | 编辑器上方/下方的 widget | `setWidget` |
| `overlay-test.ts` | 悬浮层组件 | `ui.custom` with overlay options |
| `overlay-qa-tests.ts` | 全面的悬浮层测试 | `ui.custom`, all overlay options |
| `notify.ts` | 简单通知 | `ui.notify` |
| `timed-confirm.ts` | 带超时的对话框 | `ui.confirm` with timeout/signal |
| `mac-system-theme.ts` | 自动切换主题 | `setTheme`, `exec` |
| **复杂扩展(Complex Extensions)** |||
| `plan-mode/` | 完整的计划模式实现 | All event types, `registerCommand`, `registerShortcut`, `registerFlag`, `setStatus`, `setWidget`, `sendMessage`, `setActiveTools` |
| `preset.ts` | 可保存的预设(模型、工具、思考等级) | `registerCommand`, `registerShortcut`, `registerFlag`, `setModel`, `setActiveTools`, `setThinkingLevel`, `appendEntry` |
| `tools.ts` | 切换工具开关的 UI | `registerCommand`, `setActiveTools`, `SettingsList`, session events |
| **远程与沙箱(Remote & Sandbox)** |||
| `ssh.ts` | SSH 远程执行 | `registerFlag`, `on("user_bash")`, `on("before_agent_start")`, tool operations |
| `interactive-shell.ts` | 持久化 shell 会话 | `on("user_bash")` |
| `sandbox/` | 沙箱化的工具执行 | Tool operations |
| `subagent/` | 生成子 agent | `registerTool`, `exec` |
| **游戏(Games)** |||
| `snake.ts` | 贪吃蛇游戏 | `registerCommand`, `ui.custom`, keyboard handling |
| `space-invaders.ts` | 太空侵略者游戏 | `registerCommand`, `ui.custom` |
| `doom-overlay/` | 悬浮层中的 Doom | `ui.custom` with overlay |
| **Provider** |||
| `custom-provider-anthropic/` | 自定义 Anthropic 代理 | `registerProvider` |
| `custom-provider-gitlab-duo/` | GitLab Duo 集成 | `registerProvider` with OAuth |
| **消息与通信(Messages & Communication)** |||
| `message-renderer.ts` | 自定义消息渲染 | `registerMessageRenderer`, `sendMessage` |
| `event-bus.ts` | 扩展间事件通信 | `pi.events` |
| **会话元数据(Session Metadata)** |||
| `session-name.ts` | 为会话选择器命名会话 | `setSessionName`, `getSessionName` |
| `bookmark.ts` | 为 /tree 标记条目 | `setLabel` |
| **杂项(Misc)** |||
| `inline-bash.ts` | 工具调用中的内联 bash | `on("tool_call")` |
| `bash-spawn-hook.ts` | 在执行前调整 bash 命令、cwd 和 env | `createBashTool`, `spawnHook` |
| `with-deps/` | 带 npm 依赖的扩展 | Package structure with `package.json` |
