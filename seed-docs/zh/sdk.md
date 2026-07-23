> pi 可以帮助你使用该 SDK。让它为你的用例构建一个集成即可。

# SDK

该 SDK 提供了对 pi 代理能力的编程访问接口。你可以用它将 pi 嵌入其他应用程序、构建自定义界面,或与自动化工作流集成。

**典型使用场景:**
- 构建自定义 UI(网页、桌面、移动端)
- 将代理能力集成到现有应用程序中
- 创建具备代理推理能力的自动化流水线
- 构建可派生子代理(sub-agent)的自定义工具
- 以编程方式测试代理行为

完整的示例代码(从最简单到完全可控)请参见 [examples/sdk/](../examples/sdk/)。

## 快速开始

```typescript
import { AuthStorage, createAgentSession, ModelRegistry, SessionManager } from "@earendil-works/pi-coding-agent";

// 设置凭据存储和模型注册表
const authStorage = AuthStorage.create();
const modelRegistry = ModelRegistry.create(authStorage);

const { session } = await createAgentSession({
  sessionManager: SessionManager.inMemory(),
  authStorage,
  modelRegistry,
});

session.subscribe((event) => {
  if (event.type === "message_update" && event.assistantMessageEvent.type === "text_delta") {
    process.stdout.write(event.assistantMessageEvent.delta);
  }
});

await session.prompt("What files are in the current directory?");
```

## 安装

```bash
npm install @earendil-works/pi-coding-agent
```

该 SDK 已包含在主包中,无需单独安装。

## 核心概念

### createAgentSession()

用于创建单个 `AgentSession` 的主工厂函数。

`createAgentSession()` 使用 `ResourceLoader` 来提供扩展、技能(skill)、提示词模板、主题以及上下文文件。如果你不提供该参数,它会使用带有标准发现机制的 `DefaultResourceLoader`。

```typescript
import { createAgentSession, SessionManager } from "@earendil-works/pi-coding-agent";

// 最简用法:使用 DefaultResourceLoader 的默认配置
const { session } = await createAgentSession();

// 自定义用法:覆盖指定选项
const { session } = await createAgentSession({
  model: myModel,
  tools: ["read", "bash"],
  sessionManager: SessionManager.inMemory(),
});
```

### AgentSession

该会话对象负责管理代理的生命周期、消息历史、模型状态、上下文压缩(compaction)以及事件流。

```typescript
interface AgentSession {
  // 发送一条提示词并等待完成
  prompt(text: string, options?: PromptOptions): Promise<void>;

  // 在流式输出期间排队消息
  steer(text: string): Promise<void>;
  followUp(text: string): Promise<void>;

  // 订阅事件(返回取消订阅函数)
  subscribe(listener: (event: AgentSessionEvent) => void): () => void;

  // 会话信息
  sessionFile: string | undefined;
  sessionId: string;

  // 模型控制
  setModel(model: Model): Promise<void>;
  setThinkingLevel(level: ThinkingLevel): void;
  cycleModel(): Promise<ModelCycleResult | undefined>;
  cycleThinkingLevel(): ThinkingLevel | undefined;

  // 状态访问
  agent: Agent;
  model: Model | undefined;
  thinkingLevel: ThinkingLevel;
  messages: AgentMessage[];
  isStreaming: boolean;

  // 在当前会话文件内进行原地树形导航
  navigateTree(targetId: string, options?: { summarize?: boolean; customInstructions?: string; replaceInstructions?: boolean; label?: string }): Promise<{ editorText?: string; cancelled: boolean }>;

  // 上下文压缩
  compact(customInstructions?: string): Promise<CompactionResult>;
  abortCompaction(): void;

  // 中止当前操作
  abort(): Promise<void>;

  // 清理资源
  dispose(): void;
}
```

诸如新建会话(new-session)、恢复(resume)、分叉(fork)、导入(import)等会话替换类 API 位于 `AgentSessionRuntime` 上,而非 `AgentSession` 上。

### createAgentSessionRuntime() 与 AgentSessionRuntime

当你需要替换活动会话并重建与工作目录(cwd)绑定的运行时状态时,请使用 runtime API。这与内置的交互模式(interactive)、打印模式(print)和 RPC 模式所使用的是同一层。

`createAgentSessionRuntime()` 接受一个 runtime 工厂函数以及初始的 cwd/会话目标。该工厂函数会封闭(closes over)进程级全局固定输入,为有效的 cwd 重新创建与 cwd 绑定的服务,针对这些服务解析会话选项,并返回一个完整的 runtime 结果。

```typescript
import {
  type CreateAgentSessionRuntimeFactory,
  createAgentSessionFromServices,
  createAgentSessionRuntime,
  createAgentSessionServices,
  getAgentDir,
  SessionManager,
} from "@earendil-works/pi-coding-agent";

const createRuntime: CreateAgentSessionRuntimeFactory = async ({ cwd, sessionManager, sessionStartEvent }) => {
  const services = await createAgentSessionServices({ cwd });
  return {
    ...(await createAgentSessionFromServices({
      services,
      sessionManager,
      sessionStartEvent,
    })),
    services,
    diagnostics: services.diagnostics,
  };
};

const runtime = await createAgentSessionRuntime(createRuntime, {
  cwd: process.cwd(),
  agentDir: getAgentDir(),
  sessionManager: SessionManager.create(process.cwd()),
});
```

`AgentSessionRuntime` 负责在以下操作中替换当前活动的 runtime:

- `newSession()`
- `switchSession()`
- `fork()`
- 通过 `fork(entryId, { position: "at" })` 实现的克隆(clone)流程
- `importFromJsonl()`

重要行为说明:

- 在上述操作之后,`runtime.session` 会发生变化
- 事件订阅是绑定到特定 `AgentSession` 实例的,因此替换后需要重新订阅
- 如果你使用了扩展,需要针对新会话再次调用 `runtime.session.bindExtensions(...)`
- 创建过程会将诊断信息返回到 `runtime.diagnostics` 上
- 如果 runtime 的创建或替换失败,该方法会抛出异常,由调用方决定如何处理

```typescript
let session = runtime.session;
let unsubscribe = session.subscribe(() => {});

await runtime.newSession();

unsubscribe();
session = runtime.session;
unsubscribe = session.subscribe(() => {});
```

### 提示词发送与消息排队

`PromptOptions` 控制提示词的展开方式、流式输出期间的排队行为,以及提示词预检(preflight)通知:

```typescript
interface PromptOptions {
  expandPromptTemplates?: boolean;
  images?: ImageContent[];
  streamingBehavior?: "steer" | "followUp";
  source?: InputSource;
  preflightResult?: (success: boolean) => void;
}
```

`preflightResult` 在每次调用 `prompt()` 时会被调用一次:

- 当提示词被接受、排队或立即处理时,值为 `true`
- 当提示词预检在被接受之前就被拒绝时,值为 `false`

它会在 `prompt()` 返回(resolve)之前触发。而 `prompt()` 只有在整个被接受的运行(包括重试)完全结束后才会返回。被接受之后发生的失败会通过常规的事件和消息流上报,而不会通过 `preflightResult(false)` 上报。

`prompt()` 方法负责处理提示词模板、扩展命令以及消息发送:

```typescript
// 基本提示词(非流式状态下)
await session.prompt("What files are here?");

// 携带图片
await session.prompt("What's in this image?", {
  images: [{ type: "image", source: { type: "base64", mediaType: "image/png", data: "..." } }]
});

// 在流式输出期间:必须指定消息应如何排队
await session.prompt("Stop and do this instead", { streamingBehavior: "steer" });
await session.prompt("After you're done, also check X", { streamingBehavior: "followUp" });
```

**行为说明:**
- **扩展命令**(例如 `/mycommand`):即使在流式输出期间也会立即执行。它们通过 `pi.sendMessage()` 自行管理与 LLM 的交互。
- **基于文件的提示词模板**(来自 `.md` 文件):在发送或排队之前会被展开为其文件内容。
- **在流式输出期间未指定 `streamingBehavior`**:会抛出错误。此时应直接使用 `steer()` 或 `followUp()`,或者显式指定该选项。
- **`preflightResult(true)`**:表示提示词已被接受、排队或立即处理。
- **`preflightResult(false)`**:表示预检在被接受之前就被拒绝。

若需要在流式输出期间显式排队消息:

```typescript
// 排队一条引导(steering)消息,在当前助手轮次完成其工具调用后投递
await session.steer("New instruction");

// 等待代理完成(仅在代理停止时投递)
await session.followUp("After you're done, also do this");
```

`steer()` 和 `followUp()` 都会展开基于文件的提示词模板,但如果传入扩展命令则会报错(扩展命令不能被排队)。

### Agent 与 AgentState

`Agent` 类(来自 `@earendil-works/pi-agent-core`)负责处理核心的 LLM 交互。你可以通过 `session.agent` 访问它。

```typescript
// 访问当前状态
const state = session.agent.state;

// state.messages: AgentMessage[] - 对话历史
// state.model: Model - 当前模型
// state.thinkingLevel: ThinkingLevel - 当前思考级别
// state.systemPrompt: string - 系统提示词
// state.tools: AgentTool[] - 可用工具
// state.streamingMessage?: AgentMessage - 当前部分生成的助手消息
// state.errorMessage?: string - 最新的助手错误信息

// 替换消息(适用于分支或恢复场景)
session.agent.state.messages = messages; // 复制顶层数组

// 替换工具
session.agent.state.tools = tools; // 复制顶层数组

// 等待代理处理完成
await session.agent.waitForIdle();
```

### 事件

订阅事件以接收流式输出和生命周期通知。

```typescript
session.subscribe((event) => {
  switch (event.type) {
    // 来自助手的流式文本
    case "message_update":
      if (event.assistantMessageEvent.type === "text_delta") {
        process.stdout.write(event.assistantMessageEvent.delta);
      }
      if (event.assistantMessageEvent.type === "thinking_delta") {
        // 思考过程输出(若已启用思考功能)
      }
      break;
    
    // 工具执行
    case "tool_execution_start":
      console.log(`Tool: ${event.toolName}`);
      break;
    case "tool_execution_update":
      // 流式工具输出
      break;
    case "tool_execution_end":
      console.log(`Result: ${event.isError ? "error" : "success"}`);
      break;
    
    // 消息生命周期
    case "message_start":
      // 新消息开始
      break;
    case "message_end":
      // 消息完成
      break;
    
    // 代理生命周期
    case "agent_start":
      // 代理开始处理提示词
      break;
    case "agent_end":
      // 代理完成(event.messages 包含新生成的消息)
      break;
    
    // 轮次生命周期(一次 LLM 响应 + 工具调用)
    case "turn_start":
      break;
    case "turn_end":
      // event.message: 助手的响应
      // event.toolResults: 本轮的工具调用结果
      break;
    
    // 会话事件(队列、压缩、重试)
    case "queue_update":
      console.log(event.steering, event.followUp);
      break;
    case "compaction_start":
    case "compaction_end":
    case "auto_retry_start":
    case "auto_retry_end":
      break;
  }
});
```

## 选项参考

### 目录

```typescript
const { session } = await createAgentSession({
  // 供 DefaultResourceLoader 发现资源使用的工作目录
  cwd: process.cwd(), // 默认值
  
  // 全局配置目录
  agentDir: "~/.pi/agent", // 默认值(会展开 ~)
});
```

`DefaultResourceLoader` 使用 `cwd` 来查找:
- 项目扩展(`.pi/extensions/`)
- 项目技能(skill):
  - `.pi/skills/`
  - `cwd` 及其上级目录(直到 git 仓库根目录,若不在仓库内则直到文件系统根目录)中的 `.agents/skills/`
- 项目提示词(`.pi/prompts/`)
- 上下文文件(从 cwd 向上查找的 `AGENTS.md`)
- 会话目录命名

`DefaultResourceLoader` 使用 `agentDir` 来查找:
- 全局扩展(`extensions/`)
- 全局技能(skill):
  - `agentDir` 下的 `skills/`(例如 `~/.pi/agent/skills/`)
  - `~/.agents/skills/`
- 全局提示词(`prompts/`)
- 全局上下文文件(`AGENTS.md`)
- 配置(`settings.json`)
- 自定义模型(`models.json`)
- 凭据(`auth.json`)
- 会话(`sessions/`)

当你传入自定义的 `ResourceLoader` 时,`cwd` 和 `agentDir` 将不再控制资源发现,但仍会影响会话命名和工具路径解析。

### 模型

```typescript
import { getModel } from "@earendil-works/pi-ai";
import { AuthStorage, ModelRegistry } from "@earendil-works/pi-coding-agent";

const authStorage = AuthStorage.create();
const modelRegistry = ModelRegistry.create(authStorage);

// 查找特定的内置模型(不检查是否存在 API 密钥)
const opus = getModel("anthropic", "claude-opus-4-5");
if (!opus) throw new Error("Model not found");

// 按 provider/id 查找任意模型,包括来自 models.json 的自定义模型
// (不检查是否存在 API 密钥)
const customModel = modelRegistry.find("my-provider", "my-model");

// 仅获取已配置有效 API 密钥的模型
const available = await modelRegistry.getAvailable();

const { session } = await createAgentSession({
  model: opus,
  thinkingLevel: "medium", // off、minimal、low、medium、high、xhigh
  
  // 用于循环切换的模型(交互模式下 Ctrl+P)
  scopedModels: [
    { model: opus, thinkingLevel: "high" },
    { model: haiku, thinkingLevel: "off" },
  ],
  
  authStorage,
  modelRegistry,
});
```

若未提供模型,系统会依次尝试:
1. 从会话中恢复(如果是继续已有会话)
2. 使用配置中的默认值
3. 回退到第一个可用模型

> 参见 [examples/sdk/02-custom-model.ts](../examples/sdk/02-custom-model.ts)

### API 密钥与 OAuth

API 密钥的解析优先级(由 AuthStorage 处理):
1. 运行时覆盖值(通过 `setRuntimeApiKey` 设置,不持久化)
2. 存储在 `auth.json` 中的凭据(API 密钥或 OAuth 令牌)
3. 环境变量(`ANTHROPIC_API_KEY`、`OPENAI_API_KEY` 等)
4. 回退解析器(用于 `models.json` 中自定义 provider 的密钥)

```typescript
import { AuthStorage, ModelRegistry } from "@earendil-works/pi-coding-agent";

// 默认:使用 ~/.pi/agent/auth.json 和 ~/.pi/agent/models.json
const authStorage = AuthStorage.create();
const modelRegistry = ModelRegistry.create(authStorage);

const { session } = await createAgentSession({
  sessionManager: SessionManager.inMemory(),
  authStorage,
  modelRegistry,
});

// 运行时 API 密钥覆盖(不持久化到磁盘)
authStorage.setRuntimeApiKey("anthropic", "sk-my-temp-key");

// 自定义的凭据存储位置
const customAuth = AuthStorage.create("/my/app/auth.json");
const customRegistry = ModelRegistry.create(customAuth, "/my/app/models.json");

const { session } = await createAgentSession({
  sessionManager: SessionManager.inMemory(),
  authStorage: customAuth,
  modelRegistry: customRegistry,
});

// 不使用自定义的 models.json(仅内置模型)
const simpleRegistry = ModelRegistry.inMemory(authStorage);
```

> 参见 [examples/sdk/09-api-keys-and-oauth.ts](../examples/sdk/09-api-keys-and-oauth.ts)

### 系统提示词

使用 `ResourceLoader` 来覆盖系统提示词:

```typescript
import { createAgentSession, DefaultResourceLoader } from "@earendil-works/pi-coding-agent";

const loader = new DefaultResourceLoader({
  systemPromptOverride: () => "You are a helpful assistant.",
});
await loader.reload();

const { session } = await createAgentSession({ resourceLoader: loader });
```

> 参见 [examples/sdk/03-custom-prompt.ts](../examples/sdk/03-custom-prompt.ts)

### 工具

指定要启用哪些内置工具:

- 内置工具名称:`read`、`bash`、`edit`、`write`、`grep`、`find`、`ls`
- 默认启用的内置工具:`read`、`bash`、`edit`、`write`
- `noTools: "all"` 会禁用所有工具
- `noTools: "builtin"` 会禁用默认内置工具,但保留扩展工具和自定义工具的启用状态

```typescript
import { createAgentSession } from "@earendil-works/pi-coding-agent";

// 只读模式
const { session } = await createAgentSession({
  tools: ["read", "grep", "find", "ls"],
});

// 选择特定工具
const { session } = await createAgentSession({
  tools: ["read", "bash", "grep"],
});
```

#### 自定义 cwd 下的工具

当你传入自定义的 `cwd` 时,`createAgentSession()` 会针对该 cwd 构建所选的内置工具。

```typescript
import { createAgentSession, SessionManager } from "@earendil-works/pi-coding-agent";

const cwd = "/path/to/project";

// 在自定义 cwd 下使用默认工具
const { session } = await createAgentSession({
  cwd,
  sessionManager: SessionManager.inMemory(cwd),
});

// 或在自定义 cwd 下选择特定工具
const { session } = await createAgentSession({
  cwd,
  tools: ["read", "bash", "grep"],
  sessionManager: SessionManager.inMemory(cwd),
});
```

> 参见 [examples/sdk/05-tools.ts](../examples/sdk/05-tools.ts)

### 自定义工具

```typescript
import { Type } from "typebox";
import { createAgentSession, defineTool } from "@earendil-works/pi-coding-agent";

// 内联定义的自定义工具
const myTool = defineTool({
  name: "my_tool",
  label: "My Tool",
  description: "Does something useful",
  parameters: Type.Object({
    input: Type.String({ description: "Input value" }),
  }),
  execute: async (_toolCallId, params) => ({
    content: [{ type: "text", text: `Result: ${params.input}` }],
    details: {},
  }),
});

// 直接传入自定义工具
const { session } = await createAgentSession({
  customTools: [myTool],
});
```

对于独立定义的工具及类似 `customTools: [myTool]` 这样的数组,请使用 `defineTool()`。内联的 `pi.registerTool({ ... })` 已经能够正确推断参数类型。

通过 `customTools` 传入的自定义工具会与扩展注册的工具合并。由 ResourceLoader 加载的扩展也可以通过 `pi.registerTool()` 注册工具。

如果你传入了 `tools`,需要将每个想要启用的自定义工具或扩展工具名称都包含进去,例如 `tools: ["read", "bash", "my_tool"]`。

> 参见 [examples/sdk/05-tools.ts](../examples/sdk/05-tools.ts)

### 扩展

扩展由 `ResourceLoader` 加载。`DefaultResourceLoader` 会从 `~/.pi/agent/extensions/`、`.pi/extensions/` 以及 settings.json 中配置的扩展来源发现扩展。

```typescript
import { createAgentSession, DefaultResourceLoader } from "@earendil-works/pi-coding-agent";

const loader = new DefaultResourceLoader({
  additionalExtensionPaths: ["/path/to/my-extension.ts"],
  extensionFactories: [
    (pi) => {
      pi.on("agent_start", () => {
        console.log("[Inline Extension] Agent starting");
      });
    },
  ],
});
await loader.reload();

const { session } = await createAgentSession({ resourceLoader: loader });
```

扩展可以注册工具、订阅事件、添加命令等等。完整 API 请参见 [extensions.md](extensions.md)。

**事件总线(Event Bus):** 扩展之间可以通过 `pi.events` 进行通信。如果你需要从外部发出或监听事件,可以向 `DefaultResourceLoader` 传入一个共享的 `eventBus`:

```typescript
import { createEventBus, DefaultResourceLoader } from "@earendil-works/pi-coding-agent";

const eventBus = createEventBus();
const loader = new DefaultResourceLoader({
  eventBus,
});
await loader.reload();

eventBus.on("my-extension:status", (data) => console.log(data));
```

> 参见 [examples/sdk/06-extensions.ts](../examples/sdk/06-extensions.ts) 和 [docs/extensions.md](extensions.md)

### 技能(Skills)

```typescript
import {
  createAgentSession,
  DefaultResourceLoader,
  type Skill,
} from "@earendil-works/pi-coding-agent";

const customSkill: Skill = {
  name: "my-skill",
  description: "Custom instructions",
  filePath: "/path/to/SKILL.md",
  baseDir: "/path/to",
  source: "custom",
};

const loader = new DefaultResourceLoader({
  skillsOverride: (current) => ({
    skills: [...current.skills, customSkill],
    diagnostics: current.diagnostics,
  }),
});
await loader.reload();

const { session } = await createAgentSession({ resourceLoader: loader });
```

> 参见 [examples/sdk/04-skills.ts](../examples/sdk/04-skills.ts)

### 上下文文件

```typescript
import { createAgentSession, DefaultResourceLoader } from "@earendil-works/pi-coding-agent";

const loader = new DefaultResourceLoader({
  agentsFilesOverride: (current) => ({
    agentsFiles: [
      ...current.agentsFiles,
      { path: "/virtual/AGENTS.md", content: "# Guidelines\n\n- Be concise" },
    ],
  }),
});
await loader.reload();

const { session } = await createAgentSession({ resourceLoader: loader });
```

> 参见 [examples/sdk/07-context-files.ts](../examples/sdk/07-context-files.ts)

### 斜杠命令(Slash Commands)

```typescript
import {
  createAgentSession,
  DefaultResourceLoader,
  type PromptTemplate,
} from "@earendil-works/pi-coding-agent";

const customCommand: PromptTemplate = {
  name: "deploy",
  description: "Deploy the application",
  source: "(custom)",
  content: "# Deploy\n\n1. Build\n2. Test\n3. Deploy",
};

const loader = new DefaultResourceLoader({
  promptsOverride: (current) => ({
    prompts: [...current.prompts, customCommand],
    diagnostics: current.diagnostics,
  }),
});
await loader.reload();

const { session } = await createAgentSession({ resourceLoader: loader });
```

> 参见 [examples/sdk/08-prompt-templates.ts](../examples/sdk/08-prompt-templates.ts)

### 会话管理

会话使用带有 `id`/`parentId` 链接关系的树形结构,支持原地分支(branching)。

```typescript
import {
  type CreateAgentSessionRuntimeFactory,
  createAgentSession,
  createAgentSessionFromServices,
  createAgentSessionRuntime,
  createAgentSessionServices,
  getAgentDir,
  SessionManager,
} from "@earendil-works/pi-coding-agent";

// 内存模式(不持久化)
const { session } = await createAgentSession({
  sessionManager: SessionManager.inMemory(),
});

// 新建持久化会话
const { session: persisted } = await createAgentSession({
  sessionManager: SessionManager.create(process.cwd()),
});

// 继续最近的会话
const { session: continued, modelFallbackMessage } = await createAgentSession({
  sessionManager: SessionManager.continueRecent(process.cwd()),
});
if (modelFallbackMessage) {
  console.log("Note:", modelFallbackMessage);
}

// 打开指定文件
const { session: opened } = await createAgentSession({
  sessionManager: SessionManager.open("/path/to/session.jsonl"),
});

// 列出会话
const currentProjectSessions = await SessionManager.list(process.cwd());
const allSessions = await SessionManager.listAll(process.cwd());

// 用于 /new、/resume、/fork、/clone 以及导入流程的会话替换 API。
const createRuntime: CreateAgentSessionRuntimeFactory = async ({ cwd, sessionManager, sessionStartEvent }) => {
  const services = await createAgentSessionServices({ cwd });
  return {
    ...(await createAgentSessionFromServices({
      services,
      sessionManager,
      sessionStartEvent,
    })),
    services,
    diagnostics: services.diagnostics,
  };
};

const runtime = await createAgentSessionRuntime(createRuntime, {
  cwd: process.cwd(),
  agentDir: getAgentDir(),
  sessionManager: SessionManager.create(process.cwd()),
});

// 用一个全新的会话替换当前活动会话
await runtime.newSession();

// 用另一个已保存的会话替换当前活动会话
await runtime.switchSession("/path/to/session.jsonl");

// 从指定的用户条目分叉出一个新会话来替换当前活动会话
await runtime.fork("entry-id");

// 通过指定条目克隆当前活动路径
await runtime.fork("entry-id", { position: "at" });
```

**SessionManager 树形 API:**

```typescript
const sm = SessionManager.open("/path/to/session.jsonl");

// 会话列表
const currentProjectSessions = await SessionManager.list(process.cwd());
const allSessions = await SessionManager.listAll(process.cwd());

// 树遍历
const entries = sm.getEntries();        // 所有条目(不含头部)
const tree = sm.getTree();              // 完整的树结构
const path = sm.getPath();              // 从根节点到当前叶节点的路径
const leaf = sm.getLeafEntry();         // 当前叶节点条目
const entry = sm.getEntry(id);          // 根据 ID 获取条目
const children = sm.getChildren(id);    // 某条目的直接子节点

// 标签
const label = sm.getLabel(id);          // 获取某条目的标签
sm.appendLabelChange(id, "checkpoint"); // 设置标签

// 分支
sm.branch(entryId);                     // 将叶节点移动到更早的条目
sm.branchWithSummary(id, "Summary...");  // 携带上下文摘要进行分支
sm.createBranchedSession(leafId);       // 将路径提取为新文件
```

> 参见 [examples/sdk/11-sessions.ts](../examples/sdk/11-sessions.ts) 和 [会话格式文档](session-format.md)

### 配置管理

```typescript
import { createAgentSession, SettingsManager, SessionManager } from "@earendil-works/pi-coding-agent";

// 默认:从文件加载(全局与项目级配置合并)
const { session } = await createAgentSession({
  settingsManager: SettingsManager.create(),
});

// 携带覆盖项
const settingsManager = SettingsManager.create();
settingsManager.applyOverrides({
  compaction: { enabled: false },
  retry: { enabled: true, maxRetries: 5 },
});
const { session } = await createAgentSession({ settingsManager });

// 内存模式(无文件 I/O,适用于测试)
const { session } = await createAgentSession({
  settingsManager: SettingsManager.inMemory({ compaction: { enabled: false } }),
  sessionManager: SessionManager.inMemory(),
});

// 自定义目录
const { session } = await createAgentSession({
  settingsManager: SettingsManager.create("/custom/cwd", "/custom/agent"),
});
```

**静态工厂方法:**
- `SettingsManager.create(cwd?, agentDir?)` - 从文件加载
- `SettingsManager.inMemory(settings?)` - 无文件 I/O

**项目专属配置:**

配置会从以下两个位置加载并合并:
1. 全局:`~/.pi/agent/settings.json`
2. 项目:`<cwd>/.pi/settings.json`

项目配置会覆盖全局配置。嵌套对象按键合并。默认情况下,setter 修改的是全局配置。

**持久化与错误处理语义:**

- 对于内存态数据,配置的 getter/setter 都是同步的。
- setter 会异步地将持久化写入操作加入队列。
- 当你需要确保数据已落盘时(例如在进程退出前,或在测试中断言文件内容前),请调用 `await settingsManager.flush()`。
- `SettingsManager` 不会自行打印配置 I/O 错误。请使用 `settingsManager.drainErrors()` 获取错误并在应用层进行上报。

> 参见 [examples/sdk/10-settings.ts](../examples/sdk/10-settings.ts)

## ResourceLoader

使用 `DefaultResourceLoader` 来发现扩展、技能、提示词、主题以及上下文文件。

```typescript
import {
  DefaultResourceLoader,
  getAgentDir,
} from "@earendil-works/pi-coding-agent";

const loader = new DefaultResourceLoader({
  cwd,
  agentDir: getAgentDir(),
});
await loader.reload();

const extensions = loader.getExtensions();
const skills = loader.getSkills();
const prompts = loader.getPrompts();
const themes = loader.getThemes();
const contextFiles = loader.getAgentsFiles().agentsFiles;
```

## 返回值

`createAgentSession()` 返回:

```typescript
interface CreateAgentSessionResult {
  // 会话对象
  session: AgentSession;
  
  // 扩展加载结果(用于运行器设置)
  extensionsResult: LoadExtensionsResult;
  
  // 若会话模型无法恢复,则包含警告信息
  modelFallbackMessage?: string;
}

interface LoadExtensionsResult {
  extensions: Extension[];
  errors: Array<{ path: string; error: string }>;
  runtime: ExtensionRuntime;
}
```

## 完整示例

```typescript
import { getModel } from "@earendil-works/pi-ai";
import { Type } from "typebox";
import {
  AuthStorage,
  createAgentSession,
  DefaultResourceLoader,
  defineTool,
  ModelRegistry,
  SessionManager,
  SettingsManager,
} from "@earendil-works/pi-coding-agent";

// 设置凭据存储(自定义位置)
const authStorage = AuthStorage.create("/custom/agent/auth.json");

// 运行时 API 密钥覆盖(不持久化)
if (process.env.MY_KEY) {
  authStorage.setRuntimeApiKey("anthropic", process.env.MY_KEY);
}

// 模型注册表(不使用自定义 models.json)
const modelRegistry = ModelRegistry.create(authStorage);

// 内联工具
const statusTool = defineTool({
  name: "status",
  label: "Status",
  description: "Get system status",
  parameters: Type.Object({}),
  execute: async () => ({
    content: [{ type: "text", text: `Uptime: ${process.uptime()}s` }],
    details: {},
  }),
});

const model = getModel("anthropic", "claude-opus-4-5");
if (!model) throw new Error("Model not found");

// 携带覆盖项的内存配置
const settingsManager = SettingsManager.inMemory({
  compaction: { enabled: false },
  retry: { enabled: true, maxRetries: 2 },
});

const loader = new DefaultResourceLoader({
  cwd: process.cwd(),
  agentDir: "/custom/agent",
  settingsManager,
  systemPromptOverride: () => "You are a minimal assistant. Be concise.",
});
await loader.reload();

const { session } = await createAgentSession({
  cwd: process.cwd(),
  agentDir: "/custom/agent",

  model,
  thinkingLevel: "off",
  authStorage,
  modelRegistry,

  tools: ["read", "bash", "status"],
  customTools: [statusTool],
  resourceLoader: loader,

  sessionManager: SessionManager.inMemory(),
  settingsManager,
});

session.subscribe((event) => {
  if (event.type === "message_update" && event.assistantMessageEvent.type === "text_delta") {
    process.stdout.write(event.assistantMessageEvent.delta);
  }
});

await session.prompt("Get status and list files.");
```

## 运行模式

该 SDK 导出了运行模式相关的工具函数,便于在 `createAgentSession()` 之上构建自定义界面:

### InteractiveMode

完整的 TUI 交互模式,包含编辑器、聊天历史以及所有内置命令:

```typescript
import {
  type CreateAgentSessionRuntimeFactory,
  createAgentSessionFromServices,
  createAgentSessionRuntime,
  createAgentSessionServices,
  getAgentDir,
  InteractiveMode,
  SessionManager,
} from "@earendil-works/pi-coding-agent";

const createRuntime: CreateAgentSessionRuntimeFactory = async ({ cwd, sessionManager, sessionStartEvent }) => {
  const services = await createAgentSessionServices({ cwd });
  return {
    ...(await createAgentSessionFromServices({ services, sessionManager, sessionStartEvent })),
    services,
    diagnostics: services.diagnostics,
  };
};
const runtime = await createAgentSessionRuntime(createRuntime, {
  cwd: process.cwd(),
  agentDir: getAgentDir(),
  sessionManager: SessionManager.create(process.cwd()),
});

const mode = new InteractiveMode(runtime, {
  migratedProviders: [],
  modelFallbackMessage: undefined,
  initialMessage: "Hello",
  initialImages: [],
  initialMessages: [],
});

await mode.run();
```

### runPrintMode

单次执行模式:发送提示词、输出结果、退出:

```typescript
import {
  type CreateAgentSessionRuntimeFactory,
  createAgentSessionFromServices,
  createAgentSessionRuntime,
  createAgentSessionServices,
  getAgentDir,
  runPrintMode,
  SessionManager,
} from "@earendil-works/pi-coding-agent";

const createRuntime: CreateAgentSessionRuntimeFactory = async ({ cwd, sessionManager, sessionStartEvent }) => {
  const services = await createAgentSessionServices({ cwd });
  return {
    ...(await createAgentSessionFromServices({ services, sessionManager, sessionStartEvent })),
    services,
    diagnostics: services.diagnostics,
  };
};
const runtime = await createAgentSessionRuntime(createRuntime, {
  cwd: process.cwd(),
  agentDir: getAgentDir(),
  sessionManager: SessionManager.create(process.cwd()),
});

await runPrintMode(runtime, {
  mode: "text",
  initialMessage: "Hello",
  initialImages: [],
  messages: ["Follow up"],
});
```

### runRpcMode

用于子进程集成的 JSON-RPC 模式:

```typescript
import {
  type CreateAgentSessionRuntimeFactory,
  createAgentSessionFromServices,
  createAgentSessionRuntime,
  createAgentSessionServices,
  getAgentDir,
  runRpcMode,
  SessionManager,
} from "@earendil-works/pi-coding-agent";

const createRuntime: CreateAgentSessionRuntimeFactory = async ({ cwd, sessionManager, sessionStartEvent }) => {
  const services = await createAgentSessionServices({ cwd });
  return {
    ...(await createAgentSessionFromServices({ services, sessionManager, sessionStartEvent })),
    services,
    diagnostics: services.diagnostics,
  };
};
const runtime = await createAgentSessionRuntime(createRuntime, {
  cwd: process.cwd(),
  agentDir: getAgentDir(),
  sessionManager: SessionManager.create(process.cwd()),
});

await runRpcMode(runtime);
```

JSON 协议详情请参见 [RPC 文档](rpc.md)。

## RPC 模式替代方案

若不想通过 SDK 构建,而是基于子进程进行集成,可以直接使用 CLI:

```bash
pi --mode rpc --no-session
```

JSON 协议详情请参见 [RPC 文档](rpc.md)。

以下情况适合优先选用 SDK:
- 你需要类型安全
- 你处于同一个 Node.js 进程中
- 你需要直接访问代理状态
- 你希望以编程方式自定义工具/扩展

以下情况适合优先选用 RPC 模式:
- 你需要从其他语言进行集成
- 你需要进程隔离
- 你正在构建一个与语言无关的客户端

## 导出内容

主入口导出内容如下:

```typescript
// 工厂函数
createAgentSession
createAgentSessionRuntime
AgentSessionRuntime

// 认证与模型
AuthStorage
ModelRegistry

// 资源加载
DefaultResourceLoader
type ResourceLoader
createEventBus

// 辅助函数
defineTool

// 会话管理
SessionManager
SettingsManager

// 工具工厂函数
createCodingTools
createReadOnlyTools
createReadTool, createBashTool, createEditTool, createWriteTool
createGrepTool, createFindTool, createLsTool

// 类型
type CreateAgentSessionOptions
type CreateAgentSessionResult
type ExtensionFactory
type ExtensionAPI
type ToolDefinition
type Skill
type PromptTemplate
type Tool
```

关于扩展相关类型,完整 API 请参见 [extensions.md](extensions.md)。
