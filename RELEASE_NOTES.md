## Tunerift v0.1.1

### 更新 / Changes

- 代理设置行和输入框明文显示完整代理地址，包括用户名和密码。
  Proxy settings and input now show the complete URL, including credentials.
- 新增英文 README，中英文文档可相互切换；资源包包含英文说明。
  Added an English README and included it in release packages.
- 提供 Linux / macOS 一条 curl 命令安装，以及 Windows PowerShell 安装脚本，支持架构识别、SHA256 校验、指定版本和更新。
  Added one-command installers with architecture detection, SHA256 verification, version selection, and updates.
- 修复旧版 ShellCheck 兼容性、Windows 安装测试作用域及重复安装时的文件替换问题。
  Fixed ShellCheck compatibility, Windows installer test scopes, and executable replacement during updates.

### 下载和运行 / Download and run

选择匹配系统及架构的资源包。Windows 下载 `.zip`，Linux / macOS 下载 `.tar.gz`。解压后运行 `tunerift`（Windows 为 `tunerift.exe`）。配置文件会自动生成，无需账号、API Key、Python 或外部播放器。

Linux 需要系统 ALSA 运行库。macOS 若阻止首次运行，请按系统提示允许运行下载的程序。终端需要支持 UTF-8，建议使用支持真彩色和鼠标事件的终端。

资源包包含程序、README、MIT LICENSE 和版本说明。`SHA256SUMS` 可用于校验下载完整性。GitHub 自动附带的 Source code 是源码包，不是可执行程序。

所有可执行资源均由 GitHub Actions 原生构建、测试和打包。电台由第三方提供，可用性和地区限制以各电台为准。
