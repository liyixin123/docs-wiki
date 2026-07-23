# Docs Wiki Desktop

一个基于 Tauri 的多来源文档 Wiki 桌面应用，支持本地导入、远程抓取和内置 Pi 文档种子，提供全文搜索与中英文翻译能力。

## 技术栈

- [Tauri 2](https://tauri.app/)（Rust 后端）
- TypeScript + Vite（前端，原生 Web 组件，无框架）
- [marked](https://github.com/markedjs/marked) 用于 Markdown 渲染

## 开发

```bash
pnpm install
pnpm tauri dev
```

## 构建

```bash
pnpm tauri build
```

## 项目结构

```
├── src/            前端源码（TypeScript）
├── src-tauri/      Rust 后端（Tauri 命令、文档源、搜索、翻译等）
├── seed-docs/      内置文档种子（中英文）
└── DEVELOPMENT_PLAN.md  开发计划与架构说明
```

更多设计细节见 [DEVELOPMENT_PLAN.md](DEVELOPMENT_PLAN.md)。
