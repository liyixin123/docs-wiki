# 终端设置

Pi 使用 [Kitty 键盘协议](https://sw.kovidgoyal.net/kitty/keyboard-protocol/) 来可靠地检测修饰键。大多数现代终端都支持该协议，但有些需要额外配置。

## Kitty、iTerm2

开箱即用。

## Apple Terminal

Pi 会在可用时启用增强按键报告。如果 Terminal.app 对 `Shift+Enter` 仍然发送普通的 Return，pi 会使用本地 macOS 修饰键回退机制，将该 Return 视为 `Shift+Enter`。

此回退机制仅在 pi 与 Terminal.app 运行在同一台 Mac 上时有效。通过远程 SSH 时它无法检测本地键盘。

## Ghostty

添加到你的 Ghostty 配置文件（macOS 为 `~/Library/Application Support/com.mitchellh.ghostty/config`，Linux 为 `~/.config/ghostty/config`）：

```
keybind = alt+backspace=text:\x1b\x7f
```

旧版本的 Claude Code 可能添加过以下 Ghostty 映射：

```
keybind = shift+enter=text:\n
```

该映射会发送一个原始换行字节。在 pi 内部，这与 `Ctrl+J` 无法区分，因此 tmux 和 pi 都无法再看到真正的 `shift+enter` 按键事件。

如果你添加该映射的唯一原因是 Claude Code 2.x 或更新版本，你可以移除它，除非你想在 tmux 中使用 Claude Code，那种情况下仍然需要该 Ghostty 映射。

如果你希望通过该重映射让 `Shift+Enter` 在 tmux 中继续工作，请在 `~/.pi/agent/keybindings.json` 中把 `ctrl+j` 添加到 pi 的 `newLine` 按键绑定：

```json
{
  "newLine": ["shift+enter", "ctrl+j"]
}
```

## WezTerm

WezTerm 通常通过 xterm modifyOtherKeys 对 `Shift+Enter` 开箱即用。要显式使用 Kitty 键盘协议，请创建 `~/.wezterm.lua`：

```lua
local wezterm = require 'wezterm'
local config = wezterm.config_builder()
config.enable_kitty_keyboard = true
return config
```

在 macOS 上，WezTerm 默认将 `Option+Enter` 绑定为全屏操作。要将 `Option+Enter` 用于 pi 的后续查询排队功能，请添加以下按键覆盖：

```lua
local wezterm = require 'wezterm'
local config = wezterm.config_builder()
config.keys = {
  {
    key = 'Enter',
    mods = 'ALT',
    action = wezterm.action.SendString('\x1b[13;3u'),
  },
}
return config
```

如果你已经有一个 `config.keys` 表，将该条目添加进去即可。

在 WSL 上，WezTerm 可能需要一个可见的硬件光标来正确定位 CJK 输入法候选窗口。如果 CJK 输入法候选词没有跟随文本光标移动，请在运行 pi 前设置 `PI_HARDWARE_CURSOR=1`，或在设置中将 `showHardwareCursor` 设为 `true`。

## Alacritty

Alacritty 通常对 `Shift+Enter` 开箱即用。在 macOS 上，`Option+Enter` 可能会被当作普通的 `Enter` 传入。要将 `Option+Enter` 用于 pi 的后续查询排队功能，请添加到 `~/.config/alacritty/alacritty.toml`：

```toml
[[keyboard.bindings]]
key = "Enter"
mods = "Alt"
chars = "[13;3u"
```

修改配置后需重启 Alacritty。

## VS Code（集成终端）

VS Code 1.109.5 及更新版本默认在集成终端中启用 Kitty 键盘协议，因此 `Shift+Enter` 应该可以开箱即用。

早于 1.109.5 的 VS Code 版本需要为 `Shift+Enter` 显式配置终端按键绑定。

`keybindings.json` 文件位置：
- macOS：`~/Library/Application Support/Code/User/keybindings.json`
- Linux：`~/.config/Code/User/keybindings.json`
- Windows：`%APPDATA%\\Code\\User\\keybindings.json`

添加到 `keybindings.json`：

```json
{
  "key": "shift+enter",
  "command": "workbench.action.terminal.sendSequence",
  "args": { "text": "[13;2u" },
  "when": "terminalFocus"
}
```

## Windows Terminal

添加到 `settings.json`（Ctrl+Shift+, 或设置 → 打开 JSON 文件）以转发 pi 使用的修饰过的 Enter 键：

```json
{
  "actions": [
    {
      "command": { "action": "sendInput", "input": "[13;2u" },
      "keys": "shift+enter"
    },
    {
      "command": { "action": "sendInput", "input": "[13;3u" },
      "keys": "alt+enter"
    }
  ]
}
```

- `Shift+Enter` 插入一个换行。
- Windows Terminal 默认将 `Alt+Enter` 绑定为全屏操作。这会阻止 pi 接收 `Alt+Enter` 用于后续查询排队。
- 将 `Alt+Enter` 重新映射为 `sendInput` 会把真正的按键组合转发给 pi。

如果你已经有一个 `actions` 数组，将这些对象添加进去即可。如果旧的全屏行为仍然存在，请完全关闭并重新打开 Windows Terminal。

## xfce4-terminal、terminator

这些终端对转义序列的支持有限。像 `Ctrl+Enter` 和 `Shift+Enter` 这样修饰过的 Enter 键无法与普通的 `Enter` 区分，导致诸如 `submit: ["ctrl+enter"]` 之类的自定义按键绑定无法生效。

为获得最佳体验，请使用支持 Kitty 键盘协议的终端：
- [Kitty](https://sw.kovidgoyal.net/kitty/)
- [Ghostty](https://ghostty.org/)
- [WezTerm](https://wezfurlong.org/wezterm/)
- [iTerm2](https://iterm2.com/)
- [Alacritty](https://github.com/alacritty/alacritty)（需要编译时启用 Kitty 协议支持）

## IntelliJ IDEA（集成终端）

内置终端对转义序列的支持有限。在 IntelliJ 的终端中，Shift+Enter 无法与 Enter 区分。

如果你希望硬件光标可见，请在运行 pi 前设置 `PI_HARDWARE_CURSOR=1`（出于兼容性考虑，默认禁用）。

建议使用专用的终端模拟器以获得最佳体验。
