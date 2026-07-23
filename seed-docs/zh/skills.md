> pi 可以创建技能（skills）。让它为你的使用场景构建一个。

# 技能（Skills）

技能是代理按需加载的自包含能力包。一个技能提供针对特定任务的专门工作流、设置说明、辅助脚本和参考文档。

Pi 实现了 [Agent Skills 标准](https://agentskills.io/specification)，对大多数违规情况发出警告，但整体处理较为宽松。Pi 允许技能名称与其父目录不同，尽管该标准不允许这样做；这条规则对于跨多个 agent 运行环境共享的技能目录来说并不理想。

## 目录

- [位置](#locations)
- [技能的工作原理](#how-skills-work)
- [技能命令](#skill-commands)
- [技能结构](#skill-structure)
- [前置元数据（Frontmatter）](#frontmatter)
- [验证](#validation)
- [示例](#example)
- [技能仓库](#skill-repositories)

## 位置

> **安全提示：** 技能可以指示模型执行任何操作，并且可能包含模型会调用的可执行代码。使用前请先审查技能内容。

Pi 从以下位置加载技能：

- 全局：
  - `~/.pi/agent/skills/`
  - `~/.agents/skills/`
- 项目：
  - `.pi/skills/`
  - `.agents/skills/`（在 `cwd` 及其祖先目录中查找，直到 git 仓库根目录，若不在仓库中则直到文件系统根目录）
- 包（Packages）：`skills/` 目录，或 `package.json` 中的 `pi.skills` 条目
- 设置：`skills` 数组，可包含文件或目录
- CLI：`--skill <path>`（可重复指定，即使使用 `--no-skills` 也会额外生效）

发现规则：
- 在 `~/.pi/agent/skills/` 和 `.pi/skills/` 中，根目录下的 `.md` 文件会被作为独立技能发现
- 在所有技能位置中，包含 `SKILL.md` 的目录会被递归发现
- 在 `~/.agents/skills/` 和项目的 `.agents/skills/` 中，根目录下的 `.md` 文件会被忽略

使用 `--no-skills` 可禁用自动发现（显式指定的 `--skill` 路径仍会加载）。

### 使用来自其他运行环境的技能

要使用来自 Claude Code 或 OpenAI Codex 的技能，请将其目录添加到设置中：

```json
{
  "skills": [
    "~/.claude/skills",
    "~/.codex/skills"
  ]
}
```

对于项目级别的 Claude Code 技能，添加到 `.pi/settings.json`：

```json
{
  "skills": ["../.claude/skills"]
}
```

## 技能的工作原理

1. 启动时，pi 会扫描技能位置并提取名称和描述
2. 系统提示词会按照[规范](https://agentskills.io/integrate-skills)以 XML 格式包含可用技能
3. 当任务匹配时，代理会使用 `read` 加载完整的 SKILL.md（模型并非总会这样做；可以通过提示词或 `/skill:name` 强制加载）
4. 代理会遵循相应说明，并使用相对路径引用脚本和资源

这是一种渐进式披露（progressive disclosure）：只有描述始终位于上下文中，完整说明按需加载。

## 技能命令

技能会注册为 `/skill:name` 命令：

```bash
/skill:brave-search           # Load and execute the skill
/skill:pdf-tools extract      # Load skill with arguments
```

命令后面的参数会以 `User: <args>` 的形式追加到技能内容中。

可以在交互模式下通过 `/settings` 或在 `settings.json` 中切换技能命令：

```json
{
  "enableSkillCommands": true
}
```

## 技能结构

一个技能是包含 `SKILL.md` 文件的目录。其余内容形式自由。

```
my-skill/
├── SKILL.md              # Required: frontmatter + instructions
├── scripts/              # Helper scripts
│   └── process.sh
├── references/           # Detailed docs loaded on-demand
│   └── api-reference.md
└── assets/
    └── template.json
```

### SKILL.md 格式

````markdown
---
name: my-skill
description: What this skill does and when to use it. Be specific.
---

# My Skill

## Setup

Run once before first use:
```bash
cd /path/to/skill && npm install
```

## Usage

```bash
./scripts/process.sh <input>
```
````

使用相对于技能目录的相对路径：

```markdown
See [the reference guide](references/REFERENCE.md) for details.
```

## 前置元数据（Frontmatter）

根据 [Agent Skills 规范](https://agentskills.io/specification#frontmatter-required)：

| 字段 | 是否必需 | 描述 |
|-------|----------|------|
| `name` | 是 | 最多 64 个字符。小写字母 a-z、数字 0-9、连字符。与该标准不同的是，Pi 不要求此字段与父目录名称一致，因为该标准要求对于共享的技能目录而言并不理想。 |
| `description` | 是 | 最多 1024 个字符。说明该技能的作用及使用场景。 |
| `license` | 否 | 许可证名称或对捆绑许可证文件的引用。 |
| `compatibility` | 否 | 最多 500 个字符。环境要求。 |
| `metadata` | 否 | 任意键值映射。 |
| `allowed-tools` | 否 | 以空格分隔的预先批准工具列表（实验性功能）。 |
| `disable-model-invocation` | 否 | 当为 `true` 时，该技能在系统提示词中隐藏，用户必须使用 `/skill:name` 调用。 |

### 命名规则

- 1-64 个字符
- 仅限小写字母、数字、连字符
- 不能以连字符开头或结尾
- 不能有连续的连字符

Pi 不要求名称与父目录一致。Agent Skills 标准要求如此，但该要求对于被多个工具共享使用的技能目录来说并不理想。

有效示例：`pdf-processing`、`data-analysis`、`code-review`
无效示例：`PDF-Processing`、`-pdf`、`pdf--processing`

### 描述编写最佳实践

描述内容决定了代理何时加载该技能。请务必具体明确。

良好示例：
```yaml
description: Extracts text and tables from PDF files, fills PDF forms, and merges multiple PDFs. Use when working with PDF documents.
```

不佳示例：
```yaml
description: Helps with PDFs.
```

## 验证

Pi 会根据 Agent Skills 标准对技能进行验证。大多数问题只会产生警告，但仍会加载该技能：

- 名称超过 64 个字符或包含无效字符
- 名称以连字符开头/结尾，或包含连续连字符
- 描述超过 1024 个字符

未知的前置元数据字段会被忽略。

**例外情况：** 缺少 description 的技能不会被加载。

名称冲突（不同位置存在相同名称）会发出警告，并保留最先找到的技能。

## 示例

```
brave-search/
├── SKILL.md
├── search.js
└── content.js
```

**SKILL.md：**
````markdown
---
name: brave-search
description: Web search and content extraction via Brave Search API. Use for searching documentation, facts, or any web content.
---

# Brave Search

## Setup

```bash
cd /path/to/brave-search && npm install
```

## Search

```bash
./search.js "query"              # Basic search
./search.js "query" --content    # Include page content
```

## Extract Page Content

```bash
./content.js https://example.com
```
````

## 技能仓库

- [Anthropic Skills](https://github.com/anthropics/skills) - 文档处理（docx、pdf、pptx、xlsx）、Web 开发
- [Pi Skills](https://github.com/badlogic/pi-skills) - 网络搜索、浏览器自动化、Google API、转录
