> pi 可以创建提示词模板。让它为你的工作流构建一个。

# 提示词模板（Prompt Templates）

提示词模板是可展开为完整提示词的 Markdown 片段。在编辑器中输入 `/name` 即可调用某个模板，其中 `name` 是不含 `.md` 后缀的文件名。

## 位置

Pi 从以下位置加载提示词模板：

- 全局：`~/.pi/agent/prompts/*.md`
- 项目：`.pi/prompts/*.md`（仅在项目被信任后生效）
- 包（Packages）：`prompts/` 目录，或 `package.json` 中的 `pi.prompts` 条目
- 设置：`prompts` 数组，可包含文件或目录
- CLI：`--prompt-template <path>`（可重复指定）

使用 `--no-prompt-templates` 可禁用自动发现。

## 格式

```markdown
---
description: Review staged git changes
---
Review the staged changes (`git diff --cached`). Focus on:
- Bugs and logic errors
- Security issues
- Error handling gaps
```

- 文件名会成为命令名称。`review.md` 会变为 `/review`。
- `description` 为可选项。若缺省，则使用第一个非空行作为描述。
- `argument-hint` 为可选项。设置后，该提示会在自动补全下拉列表中显示在描述之前。

### 参数提示（Argument Hints）

在前置元数据中使用 `argument-hint`，可以在自动补全中展示预期参数。必填参数使用 `<尖括号>`，可选参数使用 `[方括号]`：

```markdown
---
description: Review PRs from URLs with structured issue and code analysis
argument-hint: "<PR-URL>"
---
```

这会在自动补全下拉列表中呈现为：

```
→ pr   <PR-URL>       — Review PRs from URLs with structured issue and code analysis
  is   <issue>        — Analyze GitHub issues (bugs or feature requests)
  wr   [instructions] — Finish the current task end-to-end
  cl   — Audit changelog entries before release
```

## 用法

在编辑器中输入 `/` 后跟模板名称。自动补全会显示可用模板及其描述。

```
/review                           # Expands review.md
/component Button                 # Expands with argument
/component Button "click handler" # Multiple arguments
```

## 参数

模板支持位置参数、默认值以及简单的切片操作：

- `$1`、`$2`、……表示位置参数
- `$@` 或 `$ARGUMENTS` 表示所有参数拼接在一起
- `${1:-default}` 表示当参数 1 存在/非空时使用该参数，否则使用 `default`
- `${@:N}` 表示从第 N 个位置开始的所有参数（从 1 开始计数）
- `${@:N:L}` 表示从第 N 个位置开始的 `L` 个参数

示例：

```markdown
---
description: Create a component
---
Create a React component named $1 with features: $@
```

默认值对于可选参数非常有用：

```markdown
Summarize the current state in ${1:-7} bullet points.
```

用法：`/component Button "onClick handler" "disabled support"`

## 加载规则

- `prompts/` 中的模板发现是非递归的。
- 如果需要子目录中的模板，请通过 `prompts` 设置或包清单显式添加它们。
