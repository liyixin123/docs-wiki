> pi 能够创建 TUI 组件。让它为你的使用场景构建一个吧。

# TUI 组件

扩展和自定义工具可以渲染自定义的 TUI 组件,以构建交互式用户界面。本页介绍组件系统以及可用的构建模块。

**来源:** [`@earendil-works/pi-tui`](https://github.com/earendil-works/pi-mono/tree/main/packages/tui)

## 组件接口

所有组件都实现以下接口:

```typescript
interface Component {
  render(width: number): string[];
  handleInput?(data: string): void;
  wantsKeyRelease?: boolean;
  invalidate(): void;
}
```

| 方法 | 说明 |
|--------|-------------|
| `render(width)` | 返回字符串数组(每个元素对应一行)。每一行的宽度**不得超过 `width`**。 |
| `handleInput?(data)` | 当组件获得焦点时接收键盘输入。 |
| `wantsKeyRelease?` | 若为 true,则组件会接收按键释放事件(Kitty 协议)。默认值:false。 |
| `invalidate()` | 清除缓存的渲染状态。在主题变化时调用。 |

TUI 会在每一行渲染内容的末尾追加一次完整的 SGR 重置和 OSC 8 重置。样式不会跨行延续。如果你要输出带样式的多行文本,需要在每一行重新应用样式,或者使用 `wrapTextWithAnsi()`,以便在每个换行后的行中都保留样式。

## Focusable 接口(输入法 IME 支持)

需要显示文本光标并支持输入法(IME,Input Method Editor)的组件应当实现 `Focusable` 接口:

```typescript
import { CURSOR_MARKER, type Component, type Focusable } from "@earendil-works/pi-tui";

class MyInput implements Component, Focusable {
  focused: boolean = false;  // Set by TUI when focus changes
  
  render(width: number): string[] {
    const marker = this.focused ? CURSOR_MARKER : "";
    // Emit marker right before the fake cursor
    return [`> ${beforeCursor}${marker}\x1b[7m${atCursor}\x1b[27m${afterCursor}`];
  }
}
```

当一个 `Focusable` 组件获得焦点时,TUI 会:
1. 将组件的 `focused` 设置为 `true`
2. 在渲染输出中扫描 `CURSOR_MARKER`(一个零宽度的 APC 转义序列)
3. 将硬件终端光标定位到该位置
4. 仅当启用了 `showHardwareCursor` 时才显示硬件光标

默认情况下光标保持隐藏。这样既能保留伪光标的渲染效果,又能为那些依靠隐藏光标来追踪 IME 候选窗口位置的终端正确定位硬件光标。部分终端需要一个可见的硬件光标才能正确定位 IME,可以通过 `showHardwareCursor`、`setShowHardwareCursor(true)` 或 `PI_HARDWARE_CURSOR=1` 来启用。内置的 `Editor` 和 `Input` 组件已经实现了这个接口。

### 内嵌输入框的容器组件

当一个容器组件(对话框、选择器等)内含 `Input` 或 `Editor` 子组件时,该容器必须实现 `Focusable` 接口,并将焦点状态传递给子组件。否则,IME 输入时硬件光标将无法被正确定位。

```typescript
import { Container, type Focusable, Input } from "@earendil-works/pi-tui";

class SearchDialog extends Container implements Focusable {
  private searchInput: Input;

  // Focusable implementation - propagate to child input for IME cursor positioning
  private _focused = false;
  get focused(): boolean {
    return this._focused;
  }
  set focused(value: boolean) {
    this._focused = value;
    this.searchInput.focused = value;
  }

  constructor() {
    super();
    this.searchInput = new Input();
    this.addChild(this.searchInput);
  }
}
```

如果不做这样的传递,使用 IME(中文、日文、韩文等)输入时,候选窗口会出现在屏幕上错误的位置。

## 使用组件

**在扩展中**通过 `ctx.ui.custom()` 使用:

```typescript
pi.on("session_start", async (_event, ctx) => {
  const handle = ctx.ui.custom(myComponent);
  // handle.requestRender() - trigger re-render
  // handle.close() - restore normal UI
});
```

**在自定义工具中**通过 `pi.ui.custom()` 使用:

```typescript
async execute(toolCallId, params, onUpdate, ctx, signal) {
  const handle = pi.ui.custom(myComponent);
  // ...
  handle.close();
}
```

## 浮层(Overlay)

浮层会将组件渲染在已有内容之上,而不会清空屏幕。向 `ctx.ui.custom()` 传入 `{ overlay: true }` 即可启用:

```typescript
const result = await ctx.ui.custom<string | null>(
  (tui, theme, keybindings, done) => new MyDialog({ onClose: done }),
  { overlay: true }
);
```

如需控制位置和大小,请使用 `overlayOptions`:

```typescript
const result = await ctx.ui.custom<string | null>(
  (tui, theme, keybindings, done) => new SidePanel({ onClose: done }),
  {
    overlay: true,
    overlayOptions: {
      // Size: number or percentage string
      width: "50%",          // 50% of terminal width
      minWidth: 40,          // minimum 40 columns
      maxHeight: "80%",      // max 80% of terminal height

      // Position: anchor-based (default: "center")
      anchor: "right-center", // 9 positions: center, top-left, top-center, etc.
      offsetX: -2,            // offset from anchor
      offsetY: 0,

      // Or percentage/absolute positioning
      row: "25%",            // 25% from top
      col: 10,               // column 10

      // Margins
      margin: 2,             // all sides, or { top, right, bottom, left }

      // Responsive: hide on narrow terminals
      visible: (termWidth, termHeight) => termWidth >= 80,
    },
    // Get handle for programmatic focus and visibility control
    onHandle: (handle) => {
      // handle.focus() - focus this overlay and bring it to the visual front
      // handle.unfocus() - release input to normal fallback
      // handle.unfocus({ target }) - release input to a specific component or null
      // handle.setHidden(true/false) - toggle visibility
      // handle.hide() - permanently remove
    },
  }
);
```

### 浮层焦点

一个获得焦点的可见浮层会在临时的非浮层 UI 出现期间,继续保持对输入的所有权。如果某个浮层在没有传入 `{ overlay: true }` 的情况下打开了另一个 `ctx.ui.custom()` 组件,那么在该替代 UI 处于活动状态期间,它会接收输入;当它关闭后,原本获得焦点的浮层可以重新夺回输入。

当一个可见浮层应当停止占有输入,并让 TUI 回退到另一个可见的、捕获输入的浮层或之前的焦点目标时,使用 `handle.unfocus()`。当希望某个特定组件在浮层保持可见的同时接收输入时,使用 `handle.unfocus({ target })`。传入 `{ target: null }` 则表示有意不设置任何获得焦点的组件,直到再次设置焦点为止。

### 浮层生命周期

浮层组件在关闭时会被销毁。不要复用引用——应当创建全新的实例:

```typescript
// Wrong - stale reference
let menu: MenuComponent;
await ctx.ui.custom((_, __, ___, done) => {
  menu = new MenuComponent(done);
  return menu;
}, { overlay: true });
setActiveComponent(menu);  // Disposed

// Correct - re-call to re-show
const showMenu = () => ctx.ui.custom((_, __, ___, done) => 
  new MenuComponent(done), { overlay: true });

await showMenu();  // First show
await showMenu();  // "Back" = just call again
```

关于锚点、边距、层叠、响应式可见性和动画的完整示例,请参见 [overlay-qa-tests.ts](../examples/extensions/overlay-qa-tests.ts)。

## 内置组件

从 `@earendil-works/pi-tui` 导入:

```typescript
import { Text, Box, Container, Spacer, Markdown } from "@earendil-works/pi-tui";
```

### Text

带自动换行的多行文本。

```typescript
const text = new Text(
  "Hello World",    // content
  1,                // paddingX (default: 1)
  1,                // paddingY (default: 1)
  (s) => bgGray(s)  // optional background function
);
text.setText("Updated");
```

### Box

带内边距和背景色的容器。

```typescript
const box = new Box(
  1,                // paddingX
  1,                // paddingY
  (s) => bgGray(s)  // background function
);
box.addChild(new Text("Content", 0, 0));
box.setBgFn((s) => bgBlue(s));
```

### Container

将子组件按垂直方向分组排列。

```typescript
const container = new Container();
container.addChild(component1);
container.addChild(component2);
container.removeChild(component1);
```

### Spacer

空白的垂直间距。

```typescript
const spacer = new Spacer(2);  // 2 empty lines
```

### Markdown

渲染带语法高亮的 Markdown。

```typescript
const md = new Markdown(
  "# Title\n\nSome **bold** text",
  1,        // paddingX
  1,        // paddingY
  theme     // MarkdownTheme (see below)
);
md.setText("Updated markdown");
```

### Image

在支持的终端(Kitty、iTerm2、Ghostty、WezTerm)中渲染图片。

```typescript
const image = new Image(
  base64Data,   // base64-encoded image
  "image/png",  // MIME type
  theme,        // ImageTheme
  { maxWidthCells: 80, maxHeightCells: 24 }
);
```

## 键盘输入

使用 `matchesKey()` 来检测按键:

```typescript
import { matchesKey, Key } from "@earendil-works/pi-tui";

handleInput(data: string) {
  if (matchesKey(data, Key.up)) {
    this.selectedIndex--;
  } else if (matchesKey(data, Key.enter)) {
    this.onSelect?.(this.selectedIndex);
  } else if (matchesKey(data, Key.escape)) {
    this.onCancel?.();
  } else if (matchesKey(data, Key.ctrl("c"))) {
    // Ctrl+C
  }
}
```

**按键标识符**(使用 `Key.*` 可获得自动补全,也可以使用字符串字面量):
- 基础按键:`Key.enter`、`Key.escape`、`Key.tab`、`Key.space`、`Key.backspace`、`Key.delete`、`Key.home`、`Key.end`
- 方向键:`Key.up`、`Key.down`、`Key.left`、`Key.right`
- 带修饰键:`Key.ctrl("c")`、`Key.shift("tab")`、`Key.alt("left")`、`Key.ctrlShift("p")`
- 也可以使用字符串形式:`"enter"`、`"ctrl+c"`、`"shift+tab"`、`"ctrl+shift+p"`

## 行宽

**关键点:** `render()` 返回的每一行都不能超过 `width` 参数。

```typescript
import { visibleWidth, truncateToWidth } from "@earendil-works/pi-tui";

render(width: number): string[] {
  // Truncate long lines
  return [truncateToWidth(this.text, width)];
}
```

工具函数:
- `visibleWidth(str)` - 获取显示宽度(忽略 ANSI 转义码)
- `truncateToWidth(str, width, ellipsis?)` - 按宽度截断,可选省略号
- `wrapTextWithAnsi(str, width)` - 在保留 ANSI 转义码的同时进行自动换行

## 创建自定义组件

示例:交互式选择器

```typescript
import {
  matchesKey, Key,
  truncateToWidth, visibleWidth
} from "@earendil-works/pi-tui";

class MySelector {
  private items: string[];
  private selected = 0;
  private cachedWidth?: number;
  private cachedLines?: string[];
  
  public onSelect?: (item: string) => void;
  public onCancel?: () => void;

  constructor(items: string[]) {
    this.items = items;
  }

  handleInput(data: string): void {
    if (matchesKey(data, Key.up) && this.selected > 0) {
      this.selected--;
      this.invalidate();
    } else if (matchesKey(data, Key.down) && this.selected < this.items.length - 1) {
      this.selected++;
      this.invalidate();
    } else if (matchesKey(data, Key.enter)) {
      this.onSelect?.(this.items[this.selected]);
    } else if (matchesKey(data, Key.escape)) {
      this.onCancel?.();
    }
  }

  render(width: number): string[] {
    if (this.cachedLines && this.cachedWidth === width) {
      return this.cachedLines;
    }

    this.cachedLines = this.items.map((item, i) => {
      const prefix = i === this.selected ? "> " : "  ";
      return truncateToWidth(prefix + item, width);
    });
    this.cachedWidth = width;
    return this.cachedLines;
  }

  invalidate(): void {
    this.cachedWidth = undefined;
    this.cachedLines = undefined;
  }
}
```

在扩展中使用:

```typescript
pi.registerCommand("pick", {
  description: "Pick an item",
  handler: async (args, ctx) => {
    const items = ["Option A", "Option B", "Option C"];
    const selector = new MySelector(items);
    
    let handle: { close: () => void; requestRender: () => void };
    
    await new Promise<void>((resolve) => {
      selector.onSelect = (item) => {
        ctx.ui.notify(`Selected: ${item}`, "info");
        handle.close();
        resolve();
      };
      selector.onCancel = () => {
        handle.close();
        resolve();
      };
      handle = ctx.ui.custom(selector);
    });
  }
});
```

## 主题(Theming)

组件可接受主题对象以控制样式。

**在 `renderCall`/`renderResult` 中**,使用 `theme` 参数:

```typescript
renderResult(result, options, theme, context) {
  // Use theme.fg() for foreground colors
  return new Text(theme.fg("success", "Done!"), 0, 0);
  
  // Use theme.bg() for background colors
  const styled = theme.bg("toolPendingBg", theme.fg("accent", "text"));
}
```

**前景色**(`theme.fg(color, text)`):

| 类别 | 颜色 |
|----------|--------|
| 通用 | `text`、`accent`、`muted`、`dim` |
| 状态 | `success`、`error`、`warning` |
| 边框 | `border`、`borderAccent`、`borderMuted` |
| 消息 | `userMessageText`、`customMessageText`、`customMessageLabel` |
| 工具 | `toolTitle`、`toolOutput` |
| 差异对比(Diff) | `toolDiffAdded`、`toolDiffRemoved`、`toolDiffContext` |
| Markdown | `mdHeading`、`mdLink`、`mdLinkUrl`、`mdCode`、`mdCodeBlock`、`mdCodeBlockBorder`、`mdQuote`、`mdQuoteBorder`、`mdHr`、`mdListBullet` |
| 语法 | `syntaxComment`、`syntaxKeyword`、`syntaxFunction`、`syntaxVariable`、`syntaxString`、`syntaxNumber`、`syntaxType`、`syntaxOperator`、`syntaxPunctuation` |
| 思考 | `thinkingOff`、`thinkingMinimal`、`thinkingLow`、`thinkingMedium`、`thinkingHigh`、`thinkingXhigh` |
| 模式 | `bashMode` |

**背景色**(`theme.bg(color, text)`):

`selectedBg`、`userMessageBg`、`customMessageBg`、`toolPendingBg`、`toolSuccessBg`、`toolErrorBg`

**对于 Markdown**,使用 `getMarkdownTheme()`:

```typescript
import { getMarkdownTheme } from "@earendil-works/pi-coding-agent";
import { Markdown } from "@earendil-works/pi-tui";

renderResult(result, options, theme, context) {
  const mdTheme = getMarkdownTheme();
  return new Markdown(result.details.markdown, 0, 0, mdTheme);
}
```

**对于自定义组件**,请定义你自己的主题接口:

```typescript
interface MyTheme {
  selected: (s: string) => string;
  normal: (s: string) => string;
}
```

## 调试日志

设置 `PI_TUI_WRITE_LOG` 以捕获写入 stdout 的原始 ANSI 流。

```bash
PI_TUI_WRITE_LOG=/tmp/tui-ansi.log npx tsx packages/tui/test/chat-simple.ts
```

## 性能

尽可能地缓存渲染输出:

```typescript
class CachedComponent {
  private cachedWidth?: number;
  private cachedLines?: string[];

  render(width: number): string[] {
    if (this.cachedLines && this.cachedWidth === width) {
      return this.cachedLines;
    }
    // ... compute lines ...
    this.cachedWidth = width;
    this.cachedLines = lines;
    return lines;
  }

  invalidate(): void {
    this.cachedWidth = undefined;
    this.cachedLines = undefined;
  }
}
```

在状态发生变化时调用 `invalidate()`,然后调用 `handle.requestRender()` 以触发重新渲染。

## 失效处理与主题变化

当主题发生变化时,TUI 会对所有组件调用 `invalidate()` 以清除其缓存。组件必须正确实现 `invalidate()`,才能确保主题变化真正生效。

### 问题所在

如果一个组件将主题颜色预先烘焙(pre-bake)进字符串(通过 `theme.fg()`、`theme.bg()` 等)并加以缓存,那么缓存的字符串中会包含旧主题的 ANSI 转义码。如果组件将带主题样式的内容单独存储起来,仅仅清空渲染缓存是不够的。

**错误做法**(主题颜色不会更新):

```typescript
class BadComponent extends Container {
  private content: Text;

  constructor(message: string, theme: Theme) {
    super();
    // Pre-baked theme colors stored in Text component
    this.content = new Text(theme.fg("accent", message), 1, 0);
    this.addChild(this.content);
  }
  // No invalidate override - parent's invalidate only clears
  // child render caches, not the pre-baked content
}
```

### 解决方案

以主题颜色构建内容的组件,必须在调用 `invalidate()` 时重新构建该内容:

```typescript
class GoodComponent extends Container {
  private message: string;
  private content: Text;

  constructor(message: string) {
    super();
    this.message = message;
    this.content = new Text("", 1, 0);
    this.addChild(this.content);
    this.updateDisplay();
  }

  private updateDisplay(): void {
    // Rebuild content with current theme
    this.content.setText(theme.fg("accent", this.message));
  }

  override invalidate(): void {
    super.invalidate();  // Clear child caches
    this.updateDisplay(); // Rebuild with new theme
  }
}
```

### 模式:在 invalidate 时重新构建

对于内容较复杂的组件:

```typescript
class ComplexComponent extends Container {
  private data: SomeData;

  constructor(data: SomeData) {
    super();
    this.data = data;
    this.rebuild();
  }

  private rebuild(): void {
    this.clear();  // Remove all children

    // Build UI with current theme
    this.addChild(new Text(theme.fg("accent", theme.bold("Title")), 1, 0));
    this.addChild(new Spacer(1));

    for (const item of this.data.items) {
      const color = item.active ? "success" : "muted";
      this.addChild(new Text(theme.fg(color, item.label), 1, 0));
    }
  }

  override invalidate(): void {
    super.invalidate();
    this.rebuild();
  }
}
```

### 何时需要注意这一点

以下情况需要这种模式:

1. **预先烘焙主题颜色**——使用 `theme.fg()` 或 `theme.bg()` 创建带样式的字符串,并存储在子组件中
2. **语法高亮**——使用 `highlightCode()`,它会应用基于主题的语法颜色
3. **复杂布局**——构建内嵌了主题颜色的子组件树

以下情况则不需要这种模式:

1. **使用主题回调函数**——传入类似 `(text) => theme.fg("accent", text)` 这样的函数,在渲染时才调用
2. **简单容器**——仅对其他组件进行分组,不添加带主题样式的内容
3. **无状态渲染**——每次 `render()` 调用时都重新计算带主题样式的输出(不做缓存)

## 常见模式

以下模式覆盖了扩展中最常见的 UI 需求。**请直接复用这些模式,而不要从零开始搭建。**

### 模式 1:选择对话框(SelectList)

用于让用户从选项列表中进行选择。使用 `@earendil-works/pi-tui` 中的 `SelectList`,并配合 `DynamicBorder` 来添加边框。

```typescript
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { DynamicBorder } from "@earendil-works/pi-coding-agent";
import { Container, type SelectItem, SelectList, Text } from "@earendil-works/pi-tui";

pi.registerCommand("pick", {
  handler: async (_args, ctx) => {
    const items: SelectItem[] = [
      { value: "opt1", label: "Option 1", description: "First option" },
      { value: "opt2", label: "Option 2", description: "Second option" },
      { value: "opt3", label: "Option 3" },  // description is optional
    ];

    const result = await ctx.ui.custom<string | null>((tui, theme, _kb, done) => {
      const container = new Container();

      // Top border
      container.addChild(new DynamicBorder((s: string) => theme.fg("accent", s)));

      // Title
      container.addChild(new Text(theme.fg("accent", theme.bold("Pick an Option")), 1, 0));

      // SelectList with theme
      const selectList = new SelectList(items, Math.min(items.length, 10), {
        selectedPrefix: (t) => theme.fg("accent", t),
        selectedText: (t) => theme.fg("accent", t),
        description: (t) => theme.fg("muted", t),
        scrollInfo: (t) => theme.fg("dim", t),
        noMatch: (t) => theme.fg("warning", t),
      });
      selectList.onSelect = (item) => done(item.value);
      selectList.onCancel = () => done(null);
      container.addChild(selectList);

      // Help text
      container.addChild(new Text(theme.fg("dim", "↑↓ navigate • enter select • esc cancel"), 1, 0));

      // Bottom border
      container.addChild(new DynamicBorder((s: string) => theme.fg("accent", s)));

      return {
        render: (w) => container.render(w),
        invalidate: () => container.invalidate(),
        handleInput: (data) => { selectList.handleInput(data); tui.requestRender(); },
      };
    });

    if (result) {
      ctx.ui.notify(`Selected: ${result}`, "info");
    }
  },
});
```

**示例:** [preset.ts](../examples/extensions/preset.ts)、[tools.ts](../examples/extensions/tools.ts)

### 模式 2:可取消的异步操作(BorderedLoader)

用于耗时且需要支持取消的操作。`BorderedLoader` 会显示一个加载动画,并处理 Esc 键取消操作。

```typescript
import { BorderedLoader } from "@earendil-works/pi-coding-agent";

pi.registerCommand("fetch", {
  handler: async (_args, ctx) => {
    const result = await ctx.ui.custom<string | null>((tui, theme, _kb, done) => {
      const loader = new BorderedLoader(tui, theme, "Fetching data...");
      loader.onAbort = () => done(null);

      // Do async work
      fetchData(loader.signal)
        .then((data) => done(data))
        .catch(() => done(null));

      return loader;
    });

    if (result === null) {
      ctx.ui.notify("Cancelled", "info");
    } else {
      ctx.ui.setEditorText(result);
    }
  },
});
```

**示例:** [qna.ts](../examples/extensions/qna.ts)、[handoff.ts](../examples/extensions/handoff.ts)

### 模式 3:设置/开关列表(SettingsList)

用于切换多个设置项。使用 `@earendil-works/pi-tui` 中的 `SettingsList`,并配合 `getSettingsListTheme()`。

```typescript
import { getSettingsListTheme } from "@earendil-works/pi-coding-agent";
import { Container, type SettingItem, SettingsList, Text } from "@earendil-works/pi-tui";

pi.registerCommand("settings", {
  handler: async (_args, ctx) => {
    const items: SettingItem[] = [
      { id: "verbose", label: "Verbose mode", currentValue: "off", values: ["on", "off"] },
      { id: "color", label: "Color output", currentValue: "on", values: ["on", "off"] },
    ];

    await ctx.ui.custom((_tui, theme, _kb, done) => {
      const container = new Container();
      container.addChild(new Text(theme.fg("accent", theme.bold("Settings")), 1, 1));

      const settingsList = new SettingsList(
        items,
        Math.min(items.length + 2, 15),
        getSettingsListTheme(),
        (id, newValue) => {
          // Handle value change
          ctx.ui.notify(`${id} = ${newValue}`, "info");
        },
        () => done(undefined),  // On close
        { enableSearch: true }, // Optional: enable fuzzy search by label
      );
      container.addChild(settingsList);

      return {
        render: (w) => container.render(w),
        invalidate: () => container.invalidate(),
        handleInput: (data) => settingsList.handleInput?.(data),
      };
    });
  },
});
```

**示例:** [tools.ts](../examples/extensions/tools.ts)

### 模式 4:持久化状态指示器

在页脚(footer)中显示状态,并使其在多次渲染间保持存在。适合用作模式指示器。

```typescript
// Set status (shown in footer)
ctx.ui.setStatus("my-ext", ctx.ui.theme.fg("accent", "● active"));

// Clear status
ctx.ui.setStatus("my-ext", undefined);
```

**示例:** [status-line.ts](../examples/extensions/status-line.ts)、[plan-mode.ts](../examples/extensions/plan-mode.ts)、[preset.ts](../examples/extensions/preset.ts)

### 模式 4b:工作指示器自定义

自定义 pi 在流式输出响应时所显示的内联工作指示器(working indicator)。

```typescript
// Static indicator
ctx.ui.setWorkingIndicator({ frames: [ctx.ui.theme.fg("accent", "●")] });

// Custom animated indicator
ctx.ui.setWorkingIndicator({
  frames: [
    ctx.ui.theme.fg("dim", "·"),
    ctx.ui.theme.fg("muted", "•"),
    ctx.ui.theme.fg("accent", "●"),
    ctx.ui.theme.fg("muted", "•"),
  ],
  intervalMs: 120,
});

// Hide the indicator entirely
ctx.ui.setWorkingIndicator({ frames: [] });

// Restore pi's default spinner
ctx.ui.setWorkingIndicator();
```

这仅影响正常流式输出时的工作指示器。压缩(compaction)和重试(retry)时的加载动画会保留其内置样式。自定义帧(frames)会被原样渲染,因此扩展需要自行添加所需的颜色。

**示例:** [working-indicator.ts](../examples/extensions/working-indicator.ts)

### 模式 5:编辑器上方/下方的小部件

在输入编辑器上方或下方显示持久化内容。适合用于待办事项列表、进度展示等场景。

```typescript
// Simple string array (above editor by default)
ctx.ui.setWidget("my-widget", ["Line 1", "Line 2"]);

// Render below the editor
ctx.ui.setWidget("my-widget", ["Line 1", "Line 2"], { placement: "belowEditor" });

// Or with theme
ctx.ui.setWidget("my-widget", (_tui, theme) => {
  const lines = items.map((item, i) =>
    item.done
      ? theme.fg("success", "✓ ") + theme.fg("muted", item.text)
      : theme.fg("dim", "○ ") + item.text
  );
  return {
    render: () => lines,
    invalidate: () => {},
  };
});

// Clear
ctx.ui.setWidget("my-widget", undefined);
```

**示例:** [plan-mode.ts](../examples/extensions/plan-mode.ts)

### 模式 6:自定义页脚

替换页脚(footer)。`footerData` 暴露了一些扩展默认无法访问的数据。

```typescript
ctx.ui.setFooter((tui, theme, footerData) => ({
  invalidate() {},
  render(width: number): string[] {
    // footerData.getGitBranch(): string | null
    // footerData.getExtensionStatuses(): ReadonlyMap<string, string>
    return [`${ctx.model?.id} (${footerData.getGitBranch() || "no git"})`];
  },
  dispose: footerData.onBranchChange(() => tui.requestRender()), // reactive
}));

ctx.ui.setFooter(undefined); // restore default
```

可以通过 `ctx.sessionManager.getBranch()` 和 `ctx.model` 获取 token 统计信息。

**示例:** [custom-footer.ts](../examples/extensions/custom-footer.ts)

### 模式 7:自定义编辑器(vim 模式等)

用自定义实现替换主输入编辑器。适用于模态编辑(如 vim)、不同的按键绑定(如 emacs)或特殊的输入处理需求。

```typescript
import { CustomEditor, type ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { matchesKey, truncateToWidth } from "@earendil-works/pi-tui";

type Mode = "normal" | "insert";

class VimEditor extends CustomEditor {
  private mode: Mode = "insert";

  handleInput(data: string): void {
    // Escape: switch to normal mode, or pass through for app handling
    if (matchesKey(data, "escape")) {
      if (this.mode === "insert") {
        this.mode = "normal";
        return;
      }
      // In normal mode, escape aborts agent (handled by CustomEditor)
      super.handleInput(data);
      return;
    }

    // Insert mode: pass everything to CustomEditor
    if (this.mode === "insert") {
      super.handleInput(data);
      return;
    }

    // Normal mode: vim-style navigation
    switch (data) {
      case "i": this.mode = "insert"; return;
      case "h": super.handleInput("\x1b[D"); return; // Left
      case "j": super.handleInput("\x1b[B"); return; // Down
      case "k": super.handleInput("\x1b[A"); return; // Up
      case "l": super.handleInput("\x1b[C"); return; // Right
    }
    // Pass unhandled keys to super (ctrl+c, etc.), but filter printable chars
    if (data.length === 1 && data.charCodeAt(0) >= 32) return;
    super.handleInput(data);
  }

  render(width: number): string[] {
    const lines = super.render(width);
    // Add mode indicator to bottom border (use truncateToWidth for ANSI-safe truncation)
    if (lines.length > 0) {
      const label = this.mode === "normal" ? " NORMAL " : " INSERT ";
      const lastLine = lines[lines.length - 1]!;
      // Pass "" as ellipsis to avoid adding "..." when truncating
      lines[lines.length - 1] = truncateToWidth(lastLine, width - label.length, "") + label;
    }
    return lines;
  }
}

export default function (pi: ExtensionAPI) {
  pi.on("session_start", (_event, ctx) => {
    // Factory receives theme and keybindings from the app
    ctx.ui.setEditorComponent((tui, theme, keybindings) =>
      new VimEditor(theme, keybindings)
    );
  });
}
```

**要点:**

- **继承 `CustomEditor`**(而不是基础的 `Editor`),以获得应用层的按键绑定(如 Esc 中止 Agent、ctrl+d 退出、切换模型等)
- 对于你不处理的按键,**调用 `super.handleInput(data)`**
- **工厂模式:** `setEditorComponent` 接收一个工厂函数,该函数会得到 `tui`、`theme` 和 `keybindings`
- **传入 `undefined`** 可恢复默认编辑器:`ctx.ui.setEditorComponent(undefined)`

**示例:** [modal-editor.ts](../examples/extensions/modal-editor.ts)

## 关键规则

1. **始终使用回调中提供的 theme**——不要直接导入 theme。请使用 `ctx.ui.custom((tui, theme, keybindings, done) => ...)` 回调中提供的 `theme`。

2. **始终为 DynamicBorder 的颜色参数标注类型**——写成 `(s: string) => theme.fg("accent", s)`,而不是 `(s) => theme.fg("accent", s)`。

3. **状态变化后调用 tui.requestRender()**——在 `handleInput` 中更新状态后,调用 `tui.requestRender()`。

4. **返回包含三个方法的对象**——自定义组件需要 `{ render, invalidate, handleInput }`。

5. **优先使用现有组件**——`SelectList`、`SettingsList`、`BorderedLoader` 覆盖了 90% 的场景。不要重复造轮子。

## 示例

- **选择界面**: [examples/extensions/preset.ts](../examples/extensions/preset.ts) - 使用 DynamicBorder 加边框的 SelectList
- **可取消的异步操作**: [examples/extensions/qna.ts](../examples/extensions/qna.ts) - 用于 LLM 调用的 BorderedLoader
- **设置开关**: [examples/extensions/tools.ts](../examples/extensions/tools.ts) - 用于工具启用/禁用的 SettingsList
- **状态指示器**: [examples/extensions/plan-mode.ts](../examples/extensions/plan-mode.ts) - setStatus 和 setWidget
- **工作指示器**: [examples/extensions/working-indicator.ts](../examples/extensions/working-indicator.ts) - setWorkingIndicator
- **自定义页脚**: [examples/extensions/custom-footer.ts](../examples/extensions/custom-footer.ts) - 带统计信息的 setFooter
- **自定义编辑器**: [examples/extensions/modal-editor.ts](../examples/extensions/modal-editor.ts) - 类 Vim 的模态编辑
- **贪吃蛇小游戏**: [examples/extensions/snake.ts](../examples/extensions/snake.ts) - 包含键盘输入、游戏循环的完整游戏
- **自定义工具渲染**: [examples/extensions/todo.ts](../examples/extensions/todo.ts) - renderCall 与 renderResult
