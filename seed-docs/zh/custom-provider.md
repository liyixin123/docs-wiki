# 自定义 Provider

扩展可以通过 `pi.registerProvider()` 注册自定义的模型 provider。这可以实现:

- **代理(Proxies)** - 将请求路由经过企业代理或 API 网关
- **自定义端点** - 使用自建或私有的模型部署
- **OAuth/SSO** - 为企业级 provider 添加认证流程
- **自定义 API** - 为非标准 LLM API 实现流式传输

## 示例扩展

参见以下完整的 provider 示例:

- [`examples/extensions/custom-provider-anthropic/`](../examples/extensions/custom-provider-anthropic/)
- [`examples/extensions/custom-provider-gitlab-duo/`](../examples/extensions/custom-provider-gitlab-duo/)

## 目录

- [示例扩展](#example-extensions)
- [快速参考](#quick-reference)
- [覆盖已有 Provider](#override-existing-provider)
- [注册新 Provider](#register-new-provider)
- [取消注册 Provider](#unregister-provider)
- [OAuth 支持](#oauth-support)
- [自定义流式 API](#custom-streaming-api)
- [上下文溢出错误](#context-overflow-errors)
- [测试你的实现](#testing-your-implementation)
- [配置参考](#config-reference)
- [模型定义参考](#model-definition-reference)

## 快速参考

```typescript
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

export default function (pi: ExtensionAPI) {
  // 覆盖已有 provider 的 baseUrl
  pi.registerProvider("anthropic", {
    baseUrl: "https://proxy.example.com"
  });

  // 注册带有模型的新 provider
  pi.registerProvider("my-provider", {
    name: "My Provider",
    baseUrl: "https://api.example.com",
    apiKey: "$MY_API_KEY",
    api: "openai-completions",
    models: [
      {
        id: "my-model",
        name: "My Model",
        reasoning: false,
        input: ["text", "image"],
        cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
        contextWindow: 128000,
        maxTokens: 4096
      }
    ]
  });
}
```

扩展的工厂函数也可以是 `async` 的。如果需要动态发现模型,应在工厂函数中(而不是 `session_start` 中)完成模型的获取和注册。pi 会在启动继续之前等待工厂函数完成,因此该 provider 在交互式启动期间以及 `pi --list-models` 中都可用。

## 覆盖已有 Provider

最简单的用例:将已有 provider 的请求重定向经过代理。

```typescript
// 现在所有 Anthropic 请求都会经过你的代理
pi.registerProvider("anthropic", {
  baseUrl: "https://proxy.example.com"
});

// 为 OpenAI 请求添加自定义请求头
pi.registerProvider("openai", {
  headers: {
    "X-Custom-Header": "value"
  }
});

// 同时设置 baseUrl 和 headers
pi.registerProvider("google", {
  baseUrl: "https://ai-gateway.corp.com/google",
  headers: {
    "X-Corp-Auth": "$CORP_AUTH_TOKEN"  // 环境变量或字面量
  }
});
```

当只提供 `baseUrl` 和/或 `headers`(不提供 `models`)时,该 provider 现有的所有模型都会保留,只是改用新的端点。

## 注册新 Provider

要添加一个全新的 provider,需要连同必需的配置一起指定 `models`。

如果模型列表来自远程端点,请使用异步的扩展工厂函数:

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
    apiKey: "$LOCAL_OPENAI_API_KEY",
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

这样会在启动完成前就注册好获取到的模型。

```typescript
pi.registerProvider("my-llm", {
  baseUrl: "https://api.my-llm.com/v1",
  apiKey: "$MY_LLM_API_KEY",  // 环境变量引用
  api: "openai-completions",  // 使用哪种流式 API
  models: [
    {
      id: "my-llm-large",
      name: "My LLM Large",
      reasoning: true,        // 支持扩展思维
      input: ["text", "image"],
      cost: {
        input: 3.0,           // 每百万 token 的美元价格
        output: 15.0,
        cacheRead: 0.3,
        cacheWrite: 3.75
      },
      contextWindow: 200000,
      maxTokens: 16384
    }
  ]
});
```

当提供了 `models` 时,它会**替换**该 provider 现有的所有模型。

`apiKey` 与自定义请求头的取值使用与 `models.json` 相同的配置取值语法:以 `!command` 开头会将整个值作为命令执行,`$ENV_VAR` 和 `${ENV_VAR}` 用于插值环境变量,`$$` 输出字面量 `$`,`$!` 输出字面量 `!`。

## 取消注册 Provider

使用 `pi.unregisterProvider(name)` 移除之前通过 `pi.registerProvider(name, ...)` 注册的 provider:

```typescript
// 注册
pi.registerProvider("my-llm", {
  baseUrl: "https://api.my-llm.com/v1",
  apiKey: "$MY_LLM_API_KEY",
  api: "openai-completions",
  models: [
    {
      id: "my-llm-large",
      name: "My LLM Large",
      reasoning: true,
      input: ["text", "image"],
      cost: { input: 3.0, output: 15.0, cacheRead: 0.3, cacheWrite: 3.75 },
      contextWindow: 200000,
      maxTokens: 16384
    }
  ]
});

// 之后移除它
pi.unregisterProvider("my-llm");
```

取消注册会移除该 provider 的动态模型、API key 回退、OAuth provider 注册以及自定义的流处理器注册。任何被该 provider 覆盖的内置模型或 provider 行为都会被恢复。

在初始扩展加载阶段之后进行的调用会立即生效,不需要执行 `/reload`。

### API 类型

`api` 字段决定使用哪种流式实现:

| API | 适用场景 |
|-----|---------|
| `anthropic-messages` | Anthropic Claude API 及其兼容实现 |
| `openai-completions` | OpenAI Chat Completions API 及其兼容实现 |
| `openai-responses` | OpenAI Responses API |
| `azure-openai-responses` | Azure OpenAI Responses API |
| `openai-codex-responses` | OpenAI Codex Responses API |
| `mistral-conversations` | Mistral SDK Conversations/Chat 流式接口 |
| `google-generative-ai` | Google Generative AI API |
| `google-vertex` | Google Vertex AI API |
| `bedrock-converse-stream` | Amazon Bedrock Converse API |

大多数兼容 OpenAI 的 provider 使用 `openai-completions` 即可正常工作。使用模型级别的 `thinkingLevelMap` 来设置模型特定的思维等级,使用 `compat` 来处理 provider 的特殊情况:

```typescript
models: [{
  id: "custom-model",
  // ...
  reasoning: true,
  thinkingLevelMap: {              // 将 pi 的等级映射为 provider 的取值;null 表示隐藏不支持的等级
    minimal: null,
    low: null,
    medium: null,
    high: "default",
    xhigh: "max"
  },
  compat: {
    supportsDeveloperRole: false,   // 使用 "system" 而非 "developer"
    supportsReasoningEffort: true,
    maxTokensField: "max_tokens",   // 而非 "max_completion_tokens"
    requiresToolResultName: true,   // 工具结果需要 name 字段
    thinkingFormat: "qwen",        // 顶层的 enable_thinking: true
    cacheControlFormat: "anthropic" // Anthropic 风格的 cache_control 标记
  }
}]
```

对于 OpenRouter 风格的 `reasoning: { effort }` 控制方式,使用 `openrouter`。对于 Together 风格的 `reasoning: { enabled }` 控制方式,使用 `together`;若同时开启 `supportsReasoningEffort`,还会发送 `reasoning_effort`。对于读取 `chat_template_kwargs.enable_thinking` 的本地 Qwen 兼容服务端,请改用 `qwen-chat-template`。
对于通过 `cache_control` 在系统提示词、最后一个工具定义以及最后一条用户/assistant 文本内容上暴露 Anthropic 风格 prompt caching 的兼容 OpenAI provider,使用 `cacheControlFormat: "anthropic"`。

对于使用 `api: "anthropic-messages"` 的兼容 Anthropic provider,如果其上游模型需要自适应思维(`thinking.type: "adaptive"` 加 `output_config.effort`),请在模型或 provider 上设置 `compat.forceAdaptiveThinking: true`。内置的自适应 Claude 模型会自动设置该项。只有那些会发出空思维签名并在重放时期望得到 `signature: ""` 的 provider,才应设置 `compat.allowEmptySignature: true`。

> 迁移说明:Mistral 已从 `openai-completions` 迁移到 `mistral-conversations`。
> 原生 Mistral 模型请使用 `mistral-conversations`。
> 如果你有意通过 `openai-completions` 路由兼容 Mistral 的自定义端点,请根据需要显式设置 `compat` 标志。

### Auth Header(鉴权请求头)

如果你的 provider 期望 `Authorization: Bearer <key>`,但并未使用标准 API,请将 `authHeader` 设为 `true`:

```typescript
pi.registerProvider("custom-api", {
  baseUrl: "https://api.example.com",
  apiKey: "$MY_API_KEY",
  authHeader: true,  // 添加 Authorization: Bearer 请求头
  api: "openai-completions",
  models: [...]
});
```

## OAuth 支持

添加与 `/login` 集成的 OAuth/SSO 认证:

```typescript
import type { OAuthCredentials, OAuthLoginCallbacks } from "@earendil-works/pi-ai";

pi.registerProvider("corporate-ai", {
  baseUrl: "https://ai.corp.com/v1",
  api: "openai-responses",
  models: [...],
  oauth: {
    name: "Corporate AI (SSO)",

    async login(callbacks: OAuthLoginCallbacks): Promise<OAuthCredentials> {
      const method = await callbacks.onSelect({
        message: "Select login method:",
        options: [
          { id: "browser", label: "Browser OAuth" },
          { id: "device", label: "Device code" }
        ]
      });
      if (!method) throw new Error("Login cancelled");

      let code: string;
      if (method === "device") {
        callbacks.onDeviceCode({
          userCode: "ABCD-1234",
          verificationUri: "https://sso.corp.com/device",
          intervalSeconds: 5,
          expiresInSeconds: 900
        });
        code = await pollDeviceCodeUntilComplete();
      } else {
        callbacks.onAuth({ url: "https://sso.corp.com/authorize?..." });
        code = await callbacks.onPrompt({ message: "Enter SSO code:" });
      }

      // 兑换令牌(由你自己实现)
      const tokens = await exchangeCodeForTokens(code);

      return {
        refresh: tokens.refreshToken,
        access: tokens.accessToken,
        expires: Date.now() + tokens.expiresIn * 1000
      };
    },

    async refreshToken(credentials: OAuthCredentials): Promise<OAuthCredentials> {
      const tokens = await refreshAccessToken(credentials.refresh);
      return {
        refresh: tokens.refreshToken ?? credentials.refresh,
        access: tokens.accessToken,
        expires: Date.now() + tokens.expiresIn * 1000
      };
    },

    getApiKey(credentials: OAuthCredentials): string {
      return credentials.access;
    },

    // 可选:根据用户的订阅情况修改模型
    modifyModels(models, credentials) {
      const region = decodeRegionFromToken(credentials.access);
      return models.map(m => ({
        ...m,
        baseUrl: `https://${region}.ai.corp.com/v1`
      }));
    }
  }
});
```

注册完成后,用户可以通过 `/login corporate-ai` 进行认证。

### OAuthLoginCallbacks

`callbacks` 对象提供了三种认证方式:

```typescript
interface OAuthLoginCallbacks {
  // 在浏览器中打开 URL(用于 OAuth 重定向)
  onAuth(params: { url: string }): void;

  // 显示设备码(用于设备授权流程)
  onDeviceCode(params: {
    userCode: string;
    verificationUri: string;
    intervalSeconds?: number;
    expiresInSeconds?: number;
  }): void;

  // 提示用户输入(用于手动输入令牌)
  onPrompt(params: { message: string }): Promise<string>;

  // 显示交互式选择器,例如让用户选择浏览器 OAuth 还是设备码方式
  onSelect(params: {
    message: string;
    options: { id: string; label: string }[];
  }): Promise<string | undefined>;
}
```

### OAuthCredentials

凭证会持久化保存在 `~/.pi/agent/auth.json` 中:

```typescript
interface OAuthCredentials {
  refresh: string;   // 刷新令牌(用于 refreshToken())
  access: string;    // 访问令牌(由 getApiKey() 返回)
  expires: number;   // 过期时间戳(毫秒)
}
```

## 自定义流式 API

对于使用非标准 API 的 provider,需要实现 `streamSimple`。在编写自己的实现之前,请先研究现有的 provider 实现:

**参考实现:**
- [anthropic.ts](https://github.com/earendil-works/pi-mono/blob/main/packages/ai/src/providers/anthropic.ts) - Anthropic Messages API
- [mistral.ts](https://github.com/earendil-works/pi-mono/blob/main/packages/ai/src/providers/mistral.ts) - Mistral Conversations API
- [openai-completions.ts](https://github.com/earendil-works/pi-mono/blob/main/packages/ai/src/providers/openai-completions.ts) - OpenAI Chat Completions
- [openai-responses.ts](https://github.com/earendil-works/pi-mono/blob/main/packages/ai/src/providers/openai-responses.ts) - OpenAI Responses API
- [google.ts](https://github.com/earendil-works/pi-mono/blob/main/packages/ai/src/providers/google.ts) - Google Generative AI
- [amazon-bedrock.ts](https://github.com/earendil-works/pi-mono/blob/main/packages/ai/src/providers/amazon-bedrock.ts) - AWS Bedrock

### 流式模式(Stream Pattern)

所有 provider 都遵循相同的模式:

```typescript
import {
  type AssistantMessage,
  type AssistantMessageEventStream,
  type Context,
  type Model,
  type SimpleStreamOptions,
  calculateCost,
  createAssistantMessageEventStream,
} from "@earendil-works/pi-ai";

function streamMyProvider(
  model: Model<any>,
  context: Context,
  options?: SimpleStreamOptions
): AssistantMessageEventStream {
  const stream = createAssistantMessageEventStream();

  (async () => {
    // 初始化输出消息
    const output: AssistantMessage = {
      role: "assistant",
      content: [],
      api: model.api,
      provider: model.provider,
      model: model.id,
      usage: {
        input: 0,
        output: 0,
        cacheRead: 0,
        cacheWrite: 0,
        totalTokens: 0,
        cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
      },
      stopReason: "stop",
      timestamp: Date.now(),
    };

    try {
      // 推送 start 事件
      stream.push({ type: "start", partial: output });

      // 发起 API 请求并处理响应……
      // 随内容到达推送相应事件……

      // 推送 done 事件
      stream.push({
        type: "done",
        reason: output.stopReason as "stop" | "length" | "toolUse",
        message: output
      });
      stream.end();
    } catch (error) {
      output.stopReason = options?.signal?.aborted ? "aborted" : "error";
      output.errorMessage = error instanceof Error ? error.message : String(error);
      stream.push({ type: "error", reason: output.stopReason, error: output });
      stream.end();
    }
  })();

  return stream;
}
```

### 事件类型

通过 `stream.push()` 按以下顺序推送事件:

1. `{ type: "start", partial: output }` - 流已开始

2. 内容事件(可重复出现,每个内容块都要追踪各自的 `contentIndex`):
   - `{ type: "text_start", contentIndex, partial }` - 文本块开始
   - `{ type: "text_delta", contentIndex, delta, partial }` - 文本增量片段
   - `{ type: "text_end", contentIndex, content, partial }` - 文本块结束
   - `{ type: "thinking_start", contentIndex, partial }` - 思维块开始
   - `{ type: "thinking_delta", contentIndex, delta, partial }` - 思维增量片段
   - `{ type: "thinking_end", contentIndex, content, partial }` - 思维块结束
   - `{ type: "toolcall_start", contentIndex, partial }` - 工具调用开始
   - `{ type: "toolcall_delta", contentIndex, delta, partial }` - 工具调用的 JSON 增量片段
   - `{ type: "toolcall_end", contentIndex, toolCall, partial }` - 工具调用结束

3. `{ type: "done", reason, message }` 或 `{ type: "error", reason, error }` - 流已结束

每个事件中的 `partial` 字段包含当前的 `AssistantMessage` 状态。在接收到数据时更新 `output.content`,然后将 `output` 作为 `partial` 传入。

### 内容块(Content Blocks)

在内容块到达时将其添加到 `output.content`:

```typescript
// 文本块
output.content.push({ type: "text", text: "" });
stream.push({ type: "text_start", contentIndex: output.content.length - 1, partial: output });

// 文本到达时
const block = output.content[contentIndex];
if (block.type === "text") {
  block.text += delta;
  stream.push({ type: "text_delta", contentIndex, delta, partial: output });
}

// 内容块完成时
stream.push({ type: "text_end", contentIndex, content: block.text, partial: output });
```

### 工具调用

工具调用需要不断累积 JSON 并进行解析:

```typescript
// 开始工具调用
output.content.push({
  type: "toolCall",
  id: toolCallId,
  name: toolName,
  arguments: {}
});
stream.push({ type: "toolcall_start", contentIndex: output.content.length - 1, partial: output });

// 累积 JSON
let partialJson = "";
partialJson += jsonDelta;
try {
  block.arguments = JSON.parse(partialJson);
} catch {}
stream.push({ type: "toolcall_delta", contentIndex, delta: jsonDelta, partial: output });

// 完成
stream.push({
  type: "toolcall_end",
  contentIndex,
  toolCall: { type: "toolCall", id, name, arguments: block.arguments },
  partial: output
});
```

### 用量与成本

根据 API 响应更新用量并计算成本:

```typescript
output.usage.input = response.usage.input_tokens;
output.usage.output = response.usage.output_tokens;
output.usage.cacheRead = response.usage.cache_read_tokens ?? 0;
output.usage.cacheWrite = response.usage.cache_write_tokens ?? 0;
output.usage.totalTokens = output.usage.input + output.usage.output +
                           output.usage.cacheRead + output.usage.cacheWrite;
calculateCost(model, output.usage);
```

### 上下文溢出错误

当请求超出模型的上下文窗口时,pi 可以通过压缩对话并重试来自动恢复。只有当 pi 能将该失败识别为溢出错误时,这种恢复机制才会启动。

检测是针对最终确定的 assistant 消息进行的:

- `stopReason === "error"`
- `errorMessage` 匹配 pi 已知的某个溢出模式之一(参见 [`packages/ai/src/utils/overflow.ts`](https://github.com/earendil-works/pi-mono/blob/main/packages/ai/src/utils/overflow.ts))

如果你的 provider 返回的溢出错误信息 pi 无法识别,请在注册该 provider 的同一个扩展中对错误进行归一化处理。使用 `message_end` 处理程序重写 assistant 消息,使其 `errorMessage` 以 pi 能识别的短语开头。通用的兜底短语 `context_length_exceeded` 是最安全的选择。

```typescript
const MY_PROVIDER_OVERFLOW_PATTERN = /your provider's overflow phrase/i;

export default function (pi: ExtensionAPI) {
  pi.registerProvider("my-provider", { /* ... */ });

  pi.on("message_end", (event, ctx) => {
    const message = event.message;
    if (message.role !== "assistant") return;
    if (message.stopReason !== "error") return;
    if (
      message.provider !== "my-provider" &&
      ctx.model?.provider !== "my-provider"
    )
      return;

    const errorMessage = message.errorMessage ?? "";
    if (errorMessage.includes("context_length_exceeded")) return;
    if (!MY_PROVIDER_OVERFLOW_PATTERN.test(errorMessage)) return;

    return {
      message: {
        ...message,
        errorMessage: `context_length_exceeded: ${errorMessage}`,
      },
    };
  });
}
```

`message_end` 会在 pi 为自动压缩追踪该 assistant 消息之前运行,因此 pi 检查的正是被重写后的 `errorMessage`。设置好之后,pi 将会:

1. 从 `errorMessage` 中检测出溢出。
2. 将失败的 assistant 消息从活动上下文中移除。
3. 执行压缩。
4. 重试一次该请求。

请谨慎地为这个重写逻辑加上防护:

- 将其限定在你自己的 provider 范围内(通过 `message.provider` 和 `ctx.model?.provider`),这样来自其他 provider 的无关错误就不会受到影响。
- 匹配的是特定于该 provider 的模式,而不是 pi 的通用溢出模式。如果重写了限流或节流类的错误(`rate limit`、`too many requests`),会错误地触发压缩,而不是走 pi 正常的带退避重试路径。
- 当 `errorMessage` 已经包含 `context_length_exceeded` 时应跳过处理,以确保该处理程序是幂等的。

### 注册

注册你的流式函数:

```typescript
pi.registerProvider("my-provider", {
  baseUrl: "https://api.example.com",
  apiKey: "$MY_API_KEY",
  api: "my-custom-api",
  models: [...],
  streamSimple: streamMyProvider
});
```

## 测试你的实现

针对内置 provider 所使用的相同测试套件来测试你的 provider。从 [packages/ai/test/](https://github.com/earendil-works/pi-mono/tree/main/packages/ai/test) 中复制并改编以下测试文件:

| 测试 | 目的 |
|------|---------|
| `stream.test.ts` | 基础流式传输、文本输出 |
| `tokens.test.ts` | Token 计数与用量 |
| `abort.test.ts` | AbortSignal 处理 |
| `empty.test.ts` | 空/最小化响应 |
| `context-overflow.test.ts` | 上下文窗口限制 |
| `image-limits.test.ts` | 图片输入处理 |
| `unicode-surrogate.test.ts` | Unicode 边界情况 |
| `tool-call-without-result.test.ts` | 工具调用的边界情况 |
| `image-tool-result.test.ts` | 工具结果中的图片 |
| `total-tokens.test.ts` | 总 token 数计算 |
| `cross-provider-handoff.test.ts` | provider 之间的上下文交接 |

使用你的 provider/模型组合运行测试以验证兼容性。

## 配置参考

```typescript
interface ProviderConfig {
  /** 在 UI(如 /login)中显示的 provider 名称。 */
  name?: string;

  /** API 端点 URL。定义模型时必填。 */
  baseUrl?: string;

  /** API key 字面量、环境变量插值($ENV_VAR 或 ${ENV_VAR})或 !command。定义模型时必填(除非使用 oauth)。 */
  apiKey?: string;

  /** 用于流式传输的 API 类型。定义模型时,在 provider 或模型级别必填。 */
  api?: Api;

  /** 针对非标准 API 的自定义流式实现。 */
  streamSimple?: (
    model: Model<Api>,
    context: Context,
    options?: SimpleStreamOptions
  ) => AssistantMessageEventStream;

  /** 请求中包含的自定义请求头。取值使用与 apiKey 相同的解析语法。 */
  headers?: Record<string, string>;

  /** 若为 true,会添加带有解析后 API key 的 Authorization: Bearer 请求头。 */
  authHeader?: boolean;

  /** 要注册的模型。如果提供,会替换该 provider 现有的所有模型。 */
  models?: ProviderModelConfig[];

  /** 用于支持 /login 的 OAuth provider。 */
  oauth?: {
    name: string;
    login(callbacks: OAuthLoginCallbacks): Promise<OAuthCredentials>;
    refreshToken(credentials: OAuthCredentials): Promise<OAuthCredentials>;
    getApiKey(credentials: OAuthCredentials): string;
    modifyModels?(models: Model<Api>[], credentials: OAuthCredentials): Model<Api>[];
  };
}
```

## 模型定义参考

```typescript
interface ProviderModelConfig {
  /** 模型 ID(例如 "claude-sonnet-4-20250514")。 */
  id: string;

  /** 显示名称(例如 "Claude 4 Sonnet")。 */
  name: string;

  /** 针对该特定模型的 API 类型覆盖。 */
  api?: Api;

  /** 针对该特定模型的 API 端点 URL 覆盖。 */
  baseUrl?: string;

  /** 该模型是否支持扩展思维。 */
  reasoning: boolean;

  /** 将 pi 的思维等级映射为 provider/模型特定的取值;null 表示该等级不受支持。 */
  thinkingLevelMap?: Partial<Record<"off" | "minimal" | "low" | "medium" | "high" | "xhigh", string | null>>;

  /** 支持的输入类型。 */
  input: ("text" | "image")[];

  /** 每百万 token 的成本(用于用量追踪)。 */
  cost: {
    input: number;
    output: number;
    cacheRead: number;
    cacheWrite: number;
  };

  /** 最大上下文窗口大小(以 token 计)。 */
  contextWindow: number;

  /** 最大输出 token 数。 */
  maxTokens: number;

  /** 针对该特定模型的自定义请求头。 */
  headers?: Record<string, string>;

  /** 所选 API 的兼容性设置。 */
  compat?: {
    // openai-completions
    supportsStore?: boolean;
    supportsDeveloperRole?: boolean;
    supportsReasoningEffort?: boolean;
    supportsUsageInStreaming?: boolean;
    maxTokensField?: "max_completion_tokens" | "max_tokens";
    requiresToolResultName?: boolean;
    requiresAssistantAfterToolResult?: boolean;
    requiresThinkingAsText?: boolean;
    requiresReasoningContentOnAssistantMessages?: boolean;
    thinkingFormat?: "openai" | "openrouter" | "deepseek" | "together" | "zai" | "qwen" | "qwen-chat-template";
    cacheControlFormat?: "anthropic";

    // anthropic-messages
    supportsEagerToolInputStreaming?: boolean;
    supportsLongCacheRetention?: boolean;
    sendSessionAffinityHeaders?: boolean;
    supportsCacheControlOnTools?: boolean;
    forceAdaptiveThinking?: boolean;
    allowEmptySignature?: boolean;
  };
}
```

`openrouter` 发送 `reasoning: { effort }`。`deepseek` 发送 `thinking: { type: "enabled" | "disabled" }`,启用时还会发送 `reasoning_effort`。`together` 发送 `reasoning: { enabled }`,当启用 `supportsReasoningEffort` 时也会发送 `reasoning_effort`。`qwen` 适用于 DashScope 风格的顶层 `enable_thinking`。对于读取 `chat_template_kwargs.enable_thinking` 的本地 Qwen 兼容服务端,请使用 `qwen-chat-template`。
`cacheControlFormat: "anthropic"` 会将 Anthropic 风格的 `cache_control` 标记应用到系统提示词、最后一个工具定义以及最后一条用户/assistant 文本内容上。
