//! UI language and translations of application-generated messages.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    English,
    Chinese,
}
impl Language {
    pub fn toggle(&mut self) {
        *self = match self {
            Self::English => Self::Chinese,
            Self::Chinese => Self::English,
        };
    }
    pub fn text(self, chinese: &'static str, english: &'static str) -> &'static str {
        match self {
            Self::English => english,
            Self::Chinese => chinese,
        }
    }
}

#[macro_export]
macro_rules! ltext {
    ($language:expr, $chinese:literal, $english:literal) => {
        $language.text($chinese, $english)
    };
}
#[macro_export]
macro_rules! lformat {
    ($language:expr, $chinese:literal, $english:literal $(, $argument:expr)* $(,)?) => {
        match $language {
            $crate::i18n::Language::English => format!($english $(, $argument)*),
            $crate::i18n::Language::Chinese => format!($chinese $(, $argument)*),
        }
    };
}

/// Translate only generated message templates. Captured names, URLs and user input
/// are copied verbatim, so a station named after a UI label is never renamed.
pub fn message(language: Language, value: &str) -> String {
    if language == Language::Chinese {
        return value.into();
    }
    for &(chinese, english) in MESSAGES {
        if let Some(parts) = capture(chinese, value) {
            let mut output = String::new();
            let mut literals = english.split("{}");
            output.push_str(literals.next().unwrap_or_default());
            for (index, (part, literal)) in parts.iter().zip(literals).enumerate() {
                let generated_part = matches!(
                    (chinese, index),
                    ("缓存{} · 更新于 {}", 0)
                        | ("{}；继续显示缓存，按 u 重试", 0)
                        | ("播放停止：{} [Enter 重试 / R 换线 / S 换源]", 0)
                        | ("配置保存失败：{}", 0)
                );
                if generated_part {
                    output.push_str(&message(language, part));
                } else {
                    output.push_str(part);
                }
                output.push_str(literal);
            }
            return output;
        }
    }
    // Error chains contain translated application contexts followed by OS/library errors.
    if let Some((context, detail)) = value.split_once(": ") {
        let translated = message(language, context);
        if translated != context {
            return format!("{translated}: {}", message(language, detail));
        }
    }
    value.into()
}
fn capture<'a>(template: &str, value: &'a str) -> Option<Vec<&'a str>> {
    let mut literals = template.split("{}");
    let mut rest = value.strip_prefix(literals.next()?)?;
    let mut parts = vec![];
    let suffixes: Vec<_> = literals.collect();
    for (i, suffix) in suffixes.iter().enumerate() {
        if i + 1 == suffixes.len() {
            parts.push(rest.strip_suffix(suffix)?);
            rest = "";
        } else {
            let end = rest.find(suffix)?;
            parts.push(&rest[..end]);
            rest = &rest[end + suffix.len()..];
        }
    }
    rest.is_empty().then_some(parts)
}

const MESSAGES: &[(&str, &str)] = &[
    ("排序：默认顺序", "Sort: Default order"),
    ("排序：名称升序", "Sort: Name ascending"),
    ("排序：名称降序", "Sort: Name descending"),
    ("排序：地区", "Sort: Country"),
    ("排序：收藏优先", "Sort: Favorites first"),
    ("排序：手动顺序", "Sort: Manual order"),
    ("已更新手动顺序", "Manual order updated"),
    ("就绪", "Ready"),
    ("读取配置失败", "Failed to read config"),
    (
        "配置 TOML 格式错误；原文件未修改",
        "Invalid config TOML; original file was not modified",
    ),
    ("保存配置失败", "Failed to save config"),
    ("找不到程序目录", "Cannot locate executable directory"),
    ("找不到用户配置目录", "Cannot locate user config directory"),
    ("[直播地址]", "[stream URL]"),
    ("[代理地址]", "[proxy URL]"),
    ("已加载配置与上次会话", "Settings and last session loaded"),
    ("目录尚未加载", "Directory not loaded"),
    (
        "目录正在联网刷新，请稍候…",
        "Refreshing directory, please wait…",
    ),
    ("缓存{} · 更新于 {}", "Cache{} · Updated {}"),
    ("缓存加载 {} 个电台", "Loaded {} stations from cache"),
    ("联网获取目录…", "Fetching online directory…"),
    ("本地列表", "Local list"),
    (
        "已显示缓存，正在联网更新…",
        "Showing cache; refreshing online…",
    ),
    ("正在加载电台目录…", "Loading station directory…"),
    (
        "目录加载失败，请检查网络/订阅地址，或手动换源",
        "Directory failed; check network/subscription or switch source",
    ),
    (
        "联网目录 · 更新于 {}（未写入缓存）",
        "Online directory · Updated {} (not cached)",
    ),
    ("联网目录 · 更新于 {}", "Online directory · Updated {}"),
    ("（未写入缓存）", " (not cached)"),
    ("（已过期）", " (expired)"),
    (
        "{}；继续显示缓存，按 u 重试",
        "{}; showing cache, press u to retry",
    ),
    ("目录更新失败", "Directory refresh failed"),
    (
        "内存目录 · 更新失败",
        "In-memory directory · Refresh failed",
    ),
    (
        "播放停止：{} [Enter 重试 / R 换线 / S 换源]",
        "Playback stopped: {} [Enter Retry / r Route / s Source]",
    ),
    ("配置保存失败：{}", "Settings save failed: {}"),
    ("电台没有播放地址", "Station has no playback URL"),
    ("正在连接 {} / 线路 {}…", "Connecting to {} / Route {}…"),
    ("已停止", "Stopped"),
    ("先选择或添加一个电台", "Select or add a station first"),
    ("已收藏", "Added to favorites"),
    ("已选择线路 {} / {}", "Selected route {} / {}"),
    ("请先选择要删除的条目", "Select an entry to delete first"),
    (
        "公共目录不可删除，请在收藏或自定义列表中移除条目",
        "Public directory entries cannot be deleted; remove from favorites or custom stations",
    ),
    ("已移除收藏：{}", "Removed favorite: {}"),
    ("已删除自定义电台：{}", "Deleted custom station: {}"),
    (
        "内存列表 · 磁盘缓存已清除",
        "In-memory list · Disk cache cleared",
    ),
    (
        "目录缓存已清除，收藏与设置保持不变",
        "Directory cache cleared; favorites and settings kept",
    ),
    (
        "缓存清除失败，请检查缓存目录权限",
        "Cache clear failed; check cache folder permissions",
    ),
    ("已删除订阅：{}", "Deleted subscription: {}"),
    ("请填写电台名称", "Enter a station name"),
    (
        "名称和订阅地址不能为空",
        "Name and subscription URL cannot be empty",
    ),
    ("设置已应用", "Settings applied"),
    ("正在播放", "Playing"),
    (
        "直播已结束，可重试或换线路",
        "Stream ended; retry or switch route",
    ),
    ("直播连接失败", "Stream connection failed"),
    ("无法识别音频格式", "Unrecognized audio format"),
    ("没有可播放音轨", "No playable audio track"),
    (
        "此音频编码尚不支持，请更换线路",
        "Unsupported audio codec; switch route",
    ),
    ("无法打开声音设备", "Unable to open audio device"),
    (
        "连续音频解码失败，请更换线路",
        "Repeated audio decode failure; switch route",
    ),
    ("目录超过 8 MB 限制", "Directory exceeds the 8 MB limit"),
    ("SomaFM 无法连接", "Cannot connect to SomaFM"),
    ("SomaFM 目录格式错误", "Invalid SomaFM directory"),
    ("目录源已不存在", "Source no longer exists"),
    (
        "本地订阅文件读取失败",
        "Cannot read local subscription file",
    ),
    (
        "Radio Browser 连接失败，请重试、检查代理或手动切换目录源",
        "Radio Browser connection failed; retry, check proxy or switch source",
    ),
    (
        "这是单个 HLS 直播地址，请通过“添加电台”添加，而非作为电台目录导入",
        "This is an HLS stream; use Add station instead of importing a directory",
    ),
    (
        "代理需要完整地址，例如 socks5h://127.0.0.1:7890",
        "Enter a complete proxy URL, e.g. socks5h://127.0.0.1:7890",
    ),
    (
        "仅支持 HTTP / HTTPS / SOCKS5 / SOCKS5H 代理",
        "Only HTTP / HTTPS / SOCKS5 / SOCKS5H proxies are supported",
    ),
    (
        "直播地址必须是 HTTP 或 HTTPS",
        "Stream URL must use HTTP or HTTPS",
    ),
    (
        "直播分片或播放列表过大",
        "Stream segment or playlist is too large",
    ),
    ("播放列表过大", "Playlist is too large"),
    ("HLS 播放列表格式无效", "Invalid HLS playlist"),
    ("HLS 没有可用线路", "No usable HLS route"),
    (
        "播放列表中没有 HTTP 音频地址",
        "No HTTP audio URL in playlist",
    ),
    ("播放列表嵌套过多", "Too many nested playlists"),
    (
        "此 HLS 使用加密，请选择其他线路",
        "Encrypted HLS stream; select another route",
    ),
    (
        "暂不支持 HLS 字节范围分片，请选择其他线路",
        "HLS byte-range segments are unsupported; select another route",
    ),
    ("HLS 列表刷新失败", "HLS playlist refresh failed"),
    ("直播不支持跳转", "Live stream does not support seeking"),
    (
        "HLS TS 中未找到 MPEG/AAC 音轨，请换线路",
        "No MPEG/AAC track in HLS TS; switch route",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_messages_translate_without_changing_station_names() {
        assert_eq!(
            message(Language::English, "正在连接 就绪 / 线路 2…"),
            "Connecting to 就绪 / Route 2…"
        );
        assert_eq!(
            message(Language::English, "缓存（已过期） · 更新于 11-01 20:00"),
            "Cache (expired) · Updated 11-01 20:00"
        );
        assert_eq!(
            message(
                Language::English,
                "联网目录 · 更新于 11-01 20:00（未写入缓存）"
            ),
            "Online directory · Updated 11-01 20:00 (not cached)"
        );
        assert_eq!(
            message(
                Language::English,
                "目录加载失败，请检查网络/订阅地址，或手动换源；继续显示缓存，按 u 重试"
            ),
            "Directory failed; check network/subscription or switch source; showing cache, press u to retry"
        );
        assert_eq!(message(Language::Chinese, "就绪"), "就绪");
        assert_eq!(
            message(Language::English, "Station 自定义台"),
            "Station 自定义台"
        );
        assert_eq!(
            message(Language::English, "直播连接失败: connection refused"),
            "Stream connection failed: connection refused"
        );
    }
}
