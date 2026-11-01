## Tunerift v0.1.0

首个公开版本：用 Rust 构建的免费开源终端收音机。

- Windows / Linux / macOS，x86_64 和 ARM64 共六种构建。
- 支持鼠标操作、五种专业布局、主题和一键极简模式。
- 默认英文，设置中按 `l` 切换简体中文，立即生效并保存。
- Radio Browser、SomaFM、自定义电台、在线或本地 M3U / PLS 订阅。
- 支持 HTTP / HTTPS / SOCKS5 代理、多源切换及备用播放线路。
- 自动保存收藏、音量、设置和最后收听的电台；默认不自动播放。
- 目录和搜索缓存、后台刷新、手动强制刷新和清除缓存。

### 下载和运行 / Download and run

选择匹配系统及架构的资源包。Windows 下载 `.zip`，Linux / macOS 下载 `.tar.gz`。解压后运行 `tunerift`（Windows 为 `tunerift.exe`）。配置文件会自动生成，无需账号、API Key、Python 或外部播放器。

Linux 需要系统 ALSA 运行库。macOS 若阻止首次运行，请按系统提示允许运行下载的程序。终端需要支持 UTF-8，建议使用支持真彩色和鼠标事件的终端。

资源包包含程序、README、MIT LICENSE 和版本说明。`SHA256SUMS` 可用于校验下载完整性。GitHub 自动附带的 Source code 是源码包，不是可执行程序。

所有可执行资源均由 GitHub Actions 原生构建、测试和打包。电台由第三方提供，可用性和地区限制以各电台为准。
