# Shell 别名

Pi 以非交互模式（`bash -c`）运行 bash，默认情况下不会展开别名（alias）。

要启用你的 shell 别名，请添加到 `~/.pi/agent/settings.json`：

```json
{
  "shellCommandPrefix": "shopt -s expand_aliases\neval \"$(grep '^alias ' ~/.zshrc)\""
}
```

请根据你的 shell 配置调整路径（`~/.zshrc`、`~/.bashrc` 等）。
