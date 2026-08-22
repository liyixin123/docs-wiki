# AGENTS.md

## 分支与合并流程

- **所有代码改动一律走 PR**:在特性分支上提交(分支名 `feat/<slug>` / `fix/<slug>` / `prototype/<slug>`),push 后开 PR,合并到 `main`,不允许直接 push `main`。
- PR 标题用 conventional commits 风格(`feat:` / `fix:` / `docs:` / `refactor:` / `test:` / `chore:`),正文说明动机与改动要点。
- PR 描述中链接对应 issue(如 `Closes #N`);合并后 issue 自动关闭。
- 合并前必须通过:`npx tsc --noEmit`、`npx vite build`、`pnpm test`。
- 纯文档/约定类改动(如本文件)也走 PR。
- 例外:`prototype/` 归档分支(throwaway,只存原型,永不合并)可直接 push,不进 `main`。

## 测试

- 测试框架:Vitest + jsdom(`pnpm test`)。
- 唯一测试缝隙:前端 API 模块(`src/api.ts`)边界 + DOM 断言;只测用户可见行为(点击/键盘 → DOM 状态),不测内部函数。
- 新的交互模块应保持「纯 DOM、不 import api」,以便独立测试(参考 `src/app-menu.ts` + `src/app-menu.test.ts`)。

## 领域术语

- 左侧文档列表:统一称「目录」(代码中历史命名 nav/sidebar 保留,不强制重命名)。
- 「来源」= 一个导入的本地文件夹或远程仓库;「待更新」「待翻译」状态以 `src/api.ts` 中的 `DocMeta` 为准。
