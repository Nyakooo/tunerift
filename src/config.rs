use anyhow::{Context, Result, bail};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    Airwave,
    Receiver,
    Ops,
    Tracker,
    Btop,
}
impl Layout {
    pub const ALL: [Self; 5] = [
        Self::Airwave,
        Self::Receiver,
        Self::Ops,
        Self::Tracker,
        Self::Btop,
    ];
    pub fn name(&self) -> &'static str {
        match self {
            Self::Airwave => "留白电波",
            Self::Receiver => "午夜接收机",
            Self::Ops => "链路控制台",
            Self::Tracker => "Tracker",
            Self::Btop => "btop",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Mint,
    Amber,
    Violet,
    Ocean,
    Mono,
}
impl Theme {
    pub const ALL: [Self; 5] = [
        Self::Mint,
        Self::Amber,
        Self::Violet,
        Self::Ocean,
        Self::Mono,
    ];
    pub fn name(&self) -> &'static str {
        match self {
            Self::Mint => "薄荷",
            Self::Amber => "琥珀",
            Self::Violet => "紫罗兰",
            Self::Ocean => "海洋",
            Self::Mono => "单色",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Station {
    pub id: String,
    pub name: String,
    pub country: String,
    pub tags: String,
    pub urls: Vec<String>,
}
impl Default for Station {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: "自定义电台".into(),
            country: String::new(),
            tags: String::new(),
            urls: vec![],
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Subscription {
    pub name: String,
    pub location: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub volume: u8,
    pub layout: Layout,
    pub theme: Theme,
    pub simple: bool,
    pub autoplay: bool,
    pub proxy: String,
    pub spectrum: bool,
    pub source: usize,
    pub last_station: Option<Station>,
    pub last_route: usize,
    pub favorites: Vec<Station>,
    pub subscriptions: Vec<Subscription>,
    pub custom_stations: Vec<Station>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            volume: 65,
            layout: Layout::Airwave,
            theme: Theme::Mint,
            simple: true,
            autoplay: false,
            proxy: String::new(),
            spectrum: false,
            source: 0,
            last_station: None,
            last_route: 0,
            favorites: vec![],
            subscriptions: vec![],
            custom_stations: vec![],
        }
    }
}
impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            let c = Self::default();
            c.save(path)?;
            return Ok(c);
        }
        let mut c: Self = toml::from_str(&fs::read_to_string(path).context("读取配置失败")?)
            .context("配置 TOML 格式错误；原文件未修改")?;
        c.volume = c.volume.min(100);
        validate_proxy(&c.proxy)?;
        Ok(c)
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("toml.tmp");
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        use std::io::Write;
        let mut f = options.open(&tmp)?;
        f.write_all(toml::to_string_pretty(self)?.as_bytes())?;
        f.sync_all()?;
        fs::rename(&tmp, path).context("保存配置失败")?;
        Ok(())
    }
}
pub fn config_path(portable: bool, explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    if portable {
        return Ok(std::env::current_exe()?
            .parent()
            .context("找不到程序目录")?
            .join("config.toml"));
    }
    Ok(ProjectDirs::from("", "", "tunerift")
        .context("找不到用户配置目录")?
        .config_dir()
        .join("config.toml"))
}
pub fn validate_proxy(value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Ok(());
    }
    let url = url::Url::parse(value).context("代理需要完整地址，例如 socks5h://127.0.0.1:7890")?;
    if !matches!(url.scheme(), "http" | "https" | "socks5" | "socks5h") || url.host_str().is_none()
    {
        bail!("仅支持 HTTP / HTTPS / SOCKS5 / SOCKS5H 代理")
    }
    Ok(())
}
pub fn stream_url(value: &str) -> Result<url::Url> {
    let u = url::Url::parse(value)?;
    if !matches!(u.scheme(), "http" | "https") || u.host_str().is_none() {
        bail!("直播地址必须是 HTTP 或 HTTPS")
    };
    Ok(u)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_session_and_settings() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("config.toml");
        let mut c = Config::load(&path).unwrap();
        c.volume = 27;
        c.simple = false;
        c.layout = Layout::Btop;
        c.proxy = "socks5h://user:pass@127.0.0.1:1080".into();
        c.last_station = Some(Station {
            name: "测试电台".into(),
            urls: vec!["https://example.com/live".into()],
            ..Default::default()
        });
        c.last_route = 1;
        c.save(&path).unwrap();
        let r = Config::load(&path).unwrap();
        assert_eq!(r.volume, 27);
        assert_eq!(r.layout, Layout::Btop);
        assert_eq!(r.proxy, c.proxy);
        assert_eq!(r.last_station, c.last_station);
        assert_eq!(r.last_route, 1);
        assert!(!r.simple);
    }
    #[test]
    fn corrupt_config_is_not_overwritten() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("config.toml");
        fs::write(&p, "bad = [").unwrap();
        assert!(Config::load(&p).is_err());
        assert_eq!(fs::read_to_string(p).unwrap(), "bad = [");
    }
    #[test]
    fn reject_invalid_network_schemes() {
        assert!(validate_proxy("ftp://host").is_err());
        assert!(stream_url("file:///etc/passwd").is_err());
        assert!(validate_proxy("socks5h://localhost:1080").is_ok());
    }
}
