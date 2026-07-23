# llama.cpp

Pi 支持 [llama.cpp](https://github.com/ggml-org/llama.cpp) 路由服务器。该路由器会发现多个 GGUF 模型，并按需加载或卸载它们。

请使用支持路由功能的最新 llama.cpp 构建版本。按照[构建说明](https://github.com/ggml-org/llama.cpp/blob/master/docs/build.md)进行构建，或为你的平台安装[预构建版本](https://github.com/ggml-org/llama.cpp/releases)。

## 启动路由器

启动 `llama-server` 时不要带 `--model` 或 `-m` 参数。传入模型会启动单模型模式，而不是路由模式。

```bash
llama-server \
  --models-dir ~/models \
  --no-models-autoload \
  --jinja \
  --host 127.0.0.1 \
  --port 8080 \
  -ngl 999 \
  -c 32768
```

重要选项：

- `--models-dir ~/models` 发现本地 GGUF 文件。
- `--no-models-autoload` 使加载操作必须通过 `/llama` 显式触发。
- `--jinja` 启用兼容的聊天模板和工具调用。
- `-ngl 999` 尽可能多地将层卸载到 GPU 上。
- `-c 32768` 设置每个已加载模型的上下文窗口。省略此项将使用模型的原生上下文长度，这可能需要占用大量内存。

单文件模型可以直接放在模型目录中。多模态和多分片模型请放在单独的子目录中：

```text
~/models/
├── llama-3.2-1b-Q4_K_M.gguf
├── gemma-3-4b-it-Q4_K_M/
│   ├── gemma-3-4b-it-Q4_K_M.gguf
│   └── mmproj-F16.gguf
└── large-model-Q4_K_M/
    ├── large-model-Q4_K_M-00001-of-00003.gguf
    ├── large-model-Q4_K_M-00002-of-00003.gguf
    └── large-model-Q4_K_M-00003-of-00003.gguf
```

手动添加文件后，请重启路由器。有关按模型设置上下文大小及其他选项，请使用 [llama.cpp 模型预设](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md#model-presets)。

## 配置 Pi

启动 Pi 并配置提供方：

```text
/login llama.cpp
```

输入路由器 URL 和可选的 API 密钥。默认 URL 为 `http://127.0.0.1:8080`。

也可以通过环境变量配置相同的值，而无需使用 `/login`：

```bash
export LLAMA_BASE_URL=http://127.0.0.1:8080
export LLAMA_API_KEY=optional-secret
pi
```

如果服务器使用了 API 密钥，请在启动 `llama-server` 时带上匹配的 `--api-key` 值。为了仅限本地访问，请保留 `--host 127.0.0.1`。

## 管理模型

运行：

```text
/llama
```

- 选择一个未加载的模型即可加载它。
- 选择一个已加载的模型即可卸载它。
- 选择**下载模型…**，搜索 Hugging Face，然后选择一个仓库和量化版本。也可以直接输入准确的 `owner/repository[:quant]` 值。
- 在加载或下载过程中按 Escape 可确认取消操作。

Hugging Face 搜索会在设置了 `HF_TOKEN` 时使用它，否则会依次检查 `$HF_TOKEN_PATH`、`$HF_HOME/token`、`$XDG_CACHE_HOME/huggingface/token` 和 `~/.cache/huggingface/token`。搜索也可以在未经身份验证的情况下工作，但会受到较低的速率限制。在下载受限（gated）仓库之前，Pi 会发出警告，并附上其访问页面的链接。下载操作由 llama.cpp 服务器执行，因此当所选仓库需要访问权限时，该进程也必须设置 `HF_TOKEN`。

如果已有其他模型处于加载状态，Pi 会询问是先卸载它们，还是保持它们的加载状态。Pi 不会静默卸载模型，也绝不会删除模型文件。路由器可能与其他客户端共享，因此 `/llama` 始终显示路由器的当前状态。

只有已加载的模型才会出现在 `/model` 中。加载模型后，运行 `/model` 以在当前 Pi 会话中选择它。

如果路由器断开连接，`/llama` 会显示**重试**和**关闭**选项。重试会重新连接并刷新模型状态，但不会重放被中断的操作。

## 故障排查

检查路由器是否可访问：

```bash
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/models
```

- **`/llama` 中没有模型：** 检查 `--models-dir`、目录结构，并重启路由器。
- **`/model` 中缺少模型：** 先使用 `/llama` 加载它。
- **加载失败或占用过多内存：** 降低 `-c` 值，或卸载其他模型。
- **服务器未处于路由模式：** 启动时不要带 `--model`、`-m` 或 `-hf` 参数。
