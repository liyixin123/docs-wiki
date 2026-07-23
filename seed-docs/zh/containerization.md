# 容器化

Pi 默认拥有所有权限运行，但在某些情况下，你可能希望更好地控制 Pi 可以写入哪些目录，以及它拥有哪些访问权限。

有两种通用方案。你可以：
1. 在隔离环境中运行整个 `pi` 进程，或
2. 在主机上运行 `pi`，并将工具执行路由到隔离环境中。

## 选择一种模式

| Pattern | What is isolated | Best for | Notes |
| --- | --- | --- | --- |
| OpenShell | 整个 `pi` 进程处于策略控制的沙箱中 | 本地或远程托管的沙箱 | 需要 OpenShell 网关 |
| Gondolin extension | 内置工具和 `!` 命令 | 在保留主机认证的同时进行本地微虚拟机隔离 | 参见 [`examples/extensions/gondolin/`](../examples/extensions/gondolin/)。 |
| Plain Docker | 整个 `pi` 进程处于本地容器中 | 简单的本地隔离 | 提供商 API 密钥会进入容器。 |

扩展运行在 `pi` 进程所在的位置。如果你在主机上运行带有工具路由扩展的 pi，其他自定义扩展工具仍会在主机上运行，除非它们也委托了自己的操作。

## OpenShell

当你需要一个具备文件系统、进程、网络、凭据和推理控制能力的策略控制沙箱时，使用 [NVIDIA OpenShell](https://docs.nvidia.com/openshell/about/overview)。
OpenShell 可以通过由 Docker、Podman 或虚拟机运行时支持的本地网关，或通过远程 Kubernetes 网关来运行沙箱。

每个沙箱都需要一个活动的网关。
在创建沙箱之前先注册并选择一个网关：

```bash
openshell gateway add <gateway-url> --name <name>
openshell gateway select <name>
```

在 OpenShell 沙箱内启动 `pi`：

```bash
openshell sandbox create --name pi-sandbox --from pi -- pi
```

在这种模式下，整个 `pi` 进程都在沙箱内运行。
内置工具、`!` 命令和扩展工具都在 OpenShell 边界内执行。

如果网关是远程的，项目文件不会从主机绑定挂载，这意味着沙箱中的写入不会反映到你的机器上。
在沙箱内克隆仓库，或使用 OpenShell 文件传输命令：

```bash
openshell sandbox upload pi-sandbox ./repo /workspace
openshell sandbox download pi-sandbox /workspace/repo ./repo-out
```

OpenShell 提供商可以将原始模型 API 密钥保留在沙箱之外。
配置推理路由后，沙箱内的代码可以调用 `https://inference.local`，网关会将配置好的提供商凭据注入上游。
如果你希望模型流量使用此路由，请将 Pi 配置为使用相应的 OpenAI 兼容或 Anthropic 兼容端点。

## Gondolin

[Gondolin](https://github.com/earendil-works/gondolin) 是一个本地 Linux 微虚拟机。
当你希望 `pi` 在主机上运行，但所有内置工具都路由到虚拟机中时，使用[示例扩展](../examples/extensions/gondolin)。

设置：

```bash
cp -R packages/coding-agent/examples/extensions/gondolin ~/.pi/agent/extensions/gondolin
cd ~/.pi/agent/extensions/gondolin
npm install --ignore-scripts
```

在你想要挂载的项目中运行：

```bash
cd /path/to/project
pi -e ~/.pi/agent/extensions/gondolin
```

该扩展会将主机的当前工作目录挂载到虚拟机中的 `/workspace`，并覆盖 `read`、`write`、`edit`、`bash`、`grep`、`find` 和 `ls`。
用户的 `!` 命令也会被路由到虚拟机中。
`/workspace` 下的文件更改会写回主机。

要求：`@earendil-works/gondolin` 需要 Node.js >= 23.6.0，以及 QEMU（需要通过你的包管理器安装）。

## Plain Docker（纯 Docker）

当你希望使用最简单的本地容器边界时，在 Docker 中运行整个 `pi` 进程。

`Dockerfile.pi`：

```dockerfile
FROM node:24-bookworm-slim

RUN apt-get update \
  && apt-get install -y --no-install-recommends bash ca-certificates git ripgrep \
  && rm -rf /var/lib/apt/lists/*
RUN npm install -g --ignore-scripts @earendil-works/pi-coding-agent

WORKDIR /workspace
ENTRYPOINT ["pi"]
```

构建并运行：

```bash
docker build -t pi-sandbox -f Dockerfile.pi .

docker run --rm -it \
  -e ANTHROPIC_API_KEY \
  -v "$PWD:/workspace" \
  -v pi-agent-home:/root/.pi/agent \
  pi-sandbox
```

`-v "$PWD:/workspace"` 会将你的当前目录挂载到容器中的 /workspace，这样在 Docker 内 `/workspace` 中的读写操作会直接影响你的主机文件，与 Gondolin 示例中的情况类似。

如果你希望使用容器本地的设置和会话，请为 `/root/.pi/agent` 使用命名卷。挂载主机的 `~/.pi/agent` 会将主机的认证和会话文件暴露给容器。
