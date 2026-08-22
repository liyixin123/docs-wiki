# AGENTS.md

## 分支与合并流程

- **`main` 是保护分支**：GitHub 上禁用直接 push，只能通过 PR 合并；合并方式用 squash merge，保证 `main` 上每个 PR 只有一个 commit，历史干净。
- **日常小改动**（typo、样式微调、注释、依赖升级等）：在 `chore/` 分支上累积提交，可随时自由 push，攒够后开 PR 压缩合并。不强制建 issue，PR 描述说清即可。
- **功能/用户可见 bug**：先建 issue，再开 `feat/<slug>` / `fix/<slug>` 分支，PR 描述链接 issue（如 `Closes #N`），合并后 issue 自动关闭。
- 特性分支（`feat/` / `fix/` / `chore/` / `prototype/`）可任意 push，不限制。
- PR 标题用 conventional commits 风格（`feat:` / `fix:` / `docs:` / `refactor:` / `test:` / `chore:`），正文说明动机与改动要点。
- 合并前必须通过：`npx tsc --noEmit`、`npx vite build`、`pnpm test`。
- 例外：`prototype/` 归档分支（throwaway，只存原型，永不合并）可直接 push，不进 `main`。

## 测试

- 测试框架:Vitest + jsdom(`pnpm test`)。
- 唯一测试缝隙:前端 API 模块(`src/api.ts`)边界 + DOM 断言;只测用户可见行为(点击/键盘 → DOM 状态),不测内部函数。
- 新的交互模块应保持「纯 DOM、不 import api」,以便独立测试(参考 `src/app-menu.ts` + `src/app-menu.test.ts`)。

## 领域术语

- 左侧文档列表:统一称「目录」(代码中历史命名 nav/sidebar 保留,不强制重命名)。
- 「来源」= 一个导入的本地文件夹或远程仓库;「待更新」「待翻译」状态以 `src/api.ts` 中的 `DocMeta` 为准。
