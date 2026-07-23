# tmux 设置

Pi 可以在 tmux 中运行，但 tmux 默认会剥离某些按键的修饰符信息。如果不进行配置，`Shift+Enter` 和 `Ctrl+Enter` 通常无法与普通的 `Enter` 区分开来。

## 推荐配置

添加到 `~/.tmux.conf`：

```tmux
set -g extended-keys on
set -g extended-keys-format csi-u
```

然后完全重启 tmux：

```bash
tmux kill-server
tmux
```

当 Kitty 键盘协议不可用时，Pi 会自动请求扩展按键报告。使用 `extended-keys-format csi-u` 时，tmux 会以 CSI-u 格式转发修饰按键，这是最可靠的配置。`extended-keys-format` 选项需要 tmux 3.5 或更高版本。

## 为什么推荐使用 `csi-u`

如果只设置：

```tmux
set -g extended-keys on
```

tmux 会默认使用 `extended-keys-format xterm`。当应用程序请求扩展按键报告时，修饰按键会以 xterm 的 `modifyOtherKeys` 格式转发，例如：

- `Ctrl+C` → `\x1b[27;5;99~`
- `Ctrl+D` → `\x1b[27;5;100~`
- `Ctrl+Enter` → `\x1b[27;5;13~`

使用 `extended-keys-format csi-u` 时，同样的按键会以如下方式转发：

- `Ctrl+C` → `\x1b[99;5u`
- `Ctrl+D` → `\x1b[100;5u`
- `Ctrl+Enter` → `\x1b[13;5u`

Pi 同时支持这两种格式，但推荐的 tmux 设置是使用 `csi-u`。

## 这解决了什么问题

如果没有启用 tmux 扩展按键，修饰过的 Enter 键会退化为旧版转义序列：

| 按键 | 未启用扩展按键 | 使用 `csi-u` |
|-----|-----------------|--------------|
| Enter | `\r` | `\r` |
| Shift+Enter | `\r` | `\x1b[13;2u` |
| Ctrl+Enter | `\r` | `\x1b[13;5u` |
| Alt/Option+Enter | `\x1b\r` | `\x1b[13;3u` |

这会影响默认的按键绑定（`Enter` 提交、`Shift+Enter` 换行）以及任何使用修饰过的 Enter 的自定义按键绑定。

## 要求

- 使用 `extended-keys-format csi-u` 需要 tmux 3.5 或更高版本（运行 `tmux -V` 检查）
- 终端模拟器需要支持扩展按键（Ghostty、Kitty、iTerm2、WezTerm、Windows Terminal）

对于 tmux 3.2 到 3.4，请省略 `extended-keys-format csi-u`；Pi 仍然支持 tmux 默认的 xterm `modifyOtherKeys` 格式。
