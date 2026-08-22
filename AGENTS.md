# AGENTS.md

## 分支与合并流程

采用分级 git 流程，完整规则见 skill `git-tiered-flow`。要点：`main` 受保护、禁直接 push，只能 squash-PR 合并；小改动走 `chore/` 分支攒批，不建 issue；功能/用户可见 bug 先建 issue 再 `feat/`/`fix/` 分支；合并前过 `npx tsc --noEmit`、`npx vite build`、`pnpm test`。

## 测试

- 测试框架:Vitest + jsdom(`pnpm test`)。
- 唯一测试缝隙:前端 API 模块(`src/api.ts`)边界 + DOM 断言;只测用户可见行为(点击/键盘 → DOM 状态),不测内部函数。
- 新的交互模块应保持「纯 DOM、不 import api」,以便独立测试(参考 `src/app-menu.ts` + `src/app-menu.test.ts`)。

## 领域术语

- 左侧文档列表:统一称「目录」(代码中历史命名 nav/sidebar 保留,不强制重命名)。
- 「来源」= 一个导入的本地文件夹或远程仓库;「待更新」「待翻译」状态以 `src/api.ts` 中的 `DocMeta` 为准。
