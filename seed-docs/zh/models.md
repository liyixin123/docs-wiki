# 自定义模型

通过 `~/.pi/agent/models.json` 添加自定义 provider 和模型(Ollama、vLLM、LM Studio、代理等)。

## 目录

- [最简示例](#minimal-example)
- [完整示例](#full-example)
- [支持的 API](#supported-apis)
- [Provider 配置](#provider-configuration)
- [模型配置](#model-configuration)
- [覆盖内置 Provider](#overriding-built-in-providers)
- [按模型覆盖](#per-model-overrides)
- [Anthropic Messages 兼容性](#anthropic-messages-compatibility)
- [OpenAI 兼容性](#openai-compatibility)

## 最简示例

对于本地模型(Ollama、LM Studio、vLLM),每个模型只需要 `id` 字段:

```json
{
  "providers": {
    "ollama": {
      "baseUrl": "http://localhost:11434/v1",
      "api": "openai-completions",
      "apiKey": "ollama",
      "models": [
        { "id": "llama3.1:8b" },
        { "id": "qwen2.5-coder:7b" }
      ]
    }
  }
}
```

`apiKey` 是必填字段,但 Ollama 会忽略它,因此填任意值都可以。

部分兼容 OpenAI 的服务端并不理解用于具备推理能力的模型的 `developer` 角色。对于这些 provider,将 `compat.supportsDeveloperRole` 设为 `false`,这样 pi 会改用 `system` 消息发送系统提示词。如果服务端也不支持 `reasoning_effort`,则同时将 `compat.supportsReasoningEffort` 设为 `false`。

你可以在 provider 级别设置 `compat` 以应用于所有模型,也可以在模型级别设置以覆盖特定模型。这种情况常见于 Ollama、vLLM、SGLang 及类似的兼容 OpenAI 的服务端。

```json
{
  "providers": {
    "ollama": {
      "baseUrl": "http://localhost:11434/v1",
      "api": "openai-completions",
      "apiKey": "ollama",
      "compat": {
        "supportsDeveloperRole": false,
        "supportsReasoningEffort": false
      },
      "models": [
        {
          "id": "gpt-oss:20b",
          "reasoning": true
        }
      ]
    }
  }
}
```

## 完整示例

在需要指定具体值时可以覆盖默认值:

```json
{
  "providers": {
    "ollama": {
      "baseUrl": "http://localhost:11434/v1",
      "api": "openai-completions",
      "apiKey": "ollama",
      "models": [
        {
          "id": "llama3.1:8b",
          "name": "Llama 3.1 8B (Local)",
          "reasoning": false,
          "input": ["text"],
          "contextWindow": 128000,
          "maxTokens": 32000,
          "cost": { "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0 }
        }
      ]
    }
  }
}
```

每次打开 `/model` 时该文件都会重新加载。可以在会话过程中编辑,无需重启。

## Google AI Studio 示例

使用 `google-generative-ai` 并配合 `baseUrl` 即可添加 Google AI Studio 中的模型,包括自定义的 Gemma 4 条目:

```json
{
  "providers": {
    "my-google": {
      "baseUrl": "https://generativelanguage.googleapis.com/v1beta",
      "api": "google-generative-ai",
      "apiKey": "$GEMINI_API_KEY",
      "models": [
        {
          "id": "gemma-4-31b-it",
          "name": "Gemma 4 31B",
          "input": ["text", "image"],
          "contextWindow": 262144,
          "reasoning": true
        }
      ]
    }
  }
}
```

在 `google-generative-ai` API 类型下添加自定义模型时,`baseUrl` 是必填项。

## 支持的 API

| API | 说明 |
|-----|-------------|
| `openai-completions` | OpenAI Chat Completions(兼容性最好) |
| `openai-responses` | OpenAI Responses API |
| `anthropic-messages` | Anthropic Messages API |
| `google-generative-ai` | Google Generative AI |

`api` 可以在 provider 级别设置(作为所有模型的默认值),也可以在模型级别设置(针对单个模型覆盖)。

## Provider 配置

| 字段 | 说明 |
|-------|-------------|
| `baseUrl` | API 端点 URL |
| `api` | API 类型(见上文) |
| `apiKey` | API 密钥(取值解析规则见下文) |
| `headers` | 自定义请求头(取值解析规则见下文) |
| `authHeader` | 设为 `true` 可自动添加 `Authorization: Bearer <apiKey>` |
| `models` | 模型配置数组 |
| `modelOverrides` | 针对该 provider 下内置模型的按模型覆盖配置 |

### 取值解析

`apiKey` 和 `headers` 字段支持命令执行、环境变量插值以及字面量:

- **Shell 命令:** 以 `"!command"` 开头会将整个值作为命令执行,并使用其标准输出
  ```json
  "apiKey": "!security find-generic-password -ws 'anthropic'"
  "apiKey": "!op read 'op://vault/item/credential'"
  ```
- **环境变量插值:** `"$ENV_VAR"` 或 `"${ENV_VAR}"` 会使用对应变量的值。插值也可以出现在更长的字面量内部。
  ```json
  "apiKey": "$MY_API_KEY"
  "apiKey": "${KEY_PREFIX}_${KEY_SUFFIX}"
  ```
  `$FOO_BAR` 表示变量 `FOO_BAR`;当 `BAR` 是字面文本时应使用 `${FOO}_BAR`。缺失的环境变量会导致该值无法解析。
- **转义:** `"$$"` 会输出字面量 `"$"`;`"$!"` 会输出字面量 `"!"` 而不会触发命令执行。
  ```json
  "apiKey": "$$literal-dollar-prefix"
  "apiKey": "$!literal-bang-prefix"
  ```
- **字面量值:** 直接使用
  ```json
  "apiKey": "sk-..."
  ```

启动时,类似 `MY_API_KEY` 这种旧式的全大写环境变量写法会被自动迁移为 `$MY_API_KEY`。

对于 `models.json`,shell 命令是在请求时解析的。pi 有意不为任意命令提供内置的 TTL、旧值复用或恢复逻辑。不同命令需要不同的缓存与失败策略,pi 无法推断出正确的做法。

如果你的命令执行缓慢、成本高、有速率限制,或者希望在瞬时失败时继续使用之前的值,请将其封装到你自己的脚本或命令中,由该脚本实现你想要的缓存或 TTL 行为。

`/model` 的可用性检查只使用已配置的鉴权信息,不会执行 shell 命令。

### 自定义请求头

```json
{
  "providers": {
    "custom-proxy": {
      "baseUrl": "https://proxy.example.com/v1",
      "apiKey": "$MY_API_KEY",
      "api": "anthropic-messages",
      "headers": {
        "x-portkey-api-key": "$PORTKEY_API_KEY",
        "x-secret": "!op read 'op://vault/item/secret'"
      },
      "models": [...]
    }
  }
}
```

## 模型配置

| 字段 | 是否必填 | 默认值 | 说明 |
|-------|----------|---------|-------------|
| `id` | 是 | — | 模型标识符(传递给 API) |
| `name` | 否 | `id` | 人类可读的模型标签。用于匹配(`--model` 模式)以及在模型详情/状态文本中显示。 |
| `api` | 否 | provider 的 `api` | 为该模型覆盖 provider 的 API |
| `reasoning` | 否 | `false` | 是否支持扩展思维(extended thinking) |
| `thinkingLevelMap` | 否 | 省略 | 将 pi 的思维等级映射为 provider 的取值,并标记不支持的等级(见下文) |
| `input` | 否 | `["text"]` | 输入类型:`["text"]` 或 `["text", "image"]` |
| `contextWindow` | 否 | `128000` | 上下文窗口大小(以 token 计) |
| `maxTokens` | 否 | `16384` | 最大输出 token 数 |
| `cost` | 否 | 全部为零 | `{"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0}`(每百万 token 的价格) |
| `compat` | 否 | provider 的 `compat` | Provider 兼容性覆盖项。若同时设置了 provider 级别的 `compat`,两者会合并。 |

当前行为:
- `/model` 与 `--list-models` 会按模型 `id` 列出条目。
- 配置的 `name` 用于模型匹配以及详情/状态文本展示。

### 思维等级映射(Thinking Level Map)

在模型上使用 `thinkingLevelMap` 来描述模型特定的思维控制方式。键为 pi 的思维等级:`off`、`minimal`、`low`、`medium`、`high`、`xhigh`。

取值有三种状态:

| 取值 | 含义 |
|-------|---------|
| 省略 | 该等级受支持,并使用 provider 的默认映射 |
| 字符串 | 该等级受支持,并将该值发送给 provider |
| `null` | 该等级不受支持,会被隐藏/跳过/限制掉 |

以下示例针对一个只支持 off、high 和 max 三种推理程度的模型:

```json
{
  "id": "deepseek-v4-pro",
  "reasoning": true,
  "thinkingLevelMap": {
    "minimal": null,
    "low": null,
    "medium": null,
    "high": "high",
    "xhigh": "max"
  }
}
```

以下示例针对一个无法关闭思维的模型:

```json
{
  "id": "always-thinking-model",
  "reasoning": true,
  "thinkingLevelMap": {
    "off": null
  }
}
```

迁移说明:此前使用 `compat.reasoningEffortMap` 的旧配置应将该映射迁移到模型级别的 `thinkingLevelMap`。对于不应出现在 UI 中的等级,请使用 `null`。

## 覆盖内置 Provider

无需重新定义模型,即可将内置 provider 的请求路由到代理:

```json
{
  "providers": {
    "anthropic": {
      "baseUrl": "https://my-proxy.example.com/v1"
    }
  }
}
```

所有内置的 Anthropic 模型依然可用。现有的 OAuth 或 API key 鉴权方式继续有效。

若要将自定义模型合并进某个内置 provider,请包含 `models` 数组:

```json
{
  "providers": {
    "anthropic": {
      "baseUrl": "https://my-proxy.example.com/v1",
      "apiKey": "$ANTHROPIC_API_KEY",
      "api": "anthropic-messages",
      "models": [...]
    }
  }
}
```

合并语义:
- 内置模型会被保留。
- 自定义模型会按 `id` 在该 provider 内进行 upsert(存在则更新,不存在则插入)。
- 如果自定义模型的 `id` 与某个内置模型的 `id` 相同,自定义模型会替换该内置模型。
- 如果自定义模型的 `id` 是新的,它会与内置模型并存添加。

## 按模型覆盖

使用 `modelOverrides` 可以在不替换 provider 完整模型列表的情况下,自定义特定的内置模型。

```json
{
  "providers": {
    "openrouter": {
      "modelOverrides": {
        "anthropic/claude-sonnet-4": {
          "name": "Claude Sonnet 4 (Bedrock Route)",
          "compat": {
            "openRouterRouting": {
              "only": ["amazon-bedrock"]
            }
          }
        }
      }
    }
  }
}
```

`modelOverrides` 针对每个模型支持以下字段:`name`、`reasoning`、`input`、`cost`(部分覆盖)、`contextWindow`、`maxTokens`、`headers`、`compat`。

行为说明:
- `modelOverrides` 会应用于内置 provider 模型。
- 未知的模型 ID 会被忽略。
- 你可以将 provider 级别的 `baseUrl`/`headers` 与 `modelOverrides` 结合使用。
- 如果同时为某个 provider 定义了 `models`,自定义模型会在内置覆盖之后进行合并。与内置模型 `id` 相同的自定义模型会替换被覆盖后的内置模型条目。

## Anthropic Messages 兼容性

对于使用 `api: "anthropic-messages"` 的 provider 或代理,可通过 `compat` 控制 Anthropic 特定的请求兼容性。

默认情况下 pi 会在每个工具上发送 `eager_input_streaming: true`。如果某个代理或兼容 Anthropic 的后端拒绝这个字段,请将 `supportsEagerToolInputStreaming` 设为 `false`。pi 会省略 `tools[].eager_input_streaming`,并在启用工具的请求中改为发送旧版的 `fine-grained-tool-streaming-2025-05-14` beta 请求头。

某些 Anthropic 模型需要自适应思维(`thinking.type: "adaptive"` 加上 `output_config.effort`),而不是旧版基于预算的思维负载。内置模型会自动设置该项。对于路由到这些模型的自定义 provider 或别名,请将 `forceAdaptiveThinking` 设为 `true`。

部分兼容 Anthropic 的 provider 会发出带有空签名的思维块,并在重放时仍期望其存在。只有这些 provider 才应将 `allowEmptySignature` 设为 `true`;真正的 Anthropic 会拒绝空的思维签名。

```json
{
  "providers": {
    "anthropic-proxy": {
      "baseUrl": "https://proxy.example.com",
      "api": "anthropic-messages",
      "apiKey": "$ANTHROPIC_PROXY_KEY",
      "compat": {
        "supportsEagerToolInputStreaming": false,
        "supportsLongCacheRetention": true,
        "forceAdaptiveThinking": true,
        "allowEmptySignature": true
      },
      "models": [
        {
          "id": "claude-opus-4-7",
          "reasoning": true,
          "input": ["text", "image"]
        }
      ]
    }
  }
}
```

| 字段 | 说明 |
|-------|-------------|
| `supportsEagerToolInputStreaming` | provider 是否接受按工具设置的 `eager_input_streaming`。默认值:`true`。设为 `false` 会省略该字段,并在启用工具的请求中使用旧版的 fine-grained tool streaming beta 请求头。 |
| `supportsLongCacheRetention` | 当缓存保留策略为 `long` 时,provider 是否接受 Anthropic 长期缓存保留(`cache_control.ttl: "1h"`)。默认值:`true`。 |
| `sendSessionAffinityHeaders` | 启用缓存时,是否根据会话 ID 发送 `x-session-affinity`。默认值:对已知 provider 自动检测。 |
| `supportsCacheControlOnTools` | provider 是否接受在工具定义上使用 Anthropic 风格的 `cache_control` 标记。默认值:`true`。 |
| `forceAdaptiveThinking` | 是否为该模型发送自适应思维(`thinking.type: "adaptive"` 加 `output_config.effort`)。内置的自适应模型会自动设置该项。默认值:`false`。 |
| `allowEmptySignature` | 重放时是否将空的思维签名回放为 `signature: ""`,而不是把思维转换为文本。默认值:`false`。 |

## OpenAI 兼容性

对于部分兼容 OpenAI 的 provider,使用 `compat` 字段。

- provider 级别的 `compat` 会作为该 provider 下所有模型的默认值。
- 模型级别的 `compat` 会覆盖该模型的 provider 级别取值。

```json
{
  "providers": {
    "local-llm": {
      "baseUrl": "http://localhost:8080/v1",
      "api": "openai-completions",
      "compat": {
        "supportsUsageInStreaming": false,
        "maxTokensField": "max_tokens"
      },
      "models": [...]
    }
  }
}
```

| 字段 | 说明 |
|-------|-------------|
| `supportsStore` | provider 是否支持 `store` 字段 |
| `supportsDeveloperRole` | 使用 `developer` 还是 `system` 角色 |
| `supportsReasoningEffort` | 是否支持 `reasoning_effort` 参数 |
| `supportsUsageInStreaming` | 是否支持 `stream_options: { include_usage: true }`(默认值:`true`) |
| `maxTokensField` | 使用 `max_completion_tokens` 还是 `max_tokens` |
| `requiresToolResultName` | 在工具结果消息上包含 `name` 字段 |
| `requiresAssistantAfterToolResult` | 在工具结果之后的用户消息之前插入一条 assistant 消息 |
| `requiresThinkingAsText` | 将思维块转换为纯文本 |
| `requiresReasoningContentOnAssistantMessages` | 启用推理时,在所有重放的 assistant 消息上包含空的 `reasoning_content` |
| `thinkingFormat` | 使用 `reasoning_effort`、`openrouter`、`deepseek`、`together`、`zai`、`qwen` 或 `qwen-chat-template` 思维参数格式 |
| `cacheControlFormat` | 在系统提示词、最后一个工具定义以及最后一条用户/assistant 文本内容上使用 Anthropic 风格的 `cache_control` 标记。目前仅支持 `anthropic`。 |
| `supportsStrictMode` | 在工具定义中包含 `strict` 字段 |
| `supportsLongCacheRetention` | 当缓存保留策略为 `long` 时,provider 是否接受长期缓存保留:对 OpenAI prompt caching 为 `prompt_cache_retention: "24h"`,当 `cacheControlFormat` 为 `anthropic` 时为 `cache_control.ttl: "1h"`。默认值:`true`。 |
| `openRouterRouting` | OpenRouter provider 路由偏好设置。该对象会原样发送在 [OpenRouter API 请求](https://openrouter.ai/docs/guides/routing/provider-selection)的 `provider` 字段中。 |
| `vercelGatewayRouting` | Vercel AI Gateway 的 provider 选择路由配置(`only`、`order`) |

`openrouter` 使用 `reasoning: { effort }`。`together` 使用 `reasoning: { enabled }`,当启用 `supportsReasoningEffort` 时还会发送 `reasoning_effort`。`qwen` 使用顶层的 `enable_thinking`。对于需要 `chat_template_kwargs.enable_thinking` 的本地 Qwen 兼容服务端,请使用 `qwen-chat-template`。

`cacheControlFormat: "anthropic"` 适用于通过 `cache_control` 标记在文本内容和工具定义上暴露 Anthropic 风格 prompt caching 的兼容 OpenAI 的 provider。

示例:

```json
{
  "providers": {
    "openrouter": {
      "baseUrl": "https://openrouter.ai/api/v1",
      "apiKey": "$OPENROUTER_API_KEY",
      "api": "openai-completions",
      "models": [
        {
          "id": "openrouter/anthropic/claude-3.5-sonnet",
          "name": "OpenRouter Claude 3.5 Sonnet",
          "compat": {
            "openRouterRouting": {
              "allow_fallbacks": true,
              "require_parameters": false,
              "data_collection": "deny",
              "zdr": true,
              "enforce_distillable_text": false,
              "order": ["anthropic", "amazon-bedrock", "google-vertex"],
              "only": ["anthropic", "amazon-bedrock"],
              "ignore": ["gmicloud", "friendli"],
              "quantizations": ["fp16", "bf16"],
              "sort": {
                "by": "price",
                "partition": "model"
              },
              "max_price": {
                "prompt": 10,
                "completion": 20
              },
              "preferred_min_throughput": {
                "p50": 100,
                "p90": 50
              },
              "preferred_max_latency": {
                "p50": 1,
                "p90": 3,
                "p99": 5
              }
            }
          }
        }
      ]
    }
  }
}
```

Vercel AI Gateway 示例:

```json
{
  "providers": {
    "vercel-ai-gateway": {
      "baseUrl": "https://ai-gateway.vercel.sh/v1",
      "apiKey": "$AI_GATEWAY_API_KEY",
      "api": "openai-completions",
      "models": [
        {
          "id": "moonshotai/kimi-k2.5",
          "name": "Kimi K2.5 (Fireworks via Vercel)",
          "reasoning": true,
          "input": ["text", "image"],
          "cost": { "input": 0.6, "output": 3, "cacheRead": 0, "cacheWrite": 0 },
          "contextWindow": 262144,
          "maxTokens": 262144,
          "compat": {
            "vercelGatewayRouting": {
              "only": ["fireworks", "novita"],
              "order": ["fireworks", "novita"]
            }
          }
        }
      ]
    }
  }
}
```
