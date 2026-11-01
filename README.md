# tunerift

**A terminal radio. Tune into the world.**

用 Rust 构建的跨平台终端收音机。支持鼠标操作、多源切换、代理、会话记忆和一键极简模式。免费开源，不需要账号、API Key、Python 或外部播放器。

目前为早期开发版本。电台由第三方提供，可用性、地区限制及使用条件以各台为准。

## 使用

运行对应系统和架构的 `tunerift`（Windows 为 `tunerift.exe`）。首次运行自动生成 `config.toml`。默认打开极简界面，专业界面可随时一键切换，不打断播放。

```sh
./tunerift
./tunerift --config-path
./tunerift --portable
./tunerift --config /path/to/config.toml
./tunerift --check
```

`--check` 检查配置和声音设备；`--portable` 将配置放在可执行文件旁边。

电脑端目标：Windows、Linux、macOS 的 x86_64 和 ARM64。当前主机只能验证 Linux x86_64；其他平台由 CI 原生构建和测试，需待实际运行结果确认。暂不支持手机、Termux、树莓派专用适配或 ARMv7。

## 操作

| 操作 | 鼠标 | 键盘 |
| --- | --- | --- |
| 选择电台 | 单击 | ↑ / ↓ 或 J / K |
| 播放电台 | 双击或播放按钮 | Enter |
| 播放 / 停止 | 点击播放按钮 | 空格 |
| 搜索 | 点击搜索 | / |
| 设置 | 点击设置 | P 或逗号 |
| 一键极简 / 恢复专业布局 | 点击极简 / 专业 | M |
| 切换目录源 | 专业界面的源列表；极简模式下在设置中切换和管理 | S |
| 切换播放线路 | 点击换线 | R |
| 调整音量 | 点击或拖动音量条 | + / - |
| 收藏 / 移除收藏 | 点击收藏；移除时二次确认 | F |
| 删除收藏或自定义条目 | 点击底部删除；确认后执行 | D / Delete |
| 添加电台 | 设置 → 添加电台 | N |
| 强制联网刷新目录 | 点击刷新 | u |
| 快捷键帮助 | 点击帮助 | ? / F1 |
| 退出 | — | Q / Ctrl+C |

终端需支持 UTF-8、真彩色和鼠标事件。普通终端里按住 Shift 可以绕过鼠标捕获进行文本选择（具体取决于终端）。小窗口会临时使用简化布局，不覆盖已保存的模式。

## 界面与主题

五款专业布局共享同一播放核心：留白电波、午夜接收机、链路控制台、Tracker、btop。设置中分别选择布局和薄荷、琥珀、紫罗兰、海洋、单色配色。

默认使用小型左右声道电平。可在设置中开启基于真实解码音频的 FFT 频谱；停止或缓冲时不生成假波形。

## 电台源与线路

内置 [Radio Browser](https://www.radio-browser.info/) 和 [SomaFM](https://somafm.com/listen/)，另有收藏、自定义电台及 M3U/PLS 订阅。SomaFM 提供的线路用于个人收听，遵守其[使用说明](https://somafm.com/live/directstreamlinks.html)。

在设置中添加网络 M3U/PLS URL 或本地文件路径，并可编辑、删除已添加的订阅。自定义电台也可在设置中编辑和删除。HLS `.m3u8` 是单个直播地址，应通过“添加电台”添加，而不是作为多电台目录导入。添加电台时可同时设置备用线路。

目录源与同台线路是两层独立选择。Radio Browser 的多个镜像只属于一个目录源。切换目录不会自动停止当前电台，连接失败也不会自动替你换源。按 Enter 重试、R 换线路、S 换目录。

## 代理

设置 → 代理，输入完整地址。留空为直连，例如：

```text
http://127.0.0.1:7890
socks5h://127.0.0.1:1080
http://username:password@127.0.0.1:7890
```

搜索、直播、播放列表、跳转和 HLS 分片统一使用该配置。直连模式不继承系统代理环境变量；`socks5h` 将域名交给代理解析。修改代理后，当前播放会重新连接。

代理输入及主界面不显示凭据。凭据保存在本地配置中，未加密；Unix 创建的文件权限为 `0600`。不要将个人配置提交到 Git。

## 会话与配置

自动保存音量、布局、主题、极简模式、目录源、收藏、订阅、代理、最后播放的电台及线路。默认只恢复上次电台和设置，需手动播放。设置 → 启动行为可明确启用下次启动自动播放；旧配置中的 `autoplay` 字段继续忽略，新选项默认为关闭。

配置位置以 `tunerift --config-path` 输出为准：

- Windows：`%APPDATA%\tunerift\config\config.toml`
- Linux：`$XDG_CONFIG_HOME/tunerift/config.toml`，通常为 `~/.config/tunerift/config.toml`
- macOS：`~/Library/Application Support/tunerift/config.toml`

配置格式错误时程序会报错并保留原文件。可直接编辑 TOML 中的订阅、收藏和电台线路；不要在程序运行时同时编辑。

## 目录缓存

在线电台目录缓存 1 小时，搜索结果按源和关键词分别缓存 10 分钟。启动、切源或重复搜索时先显示缓存；过期后后台更新，更新失败时保留缓存列表。界面顶部显示数据来源及上次更新时间。

按 `u` 或点击刷新可绕过有效期强制联网更新。收藏、自定义电台及本地订阅每次直接读取，不使用磁盘目录缓存。直播音频不写入磁盘；播放时仍重新请求直播及播放列表。

缓存自动放在系统缓存目录中，每份配置独立保存，上限 10 MB、最多 128 个查询条目，超限清理旧条目。设置 → 清除目录和搜索缓存会二次确认，清除不影响收藏、配置和当前播放，也不会被正在进行的旧请求重新写回。

```sh
./tunerift --cache-path
```

## 界面语言

默认使用英文，支持 English / 简体中文。打开设置（`p`），点击 Language / 语言，或在设置中按 `l` 即可立即切换；选择自动保存，下次启动继续使用。旧配置未设置语言时默认英文。电台名称、订阅名称及搜索内容保持原样。

也可在配置文件中设置：

```toml
language = "english" # 或 "chinese"
```

## 菜单快捷键

- 设置：`↑↓` 选项、`Enter` 修改，也可直接用 `1…9`、`0` 选择，`l` 切换语言。启动行为在一级设置中直接切换，不打开二级页面。
- 电台源及自定义电台管理：`↑↓` 选择、`a` 添加、`e` 编辑、`d` 删除。目录源按 `Enter` 切换，自定义电台按 `Enter` 编辑。
- 输入框：`Tab` / `Shift+Tab` 切换字段、`Ctrl+u` 清空、`Enter` 应用、`Esc` 返回。
- 删除确认：`y` 确认、`n` / `Esc` 取消；默认选中取消。
- 二级页面按 `Esc` 或点击返回，会回到原菜单和原选中项；`F1` 查看帮助后也会保留输入内容。

## 播放范围

内置音频解码与系统声音输出，不调用 mpv/FFmpeg。支持解码器可处理的 MP3、AAC-LC、FLAC、Vorbis、WAV 等格式，以及常见未加密 HLS（MPEG-TS 中的 MPEG 音频/AAC、直接音频分片）。

加密 HLS、字节范围分片、部分 fragmented MP4、HE-AAC/SBR 等仍需要进一步兼容验证，遇到不支持的直播需更换线路。Linux 依赖系统声音设施（ALSA，常由 PipeWire/PulseAudio 提供默认输出），不是完全不依赖系统库的静态程序。

## 从源码构建

安装稳定版 Rust。Linux 额外安装编译依赖：

```sh
# Debian / Ubuntu
sudo apt-get install libasound2-dev pkg-config
cargo build --release --locked
cargo test --locked
cargo clippy --all-targets -- -D warnings
```

最终文件位于 `target/release/`。用户运行时不需要 Rust 工具链。CI 为六种电脑平台构建独立可执行文件，不存在同一二进制跨系统通用的情况。

## License

[MIT](LICENSE)。电台节目与第三方数据的权利归各自权利人所有。
