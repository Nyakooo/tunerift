## Tunerift v0.1.7

### 更新 / Changes

- 新增 `v`（view）快捷键，临时显示设置了别名的电台原名，再按 `v` 立即恢复别名；长按不会反复切换。
  Press `v` (view) to temporarily display original station names, then press `v` again to restore aliases. Holding the key does not repeatedly toggle names.
- 鼠标悬浮和移动不影响原名回显，也不触发重绘；其他按键、点击、滚轮和拖动恢复别名并照常执行。
  Mouse hovering and movement do not dismiss the preview or trigger redraws. Other keys, clicks, scrolling, and dragging restore aliases and proceed normally.

### 升级 / Upgrade

`v0.1.3` 及之后版本的用户可运行 `tunerift update`。更早版本可重新运行 README 中的一键安装命令或下载资源包。

Users of `v0.1.3` or later can run `tunerift update`. For older versions, rerun the one-command installer in the README or download an archive.

### 下载和运行 / Download and run

选择匹配系统及架构的资源包。Windows 下载 `.zip`，Linux / macOS 下载 `.tar.gz`。解压后运行 `tunerift`（Windows 为 `tunerift.exe`）。配置文件会自动生成，无需账号、API Key、Python 或外部播放器。

Linux 需要系统 ALSA 运行库。macOS 若阻止首次运行，请按系统提示允许运行下载的程序。终端需要支持 UTF-8，建议使用支持真彩色和鼠标事件的终端。

资源包包含程序、README、MIT LICENSE 和版本说明。`SHA256SUMS` 可用于校验下载完整性。GitHub 自动附带的 Source code 是源码包，不是可执行程序。

所有可执行资源均由 GitHub Actions 原生构建、测试和打包。电台由第三方提供，可用性和地区限制以各电台为准。
