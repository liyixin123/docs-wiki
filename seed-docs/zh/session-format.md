# 会话文件格式

会话以 JSONL（JSON Lines）文件的形式存储。每一行都是一个带有 `type` 字段的 JSON 对象。会话条目通过 `id`/`parentId` 字段构成一个树形结构，从而支持原地分支而无需创建新文件。

## 文件位置

```
~/.pi/agent/sessions/--<path>--/<timestamp>_<uuid>.jsonl
```

其中 `<path>` 是将工作目录中的 `/` 替换为 `-` 之后得到的字符串。

## 删除会话

可以通过删除 `~/.pi/agent/sessions/` 目录下的 `.jsonl` 文件来移除会话。

Pi 也支持在 `/resume` 中交互式地删除会话（选中一个会话后按 `Ctrl+D`，然后确认）。在可用的情况下，pi 会使用 `trash` 命令行工具，以避免永久删除。

## 会话版本

会话的头部包含一个版本字段：

- **版本 1**:线性条目序列(遗留格式,加载时自动迁移)
- **版本 2**:通过 `id`/`parentId` 关联的树形结构
- **版本 3**:将 `hookMessage` 角色重命名为 `custom`(扩展统一化)

现有会话在加载时会自动迁移到当前版本(v3)。

## 源文件

GitHub 上的源码([pi-mono](https://github.com/earendil-works/pi-mono)):
- [`packages/coding-agent/src/core/session-manager.ts`](https://github.com/earendil-works/pi-mono/blob/main/packages/coding-agent/src/core/session-manager.ts) - 会话条目类型与 SessionManager
- [`packages/coding-agent/src/core/messages.ts`](https://github.com/earendil-works/pi-mono/blob/main/packages/coding-agent/src/core/messages.ts) - 扩展消息类型(BashExecutionMessage、CustomMessage 等)
- [`packages/ai/src/types.ts`](https://github.com/earendil-works/pi-mono/blob/main/packages/ai/src/types.ts) - 基础消息类型(UserMessage、AssistantMessage、ToolResultMessage)
- [`packages/agent/src/types.ts`](https://github.com/earendil-works/pi-mono/blob/main/packages/agent/src/types.ts) - AgentMessage 联合类型

若需要在你的项目中查看 TypeScript 类型定义,请查阅 `node_modules/@earendil-works/pi-coding-agent/dist/` 和 `node_modules/@earendil-works/pi-ai/dist/`。

## 消息类型

会话条目中包含 `AgentMessage` 对象。理解这些类型对于解析会话和编写扩展至关重要。

### 内容块

消息包含一组带类型的内容块数组:

```typescript
interface TextContent {
  type: "text";
  text: string;
}

interface ImageContent {
  type: "image";
  data: string;      // base64 encoded
  mimeType: string;  // e.g., "image/jpeg", "image/png"
}

interface ThinkingContent {
  type: "thinking";
  thinking: string;
}

interface ToolCall {
  type: "toolCall";
  id: string;
  name: string;
  arguments: Record<string, any>;
}
```

### 基础消息类型(来自 pi-ai)

```typescript
interface UserMessage {
  role: "user";
  content: string | (TextContent | ImageContent)[];
  timestamp: number;  // Unix ms
}

interface AssistantMessage {
  role: "assistant";
  content: (TextContent | ThinkingContent | ToolCall)[];
  api: string;
  provider: string;
  model: string;
  usage: Usage;
  stopReason: "stop" | "length" | "toolUse" | "error" | "aborted";
  errorMessage?: string;
  timestamp: number;
}

interface ToolResultMessage {
  role: "toolResult";
  toolCallId: string;
  toolName: string;
  content: (TextContent | ImageContent)[];
  details?: any;      // Tool-specific metadata
  isError: boolean;
  timestamp: number;
}

interface Usage {
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
  totalTokens: number;
  cost: {
    input: number;
    output: number;
    cacheRead: number;
    cacheWrite: number;
    total: number;
  };
}
```

### 扩展消息类型(来自 pi-coding-agent)

```typescript
interface BashExecutionMessage {
  role: "bashExecution";
  command: string;
  output: string;
  exitCode: number | undefined;
  cancelled: boolean;
  truncated: boolean;
  fullOutputPath?: string;
  excludeFromContext?: boolean;  // true for !! prefix commands
  timestamp: number;
}

interface CustomMessage {
  role: "custom";
  customType: string;            // Extension identifier
  content: string | (TextContent | ImageContent)[];
  display: boolean;              // Show in TUI
  details?: any;                 // Extension-specific metadata
  timestamp: number;
}

interface BranchSummaryMessage {
  role: "branchSummary";
  summary: string;
  fromId: string;                // Entry we branched from
  timestamp: number;
}

interface CompactionSummaryMessage {
  role: "compactionSummary";
  summary: string;
  tokensBefore: number;
  timestamp: number;
}
```

### AgentMessage 联合类型

```typescript
type AgentMessage =
  | UserMessage
  | AssistantMessage
  | ToolResultMessage
  | BashExecutionMessage
  | CustomMessage
  | BranchSummaryMessage
  | CompactionSummaryMessage;
```

## 条目基类

除 `SessionHeader` 外,所有条目都继承自 `SessionEntryBase`:

```typescript
interface SessionEntryBase {
  type: string;
  id: string;           // 8-char hex ID
  parentId: string | null;  // Parent entry ID (null for first entry)
  timestamp: string;    // ISO timestamp
}
```

## 条目类型

### SessionHeader

文件的第一行。仅包含元数据,不属于树结构的一部分(没有 `id`/`parentId`)。

```json
{"type":"session","version":3,"id":"uuid","timestamp":"2024-12-03T14:00:00.000Z","cwd":"/path/to/project"}
```

对于带有父会话的会话(通过 `/fork`、`/clone` 或 `newSession({ parentSession })` 创建):

```json
{"type":"session","version":3,"id":"uuid","timestamp":"2024-12-03T14:00:00.000Z","cwd":"/path/to/project","parentSession":"/path/to/original/session.jsonl"}
```

### SessionMessageEntry

对话中的一条消息。`message` 字段包含一个 `AgentMessage`。

```json
{"type":"message","id":"a1b2c3d4","parentId":"prev1234","timestamp":"2024-12-03T14:00:01.000Z","message":{"role":"user","content":"Hello"}}
{"type":"message","id":"b2c3d4e5","parentId":"a1b2c3d4","timestamp":"2024-12-03T14:00:02.000Z","message":{"role":"assistant","content":[{"type":"text","text":"Hi!"}],"provider":"anthropic","model":"claude-sonnet-4-5","usage":{...},"stopReason":"stop"}}
{"type":"message","id":"c3d4e5f6","parentId":"b2c3d4e5","timestamp":"2024-12-03T14:00:03.000Z","message":{"role":"toolResult","toolCallId":"call_123","toolName":"bash","content":[{"type":"text","text":"output"}],"isError":false}}
```

### ModelChangeEntry

当用户在会话中途切换模型时触发。

```json
{"type":"model_change","id":"d4e5f6g7","parentId":"c3d4e5f6","timestamp":"2024-12-03T14:05:00.000Z","provider":"openai","modelId":"gpt-4o"}
```

### ThinkingLevelChangeEntry

当用户更改思考/推理级别时触发。

```json
{"type":"thinking_level_change","id":"e5f6g7h8","parentId":"d4e5f6g7","timestamp":"2024-12-03T14:06:00.000Z","thinkingLevel":"high"}
```

### CompactionEntry

在上下文被压缩时创建。存储对之前消息的摘要。

```json
{"type":"compaction","id":"f6g7h8i9","parentId":"e5f6g7h8","timestamp":"2024-12-03T14:10:00.000Z","summary":"User discussed X, Y, Z...","firstKeptEntryId":"c3d4e5f6","tokensBefore":50000}
```

可选字段:
- `details`:实现相关的数据(例如默认情况下为 `{ readFiles: string[], modifiedFiles: string[] }`,或扩展自定义的数据)
- `fromHook`:如果由扩展生成则为 `true`,如果由 pi 生成则为 `false`/`undefined`(遗留字段名)

### BranchSummaryEntry

通过 `/tree` 切换分支时创建,包含由 LLM 生成的、从公共祖先到该分支末端的摘要。用于捕获被放弃路径的上下文。

```json
{"type":"branch_summary","id":"g7h8i9j0","parentId":"a1b2c3d4","timestamp":"2024-12-03T14:15:00.000Z","fromId":"f6g7h8i9","summary":"Branch explored approach A..."}
```

可选字段:
- `details`:文件跟踪数据(`{ readFiles: string[], modifiedFiles: string[] }`),默认情况下如此,或扩展自定义的数据
- `fromHook`:如果由扩展生成则为 `true`,如果由 pi 生成则为 `false`/`undefined`(遗留字段名)

### CustomEntry

用于扩展状态持久化。**不会**参与 LLM 上下文。

```json
{"type":"custom","id":"h8i9j0k1","parentId":"g7h8i9j0","timestamp":"2024-12-03T14:20:00.000Z","customType":"my-extension","data":{"count":42}}
```

在重新加载时,使用 `customType` 来识别属于你扩展的条目。

### CustomMessageEntry

由扩展注入的消息,**会**参与 LLM 上下文。

```json
{"type":"custom_message","id":"i9j0k1l2","parentId":"h8i9j0k1","timestamp":"2024-12-03T14:25:00.000Z","customType":"my-extension","content":"Injected context...","display":true}
```

字段说明:
- `content`:字符串或 `(TextContent | ImageContent)[]`(与 UserMessage 相同)
- `display`:`true` 表示以独特样式在 TUI 中展示,`false` 表示隐藏
- `details`:可选的扩展专属元数据(不会发送给 LLM)

### LabelEntry

用户在某个条目上定义的书签/标记。

```json
{"type":"label","id":"j0k1l2m3","parentId":"i9j0k1l2","timestamp":"2024-12-03T14:30:00.000Z","targetId":"a1b2c3d4","label":"checkpoint-1"}
```

将 `label` 设为 `undefined` 即可清除标签。

### SessionInfoEntry

会话元数据(例如用户自定义的显示名称)。可通过 `/name` 命令或扩展中的 `pi.setSessionName()` 设置。

```json
{"type":"session_info","id":"k1l2m3n4","parentId":"j0k1l2m3","timestamp":"2024-12-03T14:35:00.000Z","name":"Refactor auth module"}
```

设置了会话名称后,会话选择器(`/resume`)中会显示该名称,而不是显示第一条消息。

## 树形结构

条目构成一棵树:
- 第一个条目的 `parentId` 为 `null`
- 每个后续条目通过 `parentId` 指向其父条目
- 分支操作会从较早的某个条目创建新的子节点
- "叶子(leaf)"表示树中当前所处的位置

```
[user msg] ─── [assistant] ─── [user msg] ─── [assistant] ─┬─ [user msg] ← current leaf
                                                            │
                                                            └─ [branch_summary] ─── [user msg] ← alternate branch
```

## 构建上下文

`buildSessionContext()` 会从当前叶子节点向根节点遍历,生成供 LLM 使用的消息列表:

1. 收集路径上的所有条目
2. 提取当前的模型设置和思考级别设置
3. 如果路径上存在 `CompactionEntry`:
   - 首先输出摘要
   - 然后输出从 `firstKeptEntryId` 到压缩点之间的消息
   - 最后输出压缩之后的消息
4. 将 `BranchSummaryEntry` 和 `CustomMessageEntry` 转换为相应的消息格式

## 解析示例

```typescript
import { readFileSync } from "fs";

const lines = readFileSync("session.jsonl", "utf8").trim().split("\n");

for (const line of lines) {
  const entry = JSON.parse(line);

  switch (entry.type) {
    case "session":
      console.log(`Session v${entry.version ?? 1}: ${entry.id}`);
      break;
    case "message":
      console.log(`[${entry.id}] ${entry.message.role}: ${JSON.stringify(entry.message.content)}`);
      break;
    case "compaction":
      console.log(`[${entry.id}] Compaction: ${entry.tokensBefore} tokens summarized`);
      break;
    case "branch_summary":
      console.log(`[${entry.id}] Branch from ${entry.fromId}`);
      break;
    case "custom":
      console.log(`[${entry.id}] Custom (${entry.customType}): ${JSON.stringify(entry.data)}`);
      break;
    case "custom_message":
      console.log(`[${entry.id}] Extension message (${entry.customType}): ${entry.content}`);
      break;
    case "label":
      console.log(`[${entry.id}] Label "${entry.label}" on ${entry.targetId}`);
      break;
    case "model_change":
      console.log(`[${entry.id}] Model: ${entry.provider}/${entry.modelId}`);
      break;
    case "thinking_level_change":
      console.log(`[${entry.id}] Thinking: ${entry.thinkingLevel}`);
      break;
  }
}
```

## SessionManager API

以编程方式操作会话时常用的方法。

### 静态创建方法
- `SessionManager.create(cwd, sessionDir?)` - 创建新会话
- `SessionManager.open(path, sessionDir?)` - 打开已有的会话文件
- `SessionManager.continueRecent(cwd, sessionDir?)` - 继续最近的会话,若不存在则创建新会话
- `SessionManager.inMemory(cwd?)` - 不进行文件持久化
- `SessionManager.forkFrom(sourcePath, targetCwd, sessionDir?)` - 从另一个项目的会话进行 fork

### 静态列出方法
- `SessionManager.list(cwd, sessionDir?, onProgress?)` - 列出某个目录下的会话
- `SessionManager.listAll(onProgress?)` - 列出所有项目下的全部会话

### 实例方法 - 会话管理
- `newSession(options?)` - 开始一个新会话(选项:`{ parentSession?: string }`)
- `setSessionFile(path)` - 切换到另一个会话文件
- `createBranchedSession(leafId)` - 将某个分支提取为新的会话文件

### 实例方法 - 追加条目(均返回条目 ID)
- `appendMessage(message)` - 添加消息
- `appendThinkingLevelChange(level)` - 记录思考级别的变更
- `appendModelChange(provider, modelId)` - 记录模型的变更
- `appendCompaction(summary, firstKeptEntryId, tokensBefore, details?, fromHook?)` - 添加压缩条目
- `appendCustomEntry(customType, data?)` - 扩展状态(不参与上下文)
- `appendSessionInfo(name)` - 设置会话显示名称
- `appendCustomMessageEntry(customType, content, display, details?)` - 扩展消息(参与上下文)
- `appendLabelChange(targetId, label)` - 设置/清除标签

### 实例方法 - 树导航
- `getLeafId()` - 获取当前所在位置
- `getLeafEntry()` - 获取当前叶子条目
- `getEntry(id)` - 按 ID 获取条目
- `getBranch(fromId?)` - 从某个条目走向根节点
- `getTree()` - 获取完整的树结构
- `getChildren(parentId)` - 获取直接子条目
- `getLabel(id)` - 获取某个条目的标签
- `branch(entryId)` - 将叶子移动到更早的条目
- `resetLeaf()` - 将叶子重置为 null(即任何条目之前)
- `branchWithSummary(entryId, summary, details?, fromHook?)` - 带上下文摘要进行分支

### 实例方法 - 上下文与信息
- `buildSessionContext()` - 为 LLM 获取消息、思考级别和模型
- `getEntries()` - 所有条目(不含头部)
- `getHeader()` - 会话头部元数据
- `getSessionName()` - 从最新的 session_info 条目获取显示名称
- `getCwd()` - 工作目录
- `getSessionDir()` - 会话存储目录
- `getSessionId()` - 会话 UUID
- `getSessionFile()` - 会话文件路径(内存会话为 undefined)
- `isPersisted()` - 该会话是否已保存到磁盘
