# 开发

更多指南请参见 [AGENTS.md](https://github.com/earendil-works/pi-mono/blob/main/AGENTS.md)。

## 环境设置

```bash
git clone https://github.com/earendil-works/pi-mono
cd pi-mono
npm install
npm run build
```

从源码运行：

```bash
/path/to/pi-mono/pi-test.sh
```

该脚本可以在任意目录下运行。Pi 会保留调用者的当前工作目录。

## 分叉 / 重新品牌化（Forking / Rebranding）

通过 `package.json` 进行配置：

```json
{
  "piConfig": {
    "name": "pi",
    "configDir": ".pi"
  }
}
```

为你的分叉项目修改 `name`、`configDir` 和 `bin` 字段。这会影响 CLI 横幅、配置路径以及环境变量名称。

## 路径解析

有三种执行模式：npm 安装、独立二进制文件、从源码通过 tsx 运行。

**对于包资源，始终使用 `src/config.ts`**：

```typescript
import { getPackageDir, getThemeDir } from "./config.js";
```

不要直接使用 `__dirname` 来获取包资源。

## 调试命令

`/debug`（隐藏命令）会写入 `~/.pi/agent/pi-debug.log`：
- 带 ANSI 代码的已渲染 TUI 行
- 发送给 LLM 的最后几条消息

## 测试

```bash
./test.sh                         # 运行非 LLM 测试（无需 API 密钥）
npm test                          # 运行所有测试
npm test -- test/specific.test.ts # 运行指定测试
```

## 项目结构

```
packages/
  ai/           # LLM provider abstraction
  agent/        # Agent loop and message types  
  tui/          # Terminal UI components
  coding-agent/ # CLI and interactive mode
```
