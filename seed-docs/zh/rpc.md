# RPC 模式

RPC 模式通过 stdin/stdout 上的 JSON 协议,使 coding agent 能够以无头(headless)方式运行。这对于将该代理嵌入其他应用程序、IDE 或自定义 UI 中非常有用。

**Node.js/TypeScript 用户须知**:如果你正在构建一个 Node.js 应用程序,建议直接使用 `@earendil-works/pi-coding-agent` 中的 `AgentSession`,而不是派生一个子进程。API 详情请参见 [`src/core/agent-session.ts`](../src/core/agent-session.ts)。若需要基于子进程的 TypeScript 客户端,请参见 [`src/modes/rpc/rpc-client.ts`](../src/modes/rpc/rpc-client.ts)。

## 启动 RPC 模式

```bash
pi --mode rpc [options]
```

常用选项:
- `--provider <name>`:设置 LLM 提供方(anthropic、openai、google 等)
- `--model <pattern>`:模型匹配模式或 ID(支持 `provider/id` 格式以及可选的 `:<thinking>` 后缀)
- `--no-session`:禁用会话持久化
- `--session-dir <path>`:自定义会话存储目录

## 协议概览

- **命令(Commands)**:发送到 stdin 的 JSON 对象,每行一个
- **响应(Responses)**:带有 `type: "response"` 字段的 JSON 对象,表示命令执行的成功或失败
- **事件(Events)**:以 JSON 行的形式流式输出到 stdout 的代理事件

所有命令都支持可选的 `id` 字段,用于请求/响应关联。如果提供了该字段,对应的响应也会包含相同的 `id`。

### 分帧(Framing)

RPC 模式采用严格的 JSONL 语义,仅以 LF(`\n`)作为记录分隔符。

这一点对客户端实现十分重要:
- 仅按 `\n` 拆分记录
- 可以接受可选的 `\r\n` 输入,方式是去掉末尾的 `\r`
- 不要使用会将 Unicode 分隔符也当作换行符处理的通用行读取器

需要特别说明的是,Node 的 `readline` 并不符合 RPC 模式的协议要求,因为它还会按 `U+2028` 和 `U+2029` 进行拆分,而这两个字符在 JSON 字符串内部是合法的。

## 命令

### 提示词发送

#### prompt

向代理发送一条用户提示词。命令的响应会在提示词被接受、排队或处理之后发出。事件会在被接受之后异步持续流式输出。

```json
{"id": "req-1", "type": "prompt", "message": "Hello, world!"}
```

携带图片:
```json
{"type": "prompt", "message": "What's in this image?", "images": [{"type": "image", "data": "base64-encoded-data", "mimeType": "image/png"}]}
```

**流式输出期间**:如果代理已经处于流式输出状态,你必须指定 `streamingBehavior` 才能将消息排队:

```json
{"type": "prompt", "message": "New instruction", "streamingBehavior": "steer"}
```

- `"steer"`:在代理运行期间将消息排队。它会在当前助手轮次完成其工具调用之后、下一次 LLM 调用之前被投递。
- `"followUp"`:等待直到代理完成。消息仅在代理停止时才会被投递。

如果代理正在流式输出且未指定 `streamingBehavior`,该命令会返回错误。

**扩展命令**:如果消息是一个扩展命令(例如 `/mycommand`),即使在流式输出期间也会立即执行。扩展命令通过 `pi.sendMessage()` 自行管理与 LLM 的交互。

**输入展开**:技能命令(`/skill:name`)和提示词模板(`/template`)会在发送/排队之前被展开。

响应:
```json
{"id": "req-1", "type": "response", "command": "prompt", "success": true}
```

`success: true` 表示提示词已被接受、排队或立即处理。`success: false` 表示提示词在被接受之前就被拒绝。被接受之后发生的失败会通过常规的事件和消息流上报,而不会作为针对同一请求 id 的第二个 `response` 上报。

`images` 字段是可选的。每张图片使用 `ImageContent` 格式:`{"type": "image", "data": "base64-encoded-data", "mimeType": "image/png"}`。

#### steer

在代理运行期间排队一条引导(steering)消息。它会在当前助手轮次完成其工具调用之后、下一次 LLM 调用之前被投递。技能命令和提示词模板会被展开。不允许使用扩展命令(请改用 `prompt`)。

```json
{"type": "steer", "message": "Stop and do this instead"}
```

携带图片:
```json
{"type": "steer", "message": "Look at this instead", "images": [{"type": "image", "data": "base64-encoded-data", "mimeType": "image/png"}]}
```

`images` 字段是可选的。每张图片使用与 `prompt` 相同的 `ImageContent` 格式。

响应:
```json
{"type": "response", "command": "steer", "success": true}
```

关于如何控制引导消息的处理方式,请参见 [set_steering_mode](#set_steering_mode)。

#### follow_up

排队一条后续(follow-up)消息,待代理完成后处理。仅在代理没有更多工具调用或引导消息时才会被投递。技能命令和提示词模板会被展开。不允许使用扩展命令(请改用 `prompt`)。

```json
{"type": "follow_up", "message": "After you're done, also do this"}
```

携带图片:
```json
{"type": "follow_up", "message": "Also check this image", "images": [{"type": "image", "data": "base64-encoded-data", "mimeType": "image/png"}]}
```

`images` 字段是可选的。每张图片使用与 `prompt` 相同的 `ImageContent` 格式。

响应:
```json
{"type": "response", "command": "follow_up", "success": true}
```

关于如何控制后续消息的处理方式,请参见 [set_follow_up_mode](#set_follow_up_mode)。

#### abort

中止当前的代理操作。

```json
{"type": "abort"}
```

响应:
```json
{"type": "response", "command": "abort", "success": true}
```

#### new_session

开始一个全新的会话。该操作可被 `session_before_switch` 扩展事件处理器取消。

```json
{"type": "new_session"}
```

携带可选的父会话跟踪信息:
```json
{"type": "new_session", "parentSession": "/path/to/parent-session.jsonl"}
```

响应:
```json
{"type": "response", "command": "new_session", "success": true, "data": {"cancelled": false}}
```

若被扩展取消:
```json
{"type": "response", "command": "new_session", "success": true, "data": {"cancelled": true}}
```

### 状态

#### get_state

获取当前会话状态。

```json
{"type": "get_state"}
```

响应:
```json
{
  "type": "response",
  "command": "get_state",
  "success": true,
  "data": {
    "model": {...},
    "thinkingLevel": "medium",
    "isStreaming": false,
    "isCompacting": false,
    "steeringMode": "all",
    "followUpMode": "one-at-a-time",
    "sessionFile": "/path/to/session.jsonl",
    "sessionId": "abc123",
    "sessionName": "my-feature-work",
    "autoCompactionEnabled": true,
    "messageCount": 5,
    "pendingMessageCount": 0
  }
}
```

`model` 字段是一个完整的 [Model](#model) 对象,或为 `null`。`sessionName` 字段是通过 `set_session_name` 设置的展示名称,若未设置则省略该字段。

#### get_messages

获取对话中的所有消息。

```json
{"type": "get_messages"}
```

响应:
```json
{
  "type": "response",
  "command": "get_messages",
  "success": true,
  "data": {"messages": [...]}
}
```

消息为 `AgentMessage` 对象(参见[消息类型](#message-types))。

### 模型

#### set_model

切换到指定模型。

```json
{"type": "set_model", "provider": "anthropic", "modelId": "claude-sonnet-4-20250514"}
```

响应包含完整的 [Model](#model) 对象:
```json
{
  "type": "response",
  "command": "set_model",
  "success": true,
  "data": {...}
}
```

#### cycle_model

循环切换到下一个可用模型。如果只有一个可用模型,则返回 `null` 数据。

```json
{"type": "cycle_model"}
```

响应:
```json
{
  "type": "response",
  "command": "cycle_model",
  "success": true,
  "data": {
    "model": {...},
    "thinkingLevel": "medium",
    "isScoped": false
  }
}
```

`model` 字段是一个完整的 [Model](#model) 对象。

#### get_available_models

列出所有已配置的模型。

```json
{"type": "get_available_models"}
```

响应包含一个由完整 [Model](#model) 对象组成的数组:
```json
{
  "type": "response",
  "command": "get_available_models",
  "success": true,
  "data": {
    "models": [...]
  }
}
```

### 思考(Thinking)

#### set_thinking_level

为支持该功能的模型设置推理/思考级别。

```json
{"type": "set_thinking_level", "level": "high"}
```

级别取值:`"off"`、`"minimal"`、`"low"`、`"medium"`、`"high"`、`"xhigh"`

注意:`"xhigh"` 仅被 OpenAI 的 codex-max 系列模型支持。

响应:
```json
{"type": "response", "command": "set_thinking_level", "success": true}
```

#### cycle_thinking_level

循环切换可用的思考级别。如果模型不支持思考功能,则返回 `null` 数据。

```json
{"type": "cycle_thinking_level"}
```

响应:
```json
{
  "type": "response",
  "command": "cycle_thinking_level",
  "success": true,
  "data": {"level": "high"}
}
```

### 队列模式

#### set_steering_mode

控制引导消息(来自 `steer`)的投递方式。

```json
{"type": "set_steering_mode", "mode": "one-at-a-time"}
```

模式:
- `"all"`:在当前助手轮次完成其工具调用之后,投递所有引导消息
- `"one-at-a-time"`:每完成一个助手轮次,投递一条引导消息(默认值)

响应:
```json
{"type": "response", "command": "set_steering_mode", "success": true}
```

#### set_follow_up_mode

控制后续消息(来自 `follow_up`)的投递方式。

```json
{"type": "set_follow_up_mode", "mode": "one-at-a-time"}
```

模式:
- `"all"`:代理完成时投递所有后续消息
- `"one-at-a-time"`:代理每完成一次,投递一条后续消息(默认值)

响应:
```json
{"type": "response", "command": "set_follow_up_mode", "success": true}
```

### 上下文压缩(Compaction)

#### compact

手动压缩对话上下文以减少 token 用量。

```json
{"type": "compact"}
```

携带自定义指令:
```json
{"type": "compact", "customInstructions": "Focus on code changes"}
```

响应:
```json
{
  "type": "response",
  "command": "compact",
  "success": true,
  "data": {
    "summary": "Summary of conversation...",
    "firstKeptEntryId": "abc123",
    "tokensBefore": 150000,
    "details": {}
  }
}
```

#### set_auto_compaction

在上下文接近占满时,启用或禁用自动压缩。

```json
{"type": "set_auto_compaction", "enabled": true}
```

响应:
```json
{"type": "response", "command": "set_auto_compaction", "success": true}
```

### 重试(Retry)

#### set_auto_retry

在出现瞬时性错误(过载、速率限制、5xx)时,启用或禁用自动重试。

```json
{"type": "set_auto_retry", "enabled": true}
```

响应:
```json
{"type": "response", "command": "set_auto_retry", "success": true}
```

#### abort_retry

中止正在进行的重试(取消延迟并停止重试)。

```json
{"type": "abort_retry"}
```

响应:
```json
{"type": "response", "command": "abort_retry", "success": true}
```

### Bash

#### bash

执行一条 shell 命令,并将输出添加到对话上下文中。

```json
{"type": "bash", "command": "ls -la"}
```

响应:
```json
{
  "type": "response",
  "command": "bash",
  "success": true,
  "data": {
    "output": "total 48\ndrwxr-xr-x ...",
    "exitCode": 0,
    "cancelled": false,
    "truncated": false
  }
}
```

如果输出被截断,响应会包含 `fullOutputPath`:
```json
{
  "type": "response",
  "command": "bash",
  "success": true,
  "data": {
    "output": "truncated output...",
    "exitCode": 0,
    "cancelled": false,
    "truncated": true,
    "fullOutputPath": "/tmp/pi-bash-abc123.log"
  }
}
```

**bash 执行结果如何传递给 LLM:**

`bash` 命令会立即执行并返回一个 `BashResult`。在内部,系统会创建一个 `BashExecutionMessage` 并存储到代理的消息状态中。该消息不会触发任何事件。

当下一条 `prompt` 命令发出时,所有消息(包括 `BashExecutionMessage`)会在发送给 LLM 之前被转换。`BashExecutionMessage` 会被转换为如下格式的 `UserMessage`:

````
Ran `ls -la`
```
total 48
drwxr-xr-x ...
```
````

这意味着:
1. bash 输出会在**下一次** prompt 时才被纳入 LLM 上下文,而非立即纳入
2. 可以在一次 prompt 之前执行多条 bash 命令;所有输出都会被一并纳入
3. `BashExecutionMessage` 本身不会触发任何事件

#### abort_bash

中止一个正在运行的 bash 命令。

```json
{"type": "abort_bash"}
```

响应:
```json
{"type": "response", "command": "abort_bash", "success": true}
```

### 会话

#### get_session_stats

获取 token 用量、成本统计以及当前上下文窗口的使用情况。

```json
{"type": "get_session_stats"}
```

响应:
```json
{
  "type": "response",
  "command": "get_session_stats",
  "success": true,
  "data": {
    "sessionFile": "/path/to/session.jsonl",
    "sessionId": "abc123",
    "userMessages": 5,
    "assistantMessages": 5,
    "toolCalls": 12,
    "toolResults": 12,
    "totalMessages": 22,
    "tokens": {
      "input": 50000,
      "output": 10000,
      "cacheRead": 40000,
      "cacheWrite": 5000,
      "total": 105000
    },
    "cost": 0.45,
    "contextUsage": {
      "tokens": 60000,
      "contextWindow": 200000,
      "percent": 30
    }
  }
}
```

`tokens` 包含当前会话状态下助手的用量总计。`contextUsage` 包含用于压缩判断和页脚显示的、真实的当前上下文窗口估算值。

当没有可用的模型或上下文窗口信息时,`contextUsage` 会被省略。压缩刚完成后,`contextUsage.tokens` 和 `contextUsage.percent` 会是 `null`,直到出现一条压缩后的全新助手响应提供有效的用量数据为止。

#### export_html

将会话导出为 HTML 文件。

```json
{"type": "export_html"}
```

携带自定义路径:
```json
{"type": "export_html", "outputPath": "/tmp/session.html"}
```

响应:
```json
{
  "type": "response",
  "command": "export_html",
  "success": true,
  "data": {"path": "/tmp/session.html"}
}
```

#### switch_session

加载另一个会话文件。该操作可被 `session_before_switch` 扩展事件处理器取消。

```json
{"type": "switch_session", "sessionPath": "/path/to/session.jsonl"}
```

响应:
```json
{"type": "response", "command": "switch_session", "success": true, "data": {"cancelled": false}}
```

若被扩展取消该切换操作:
```json
{"type": "response", "command": "switch_session", "success": true, "data": {"cancelled": true}}
```

#### fork

从当前活动分支上的某条历史用户消息创建一个新的分叉(fork)。该操作可被 `session_before_fork` 扩展事件处理器取消。返回被分叉的消息文本。

```json
{"type": "fork", "entryId": "abc123"}
```

响应:
```json
{
  "type": "response",
  "command": "fork",
  "success": true,
  "data": {"text": "The original prompt text...", "cancelled": false}
}
```

若被扩展取消该分叉操作:
```json
{
  "type": "response",
  "command": "fork",
  "success": true,
  "data": {"text": "The original prompt text...", "cancelled": true}
}
```

#### clone

在当前所处位置,将当前活动分支复制到一个新会话中。该操作可被 `session_before_fork` 扩展事件处理器取消。

```json
{"type": "clone"}
```

响应:
```json
{
  "type": "response",
  "command": "clone",
  "success": true,
  "data": {"cancelled": false}
}
```

若被扩展取消该克隆操作:
```json
{
  "type": "response",
  "command": "clone",
  "success": true,
  "data": {"cancelled": true}
}
```

#### get_fork_messages

获取可用于分叉的用户消息列表。

```json
{"type": "get_fork_messages"}
```

响应:
```json
{
  "type": "response",
  "command": "get_fork_messages",
  "success": true,
  "data": {
    "messages": [
      {"entryId": "abc123", "text": "First prompt..."},
      {"entryId": "def456", "text": "Second prompt..."}
    ]
  }
}
```

#### get_last_assistant_text

获取最后一条助手消息的文本内容。

```json
{"type": "get_last_assistant_text"}
```

响应:
```json
{
  "type": "response",
  "command": "get_last_assistant_text",
  "success": true,
  "data": {"text": "The assistant's response..."}
}
```

若不存在任何助手消息,则返回 `{"text": null}`。

#### set_session_name

为当前会话设置一个展示名称。该名称会出现在会话列表中,便于识别会话。

```json
{"type": "set_session_name", "name": "my-feature-work"}
```

响应:
```json
{
  "type": "response",
  "command": "set_session_name",
  "success": true
}
```

当前会话名称可以通过 `get_state` 的 `sessionName` 字段获取。

### 命令(Commands)

#### get_commands

获取可用命令(扩展命令、提示词模板以及技能)。这些命令都可以通过 `prompt` 命令、以 `/` 为前缀来调用。

```json
{"type": "get_commands"}
```

响应:
```json
{
  "type": "response",
  "command": "get_commands",
  "success": true,
  "data": {
    "commands": [
      {"name": "session-name", "description": "Set or clear session name", "source": "extension", "path": "/home/user/.pi/agent/extensions/session.ts"},
      {"name": "fix-tests", "description": "Fix failing tests", "source": "prompt", "location": "project", "path": "/home/user/myproject/.pi/agent/prompts/fix-tests.md"},
      {"name": "skill:brave-search", "description": "Web search via Brave API", "source": "skill", "location": "user", "path": "/home/user/.pi/agent/skills/brave-search/SKILL.md"}
    ]
  }
}
```

每个命令包含以下字段:
- `name`:命令名称(通过 `/name` 调用)
- `description`:人类可读的描述(对于扩展命令是可选的)
- `source`:命令的种类:
  - `"extension"`:在扩展中通过 `pi.registerCommand()` 注册
  - `"prompt"`:从提示词模板 `.md` 文件加载
  - `"skill"`:从技能目录加载(名称会带有 `skill:` 前缀)
- `location`:加载来源(可选字段,扩展命令不包含此字段):
  - `"user"`:用户级(`~/.pi/agent/`)
  - `"project"`:项目级(`./.pi/agent/`)
  - `"path"`:通过 CLI 或配置指定的显式路径
- `path`:命令来源文件的绝对路径(可选)

**注意**:内置的 TUI 命令(`/settings`、`/hotkeys` 等)不包含在内。它们仅在交互模式下处理,如果通过 `prompt` 发送则不会执行。

## 事件

事件会在代理运行期间以 JSON 行的形式流式输出到 stdout。事件不包含 `id` 字段(只有响应才包含)。

### 事件类型

| 事件 | 说明 |
|-------|-------------|
| `agent_start` | 代理开始处理 |
| `agent_end` | 代理完成(包含所有生成的消息) |
| `turn_start` | 新一轮开始 |
| `turn_end` | 本轮完成(包含助手消息和工具调用结果) |
| `message_start` | 消息开始 |
| `message_update` | 流式更新(文本/思考/工具调用增量) |
| `message_end` | 消息完成 |
| `tool_execution_start` | 工具开始执行 |
| `tool_execution_update` | 工具执行进度(流式输出) |
| `tool_execution_end` | 工具执行完成 |
| `queue_update` | 待处理的引导/后续消息队列发生变化 |
| `compaction_start` | 压缩开始 |
| `compaction_end` | 压缩完成 |
| `auto_retry_start` | 自动重试开始(发生瞬时性错误之后) |
| `auto_retry_end` | 自动重试结束(成功或最终失败) |
| `extension_error` | 扩展抛出了错误 |

### agent_start

当代理开始处理某条提示词时触发。

```json
{"type": "agent_start"}
```

### agent_end

当代理完成时触发。包含本次运行过程中生成的全部消息。

```json
{
  "type": "agent_end",
  "messages": [...]
}
```

### turn_start / turn_end

一轮(turn)由一次助手响应及其引发的工具调用和结果组成。

```json
{"type": "turn_start"}
```

```json
{
  "type": "turn_end",
  "message": {...},
  "toolResults": [...]
}
```

### message_start / message_end

当一条消息开始和完成时触发。`message` 字段包含一个 `AgentMessage`。

```json
{"type": "message_start", "message": {...}}
{"type": "message_end", "message": {...}}
```

### message_update(流式)

在助手消息流式生成期间触发。同时包含部分消息内容和一个流式增量事件。

```json
{
  "type": "message_update",
  "message": {...},
  "assistantMessageEvent": {
    "type": "text_delta",
    "contentIndex": 0,
    "delta": "Hello ",
    "partial": {...}
  }
}
```

`assistantMessageEvent` 字段包含以下增量类型之一:

| 类型 | 说明 |
|------|-------------|
| `start` | 消息生成开始 |
| `text_start` | 文本内容块开始 |
| `text_delta` | 文本内容片段 |
| `text_end` | 文本内容块结束 |
| `thinking_start` | 思考块开始 |
| `thinking_delta` | 思考内容片段 |
| `thinking_end` | 思考块结束 |
| `toolcall_start` | 工具调用开始 |
| `toolcall_delta` | 工具调用参数片段 |
| `toolcall_end` | 工具调用结束(包含完整的 `toolCall` 对象) |
| `done` | 消息完成(reason 取值:`"stop"`、`"length"`、`"toolUse"`) |
| `error` | 发生错误(reason 取值:`"aborted"`、`"error"`) |

流式返回文本响应的示例:
```json
{"type":"message_update","message":{...},"assistantMessageEvent":{"type":"text_start","contentIndex":0,"partial":{...}}}
{"type":"message_update","message":{...},"assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"Hello","partial":{...}}}
{"type":"message_update","message":{...},"assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":" world","partial":{...}}}
{"type":"message_update","message":{...},"assistantMessageEvent":{"type":"text_end","contentIndex":0,"content":"Hello world","partial":{...}}}
```

### tool_execution_start / tool_execution_update / tool_execution_end

当工具开始执行、流式输出进度以及执行完成时触发。

```json
{
  "type": "tool_execution_start",
  "toolCallId": "call_abc123",
  "toolName": "bash",
  "args": {"command": "ls -la"}
}
```

在执行过程中,`tool_execution_update` 事件会流式输出部分结果(例如 bash 输出会随执行进度陆续到达):

```json
{
  "type": "tool_execution_update",
  "toolCallId": "call_abc123",
  "toolName": "bash",
  "args": {"command": "ls -la"},
  "partialResult": {
    "content": [{"type": "text", "text": "partial output so far..."}],
    "details": {"truncation": null, "fullOutputPath": null}
  }
}
```

完成时:

```json
{
  "type": "tool_execution_end",
  "toolCallId": "call_abc123",
  "toolName": "bash",
  "result": {
    "content": [{"type": "text", "text": "total 48\n..."}],
    "details": {...}
  },
  "isError": false
}
```

使用 `toolCallId` 来关联相关事件。`tool_execution_update` 中的 `partialResult` 包含的是目前累积的完整输出(而不仅仅是增量部分),因此客户端在每次更新时只需直接替换显示内容即可。

### queue_update

每当待处理的引导消息或后续消息队列发生变化时触发。

```json
{
  "type": "queue_update",
  "steering": ["Focus on error handling"],
  "followUp": ["After that, summarize the result"]
}
```

### compaction_start / compaction_end

无论是手动压缩还是自动压缩,在压缩执行时都会触发该事件。

```json
{"type": "compaction_start", "reason": "threshold"}
```

`reason` 字段取值为 `"manual"`、`"threshold"` 或 `"overflow"`。

```json
{
  "type": "compaction_end",
  "reason": "threshold",
  "result": {
    "summary": "Summary of conversation...",
    "firstKeptEntryId": "abc123",
    "tokensBefore": 150000,
    "details": {}
  },
  "aborted": false,
  "willRetry": false
}
```

如果 `reason` 为 `"overflow"` 且压缩成功,`willRetry` 会是 `true`,代理会自动重试该提示词。

如果压缩被中止,`result` 为 `null` 且 `aborted` 为 `true`。

如果压缩失败(例如 API 配额已用尽),`result` 为 `null`、`aborted` 为 `false`,并且 `errorMessage` 会包含错误说明。

### auto_retry_start / auto_retry_end

当出现瞬时性错误(过载、速率限制、5xx)后触发自动重试时,会发出该事件。

```json
{
  "type": "auto_retry_start",
  "attempt": 1,
  "maxAttempts": 3,
  "delayMs": 2000,
  "errorMessage": "529 {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}"
}
```

```json
{
  "type": "auto_retry_end",
  "success": true,
  "attempt": 2
}
```

最终失败时(超过最大重试次数):
```json
{
  "type": "auto_retry_end",
  "success": false,
  "attempt": 3,
  "finalError": "529 overloaded_error: Overloaded"
}
```

### extension_error

当某个扩展抛出错误时触发。

```json
{
  "type": "extension_error",
  "extensionPath": "/path/to/extension.ts",
  "event": "tool_call",
  "error": "Error message..."
}
```

## 扩展 UI 协议

扩展可以通过 `ctx.ui.select()`、`ctx.ui.confirm()` 等方法请求用户交互。在 RPC 模式下,这些调用会被转换为构建在基础命令/事件流之上的一套请求/响应子协议。

扩展 UI 方法分为两类:

- **对话框方法**(`select`、`confirm`、`input`、`editor`):会在 stdout 上发出一个 `extension_ui_request`,并阻塞等待客户端通过 stdin 发回带有匹配 `id` 的 `extension_ui_response`。
- **触发即忘方法**(`notify`、`setStatus`、`setWidget`、`setTitle`、`set_editor_text`):会在 stdout 上发出一个 `extension_ui_request`,但不期望获得响应。客户端可以选择展示这些信息,也可以忽略。

如果某个对话框方法包含 `timeout` 字段,当超时到期时代理端会自动以默认值完成解析(resolve)。客户端无需自行跟踪超时。

由于需要直接访问 TUI,部分 `ExtensionUIContext` 方法在 RPC 模式下不受支持或功能受限:
- `custom()` 返回 `undefined`
- `setWorkingMessage()`、`setWorkingIndicator()`、`setFooter()`、`setHeader()`、`setEditorComponent()`、`setToolsExpanded()` 均为空操作(no-op)
- `getEditorText()` 返回 `""`
- `getToolsExpanded()` 返回 `false`
- `pasteToEditor()` 会委托给 `setEditorText()`(不处理粘贴/折叠逻辑)
- `getAllThemes()` 返回 `[]`
- `getTheme()` 返回 `undefined`
- `setTheme()` 返回 `{ success: false, error: "..." }`

注意:在 RPC 模式下 `ctx.hasUI` 为 `true`,因为对话框方法和触发即忘方法都可以通过扩展 UI 子协议正常工作。

### 扩展 UI 请求(stdout)

所有请求都带有 `type: "extension_ui_request"`、一个唯一的 `id`,以及一个 `method` 字段。

#### select

提示用户从列表中做出选择。带有 `timeout` 字段的对话框方法会以毫秒为单位包含超时时间;如果客户端未能及时响应,代理端会自动以 `undefined` 完成解析。

```json
{
  "type": "extension_ui_request",
  "id": "uuid-1",
  "method": "select",
  "title": "Allow dangerous command?",
  "options": ["Allow", "Block"],
  "timeout": 10000
}
```

期望的响应:带有 `value`(所选选项字符串)或 `cancelled: true` 的 `extension_ui_response`。

#### confirm

提示用户进行是/否确认。

```json
{
  "type": "extension_ui_request",
  "id": "uuid-2",
  "method": "confirm",
  "title": "Clear session?",
  "message": "All messages will be lost.",
  "timeout": 5000
}
```

期望的响应:带有 `confirmed: true/false` 或 `cancelled: true` 的 `extension_ui_response`。

#### input

提示用户输入自由格式的文本。

```json
{
  "type": "extension_ui_request",
  "id": "uuid-3",
  "method": "input",
  "title": "Enter a value",
  "placeholder": "type something..."
}
```

期望的响应:带有 `value`(输入的文本)或 `cancelled: true` 的 `extension_ui_response`。

#### editor

打开一个支持多行文本、可预填内容的编辑器。

```json
{
  "type": "extension_ui_request",
  "id": "uuid-4",
  "method": "editor",
  "title": "Edit some text",
  "prefill": "Line 1\nLine 2\nLine 3"
}
```

期望的响应:带有 `value`(编辑后的文本)或 `cancelled: true` 的 `extension_ui_response`。

#### notify

显示一条通知。属于触发即忘方法,不期望获得响应。

```json
{
  "type": "extension_ui_request",
  "id": "uuid-5",
  "method": "notify",
  "message": "Command blocked by user",
  "notifyType": "warning"
}
```

`notifyType` 字段取值为 `"info"`、`"warning"` 或 `"error"`。若省略则默认为 `"info"`。

#### setStatus

在页脚/状态栏中设置或清除一条状态信息。属于触发即忘方法。

```json
{
  "type": "extension_ui_request",
  "id": "uuid-6",
  "method": "setStatus",
  "statusKey": "my-ext",
  "statusText": "Turn 3 running..."
}
```

将 `statusText` 设为 `undefined`(或省略该字段)即可清除该 key 对应的状态信息。

#### setWidget

在编辑器上方或下方设置或清除一个小部件(widget,一段文本行)。属于触发即忘方法。

```json
{
  "type": "extension_ui_request",
  "id": "uuid-7",
  "method": "setWidget",
  "widgetKey": "my-ext",
  "widgetLines": ["--- My Widget ---", "Line 1", "Line 2"],
  "widgetPlacement": "aboveEditor"
}
```

将 `widgetLines` 设为 `undefined`(或省略该字段)即可清除该小部件。`widgetPlacement` 字段取值为 `"aboveEditor"`(默认值)或 `"belowEditor"`。RPC 模式下仅支持字符串数组;组件工厂函数会被忽略。

#### setTitle

设置终端窗口/标签页标题。属于触发即忘方法。

```json
{
  "type": "extension_ui_request",
  "id": "uuid-8",
  "method": "setTitle",
  "title": "pi - my project"
}
```

#### set_editor_text

设置输入编辑器中的文本。属于触发即忘方法。

```json
{
  "type": "extension_ui_request",
  "id": "uuid-9",
  "method": "set_editor_text",
  "text": "prefilled text for the user"
}
```

### 扩展 UI 响应(stdin)

响应仅针对对话框方法发送(`select`、`confirm`、`input`、`editor`)。`id` 必须与对应请求匹配。

#### 值类响应(select、input、editor)

```json
{"type": "extension_ui_response", "id": "uuid-1", "value": "Allow"}
```

#### 确认类响应(confirm)

```json
{"type": "extension_ui_response", "id": "uuid-2", "confirmed": true}
```

#### 取消类响应(适用于任意对话框方法)

用于关闭任意对话框方法。扩展会收到 `undefined`(针对 select/input/editor)或 `false`(针对 confirm)。

```json
{"type": "extension_ui_response", "id": "uuid-3", "cancelled": true}
```

## 错误处理

失败的命令会返回一个 `success: false` 的响应:

```json
{
  "type": "response",
  "command": "set_model",
  "success": false,
  "error": "Model not found: invalid/model"
}
```

解析错误:

```json
{
  "type": "response",
  "command": "parse",
  "success": false,
  "error": "Failed to parse command: Unexpected token..."
}
```

## 类型

源文件:
- [`packages/ai/src/types.ts`](../../ai/src/types.ts) - `Model`、`UserMessage`、`AssistantMessage`、`ToolResultMessage`
- [`packages/agent/src/types.ts`](../../agent/src/types.ts) - `AgentMessage`、`AgentEvent`
- [`src/core/messages.ts`](../src/core/messages.ts) - `BashExecutionMessage`
- [`src/modes/rpc/rpc-types.ts`](../src/modes/rpc/rpc-types.ts) - RPC 命令/响应类型、扩展 UI 请求/响应类型

### Model

```json
{
  "id": "claude-sonnet-4-20250514",
  "name": "Claude Sonnet 4",
  "api": "anthropic-messages",
  "provider": "anthropic",
  "baseUrl": "https://api.anthropic.com",
  "reasoning": true,
  "input": ["text", "image"],
  "contextWindow": 200000,
  "maxTokens": 16384,
  "cost": {
    "input": 3.0,
    "output": 15.0,
    "cacheRead": 0.3,
    "cacheWrite": 3.75
  }
}
```

### UserMessage

```json
{
  "role": "user",
  "content": "Hello!",
  "timestamp": 1733234567890,
  "attachments": []
}
```

`content` 字段可以是字符串,也可以是由 `TextContent`/`ImageContent` 块组成的数组。

### AssistantMessage

```json
{
  "role": "assistant",
  "content": [
    {"type": "text", "text": "Hello! How can I help?"},
    {"type": "thinking", "thinking": "User is greeting me..."},
    {"type": "toolCall", "id": "call_123", "name": "bash", "arguments": {"command": "ls"}}
  ],
  "api": "anthropic-messages",
  "provider": "anthropic",
  "model": "claude-sonnet-4-20250514",
  "usage": {
    "input": 100,
    "output": 50,
    "cacheRead": 0,
    "cacheWrite": 0,
    "cost": {"input": 0.0003, "output": 0.00075, "cacheRead": 0, "cacheWrite": 0, "total": 0.00105}
  },
  "stopReason": "stop",
  "timestamp": 1733234567890
}
```

结束原因(stop reason)取值:`"stop"`、`"length"`、`"toolUse"`、`"error"`、`"aborted"`

### ToolResultMessage

```json
{
  "role": "toolResult",
  "toolCallId": "call_123",
  "toolName": "bash",
  "content": [{"type": "text", "text": "total 48\ndrwxr-xr-x ..."}],
  "isError": false,
  "timestamp": 1733234567890
}
```

### BashExecutionMessage

由 `bash` 这一 RPC 命令创建(而非由 LLM 的工具调用创建):

```json
{
  "role": "bashExecution",
  "command": "ls -la",
  "output": "total 48\ndrwxr-xr-x ...",
  "exitCode": 0,
  "cancelled": false,
  "truncated": false,
  "fullOutputPath": null,
  "timestamp": 1733234567890
}
```

### Attachment

```json
{
  "id": "img1",
  "type": "image",
  "fileName": "photo.jpg",
  "mimeType": "image/jpeg",
  "size": 102400,
  "content": "base64-encoded-data...",
  "extractedText": null,
  "preview": null
}
```

## 示例:基础客户端(Python)

```python
import subprocess
import json

proc = subprocess.Popen(
    ["pi", "--mode", "rpc", "--no-session"],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    text=True
)

def send(cmd):
    proc.stdin.write(json.dumps(cmd) + "\n")
    proc.stdin.flush()

def read_events():
    for line in proc.stdout:
        yield json.loads(line)

# 发送提示词
send({"type": "prompt", "message": "Hello!"})

# 处理事件
for event in read_events():
    if event.get("type") == "message_update":
        delta = event.get("assistantMessageEvent", {})
        if delta.get("type") == "text_delta":
            print(delta["delta"], end="", flush=True)
    
    if event.get("type") == "agent_end":
        print()
        break
```

## 示例:交互式客户端(Node.js)

完整的交互式示例请参见 [`test/rpc-example.ts`](../test/rpc-example.ts),类型化的客户端实现请参见 [`src/modes/rpc/rpc-client.ts`](../src/modes/rpc/rpc-client.ts)。

关于处理扩展 UI 协议的完整示例,请参见 [`examples/rpc-extension-ui.ts`](../examples/rpc-extension-ui.ts),它与 [`examples/extensions/rpc-demo.ts`](../examples/extensions/rpc-demo.ts) 扩展配套使用。

```javascript
const { spawn } = require("child_process");
const { StringDecoder } = require("string_decoder");

const agent = spawn("pi", ["--mode", "rpc", "--no-session"]);

function attachJsonlReader(stream, onLine) {
    const decoder = new StringDecoder("utf8");
    let buffer = "";

    stream.on("data", (chunk) => {
        buffer += typeof chunk === "string" ? chunk : decoder.write(chunk);

        while (true) {
            const newlineIndex = buffer.indexOf("\n");
            if (newlineIndex === -1) break;

            let line = buffer.slice(0, newlineIndex);
            buffer = buffer.slice(newlineIndex + 1);
            if (line.endsWith("\r")) line = line.slice(0, -1);
            onLine(line);
        }
    });

    stream.on("end", () => {
        buffer += decoder.end();
        if (buffer.length > 0) {
            onLine(buffer.endsWith("\r") ? buffer.slice(0, -1) : buffer);
        }
    });
}

attachJsonlReader(agent.stdout, (line) => {
    const event = JSON.parse(line);

    if (event.type === "message_update") {
        const { assistantMessageEvent } = event;
        if (assistantMessageEvent.type === "text_delta") {
            process.stdout.write(assistantMessageEvent.delta);
        }
    }
});

// 发送提示词
agent.stdin.write(JSON.stringify({ type: "prompt", message: "Hello" }) + "\n");

// 在 Ctrl+C 时中止
process.on("SIGINT", () => {
    agent.stdin.write(JSON.stringify({ type: "abort" }) + "\n");
});
```
