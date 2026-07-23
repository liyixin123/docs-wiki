> pi 可以帮你创建 pi 包。让它帮你打包扩展、技能、提示词模板或主题。

# Pi 包（Pi Packages）

Pi 包将扩展、技能、提示词模板和主题打包在一起，以便通过 npm 或 git 进行分享。一个包可以在 `package.json` 中通过 `pi` 键声明资源，也可以使用约定的目录结构。

## 目录

- [安装与管理](#install-and-manage)
- [包来源](#package-sources)
- [创建 Pi 包](#creating-a-pi-package)
- [包结构](#package-structure)
- [依赖项](#dependencies)
- [包过滤](#package-filtering)
- [启用与禁用资源](#enable-and-disable-resources)
- [作用域与去重](#scope-and-deduplication)

## 安装与管理

> **安全提示：** Pi 包以完整的系统权限运行。扩展会执行任意代码，技能可以指示模型执行任何操作，包括运行可执行文件。安装第三方包之前请先审查源代码。

```bash
pi install npm:@foo/bar@1.0.0
pi install git:github.com/user/repo@v1
pi install https://github.com/user/repo  # raw URLs work too
pi install /absolute/path/to/package
pi install ./relative/path/to/package

pi remove npm:@foo/bar
pi list                     # show installed packages from settings
pi update                   # update pi, update packages, and reconcile pinned git refs
pi update --extensions      # update packages and reconcile pinned git refs only
pi update --self            # update pi only
pi update --self --force    # reinstall pi even if current
pi update npm:@foo/bar      # update one package
pi update --extension npm:@foo/bar
```

这些命令用于管理 pi 包，而非 pi CLI 本身的安装。若要卸载 pi 本身，请参见[快速入门](quickstart.md#uninstall)。

默认情况下，`install` 和 `remove` 会写入用户设置（`~/.pi/agent/settings.json`）。使用 `-l` 可改为写入项目设置（`.pi/settings.json`）。项目设置可以与团队共享，pi 会在启动时自动安装任何缺失的包。

若要在不安装的情况下试用某个包，可使用 `--extension` 或 `-e`。这会将其安装到临时目录，仅在当前运行期间有效：

```bash
pi -e npm:@foo/bar
pi -e git:github.com/user/repo
```

## 包来源

Pi 在设置和 `pi install` 中接受三种来源类型。

### npm

```
npm:@scope/pkg@1.2.3
npm:pkg
```

- 带版本号的规格会被固定（pinned），并在包更新（`pi update`、`pi update --extensions`）时跳过。
- 用户安装位于 `~/.pi/agent/npm/` 下。
- 项目安装位于 `.pi/npm/` 下。
- 在 `settings.json` 中设置 `npmCommand`，可将 npm 包查找和安装操作固定到特定的包装命令，例如 `mise` 或 `asdf`。

示例：

```json
{
  "npmCommand": ["mise", "exec", "node@20", "--", "npm"]
}
```

### git

```
git:github.com/user/repo@v1
git:git@github.com:user/repo@v1
https://github.com/user/repo@v1
ssh://git@github.com/user/repo@v1
```

- 不带 `git:` 前缀时，只接受带协议的 URL（`https://`、`http://`、`ssh://`、`git://`）。
- 带 `git:` 前缀时，可接受简写格式，包括 `github.com/user/repo` 和 `git@github.com:user/repo`。
- HTTPS 和 SSH URL 均受支持。
- SSH URL 会自动使用你配置的 SSH 密钥（遵循 `~/.ssh/config`）。
- 对于非交互式运行（例如 CI），可以设置 `GIT_TERMINAL_PROMPT=0` 来禁用凭据提示，并设置 `GIT_SSH_COMMAND`（例如 `ssh -o BatchMode=yes -o ConnectTimeout=5`）以实现快速失败。
- 引用（refs）会被固定为标签或提交（tags or commits）。`pi update` 和 `pi update --extensions` 不会将其移动到更新的引用，但会将现有克隆与配置的引用进行协调（reconcile）。
- 使用 `pi install git:host/user/repo@new-ref` 可更新设置，并将现有包移动到新的固定引用。
- 克隆到 `~/.pi/agent/git/<host>/<path>`（全局）或 `.pi/git/<host>/<path>`（项目）。
- 当协调操作改变了检出内容时，pi 会重置并清理该克隆，然后在存在 `package.json` 时运行 `npm install`。

**SSH 示例：**
```bash
# git@host:path shorthand (requires git: prefix)
pi install git:git@github.com:user/repo

# ssh:// protocol format
pi install ssh://git@github.com/user/repo

# With version ref
pi install git:git@github.com:user/repo@v1.0.0
```

### 本地路径

```
/absolute/path/to/package
./relative/path/to/package
```

本地路径指向磁盘上的文件或目录，会被添加到设置中而不会被复制。相对路径会相对于其所在的设置文件进行解析。若路径指向文件，则作为单个扩展加载。若路径指向目录，pi 会按照包规则加载其中的资源。

## 创建 Pi 包

在 `package.json` 中添加 `pi` 清单，或使用约定的目录结构。为便于被发现，请添加 `pi-package` 关键字。

```json
{
  "name": "my-package",
  "keywords": ["pi-package"],
  "pi": {
    "extensions": ["./extensions"],
    "skills": ["./skills"],
    "prompts": ["./prompts"],
    "themes": ["./themes"]
  }
}
```

路径均相对于包根目录。数组支持 glob 模式和 `!排除项`。

### 图库元数据（Gallery Metadata）

[包图库](https://pi.dev/packages) 会展示带有 `pi-package` 标签的包。添加 `video` 或 `image` 字段以显示预览：

```json
{
  "name": "my-package",
  "keywords": ["pi-package"],
  "pi": {
    "extensions": ["./extensions"],
    "video": "https://example.com/demo.mp4",
    "image": "https://example.com/screenshot.png"
  }
}
```

- **video：** 仅支持 MP4。在桌面端，鼠标悬停时自动播放。点击后打开全屏播放器。
- **image：** 支持 PNG、JPEG、GIF 或 WebP。以静态预览方式展示。

若两者都设置，video 优先。

## 包结构

### 约定目录

若不存在 `pi` 清单，pi 会从以下目录自动发现资源：

- `extensions/` 加载 `.ts` 和 `.js` 文件
- `skills/` 递归查找 `SKILL.md` 所在文件夹，并将顶层 `.md` 文件作为技能加载
- `prompts/` 加载 `.md` 文件
- `themes/` 加载 `.json` 文件

## 依赖项

第三方运行时依赖应放在 `package.json` 的 `dependencies` 中。不注册扩展、技能、提示词模板或主题的依赖项同样属于 `dependencies`。当 pi 从 npm 或 git 安装某个包时，会运行 `npm install`，因此这些依赖项会被自动安装。

Pi 内置捆绑了用于扩展和技能的核心包。如果你导入了以下任何一个包，请将其列在 `peerDependencies` 中，并使用 `"*"` 范围，且不要将其捆绑进你的包：`@earendil-works/pi-ai`、`@earendil-works/pi-agent-core`、`@earendil-works/pi-coding-agent`、`@earendil-works/pi-tui`、`typebox`。

其他 pi 包必须捆绑在你的 tarball 中。请将它们添加到 `dependencies` 和 `bundledDependencies` 中，然后通过 `node_modules/` 路径引用它们的资源。Pi 会以独立的模块根目录加载各个包，因此不同的安装不会发生冲突或共享模块。

示例：

```json
{
  "dependencies": {
    "shitty-extensions": "^1.0.1"
  },
  "bundledDependencies": ["shitty-extensions"],
  "pi": {
    "extensions": ["extensions", "node_modules/shitty-extensions/extensions"],
    "skills": ["skills", "node_modules/shitty-extensions/skills"]
  }
}
```

## 包过滤

使用设置中的对象形式，可以过滤某个包加载的内容：

```json
{
  "packages": [
    "npm:simple-pkg",
    {
      "source": "npm:my-package",
      "extensions": ["extensions/*.ts", "!extensions/legacy.ts"],
      "skills": [],
      "prompts": ["prompts/review.md"],
      "themes": ["+themes/legacy.json"]
    }
  ]
}
```

`+path` 和 `-path` 是相对于包根目录的精确路径。

- 省略某个键表示加载该类型的全部资源。
- 使用 `[]` 表示不加载该类型的任何资源。
- `!pattern` 表示排除匹配项。
- `+path` 强制包含某个精确路径。
- `-path` 强制排除某个精确路径。
- 过滤器会在清单基础上叠加生效，进一步缩小已允许的范围。

## 启用与禁用资源

使用 `pi config` 可以启用或禁用已安装包及本地目录中的扩展、技能、提示词模板和主题。该命令同时适用于全局（`~/.pi/agent`）和项目（`.pi/`）作用域。

## 作用域与去重

包既可以出现在全局设置中，也可以出现在项目设置中。若同一个包同时出现在两处，则以项目条目为准。身份识别方式如下：

- npm：包名称
- git：不含引用（ref）的仓库 URL
- 本地：解析后的绝对路径
