## Tunerift v0.1.3

### 更新 / Changes

- 新增电台排序：默认顺序、名称升序/降序、地区、收藏优先和手动顺序。按 `o` 切换，也可使用设置或鼠标按钮。
  Added default, name ascending/descending, country, favorites-first, and manual sorting; use `o`, Settings, or the mouse controls.
- 按 `Ctrl+↑/↓`、`[` / `]` 或顶部箭头移动电台。手动顺序按源保存，调整搜索结果时保留隐藏条目。
  Move stations with keyboard or arrow buttons. Manual ordering is saved per source and preserves hidden search entries.
- 新增电台别名（alias）：按 `a` 编辑，留空恢复原名。别名应用于列表、播放器和名称排序，并独立保存，不修改公共目录和播放地址。
  Added persistent station aliases, displayed in lists and the player and used for name sorting. Press `a` to edit; leave empty to restore the original name.
- 新增 `tunerift update` 原生更新命令：使用配置中的代理，下载最新正式 Release，验证 SHA256 后替换程序，保留设置和收藏。
  Added native self-update through `tunerift update`, with configured proxy support and SHA256 verification. Settings and favorites are preserved.
- 补充中英文说明及相关回归测试，由 GitHub Actions 执行六平台检查、构建和打包。
  Updated bilingual documentation and regression tests; GitHub Actions handles all platform checks and packaging.

### 从旧版本升级 / Upgrading

`v0.1.2` 及更早版本没有 `update` 命令，请在本版本发布完成后重新下载资源包，或重新运行 README 中的一键安装命令。确认 `tunerift --version` 输出 `0.1.3`，退出旧进程后重新启动；之后可使用 `tunerift update` 更新。

Versions up to `v0.1.2` do not have the `update` command. Once this release is published, download its package or rerun the one-command installer. Verify that `tunerift --version` reports `0.1.3`, then close old instances and restart. Future updates can use `tunerift update`.

### 下载和运行 / Download and run

选择匹配系统及架构的资源包。Windows 下载 `.zip`，Linux / macOS 下载 `.tar.gz`。解压后运行 `tunerift`（Windows 为 `tunerift.exe`）。配置文件会自动生成，无需账号、API Key、Python 或外部播放器。

Linux 需要系统 ALSA 运行库。macOS 若阻止首次运行，请按系统提示允许运行下载的程序。终端需要支持 UTF-8，建议使用支持真彩色和鼠标事件的终端。

资源包包含程序、README、MIT LICENSE 和版本说明。`SHA256SUMS` 可用于校验下载完整性。GitHub 自动附带的 Source code 是源码包，不是可执行程序。

所有可执行资源均由 GitHub Actions 原生构建、测试和打包。电台由第三方提供，可用性和地区限制以各电台为准。
