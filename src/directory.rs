use crate::config::{Config, Station, stream_url, validate_proxy};
use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::{io::Read, time::Duration};

pub fn client(proxy: &str) -> Result<Client> {
    validate_proxy(proxy)?;
    let mut b = Client::builder()
        .no_proxy()
        .user_agent("Tunerift/0.1 (+https://github.com/Nyakooo/tunerift)")
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(15));
    if !proxy.trim().is_empty() {
        b = b.proxy(reqwest::Proxy::all(proxy)?)
    }
    Ok(b.build()?)
}
pub fn source_names(c: &Config) -> Vec<String> {
    let mut n = vec![
        "Radio Browser".into(),
        "SomaFM".into(),
        "收藏".into(),
        "自定义电台".into(),
    ];
    n.extend(c.subscriptions.iter().map(|s| s.name.clone()));
    n
}
pub fn bounded_text(response: reqwest::blocking::Response) -> Result<String> {
    let mut s = String::new();
    response
        .error_for_status()?
        .take(8_000_001)
        .read_to_string(&mut s)?;
    if s.len() > 8_000_000 {
        bail!("目录超过 8 MB 限制")
    };
    Ok(s)
}
#[derive(Deserialize)]
struct RadioStation {
    stationuuid: String,
    name: String,
    #[serde(default)]
    country: String,
    #[serde(default)]
    tags: String,
    url: String,
    #[serde(default)]
    url_resolved: String,
}
pub fn fetch(c: &Config, query: &str) -> Result<Vec<Station>> {
    match c.source {
        0 => radio_browser(&client(&c.proxy)?, query),
        1 => {
            let http = client(&c.proxy)?;
            let text = bounded_text(
                http.get("https://somafm.com/channels.json")
                    .send()
                    .context("SomaFM 无法连接")?,
            )?;
            let json: serde_json::Value = serde_json::from_str(&text)?;
            let mut out = vec![];
            for channel in json["channels"].as_array().context("SomaFM 目录格式错误")? {
                let name = channel["title"].as_str().unwrap_or("SomaFM");
                let mut urls = vec![];
                if let Some(lists) = channel["playlists"].as_array() {
                    for p in lists {
                        if let Some(u) = p["url"].as_str()
                            && stream_url(u).is_ok()
                        {
                            urls.push(u.into());
                        }
                    }
                }
                if !urls.is_empty() {
                    out.push(Station {
                        id: format!("soma:{}", channel["id"].as_str().unwrap_or(name)),
                        name: name.into(),
                        country: "美国".into(),
                        tags: channel["description"].as_str().unwrap_or("").into(),
                        urls,
                    });
                }
            }
            Ok(filter(out, query))
        }
        2 => Ok(filter(c.favorites.clone(), query)),
        3 => Ok(filter(c.custom_stations.clone(), query)),
        i => {
            let sub = c.subscriptions.get(i - 4).context("目录源已不存在")?;
            let text =
                if sub.location.starts_with("https://") || sub.location.starts_with("http://") {
                    bounded_text(client(&c.proxy)?.get(&sub.location).send()?)?
                } else {
                    std::fs::read_to_string(&sub.location).context("本地订阅文件读取失败")?
                };
            Ok(filter(parse_playlist(&text, &sub.location)?, query))
        }
    }
}
fn radio_browser(http: &Client, query: &str) -> Result<Vec<Station>> {
    // These are mirrors of ONE directory, not independent station sources.
    for host in [
        "https://de1.api.radio-browser.info",
        "https://nl1.api.radio-browser.info",
        "https://at1.api.radio-browser.info",
    ] {
        let result = (|| -> Result<Vec<Station>> {
            let response = http
                .get(format!("{host}/json/stations/search"))
                .query(&[
                    ("name", query),
                    ("hidebroken", "true"),
                    ("limit", "200"),
                    ("order", "clickcount"),
                    ("reverse", "true"),
                ])
                .send()?;
            let raw: Vec<RadioStation> = serde_json::from_str(&bounded_text(response)?)?;
            Ok(raw
                .into_iter()
                .filter_map(|r| {
                    let mut urls = vec![];
                    for u in [r.url_resolved, r.url] {
                        if stream_url(&u).is_ok() && !urls.contains(&u) {
                            urls.push(u)
                        }
                    }
                    if urls.is_empty() {
                        None
                    } else {
                        Some(Station {
                            id: r.stationuuid,
                            name: r.name,
                            country: r.country,
                            tags: r.tags,
                            urls,
                        })
                    }
                })
                .collect())
        })();
        if let Ok(stations) = result {
            return Ok(stations);
        }
    }
    bail!("Radio Browser 连接失败，请重试、检查代理或手动切换目录源")
}
fn filter(stations: Vec<Station>, query: &str) -> Vec<Station> {
    let q = query.to_lowercase();
    stations
        .into_iter()
        .filter(|s| {
            format!("{} {} {}", s.name, s.country, s.tags)
                .to_lowercase()
                .contains(&q)
        })
        .collect()
}
pub fn parse_playlist(text: &str, base: &str) -> Result<Vec<Station>> {
    if text.contains("#EXT-X-") {
        bail!("这是单个 HLS 直播地址，请通过“添加电台”添加，而非作为电台目录导入")
    }
    let base = url::Url::parse(base).ok();
    let resolve = |s: &str| -> Option<String> {
        let u = url::Url::parse(s)
            .ok()
            .or_else(|| base.as_ref().and_then(|b| b.join(s).ok()))?;
        stream_url(u.as_str()).ok().map(|u| u.to_string())
    };
    let is_pls = text
        .lines()
        .any(|line| line.trim().eq_ignore_ascii_case("[playlist]"));
    let mut out = vec![];
    let mut title = String::new();
    let mut pls = std::collections::BTreeMap::<usize, (String, String)>::new();
    for raw in text.lines() {
        let line = raw.trim().trim_start_matches('\u{feff}');
        if let Some(v) = line.strip_prefix("#EXTINF:") {
            title = v.split_once(',').map(|(_, t)| t).unwrap_or(v).trim().into();
            continue;
        }
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            let lower = key.to_lowercase();
            if let Some(n) = lower
                .strip_prefix("file")
                .and_then(|s| s.parse::<usize>().ok())
            {
                pls.entry(n).or_default().0 = val.trim().into();
                continue;
            }
            if let Some(n) = lower
                .strip_prefix("title")
                .and_then(|s| s.parse::<usize>().ok())
            {
                pls.entry(n).or_default().1 = val.trim().into();
                continue;
            }
        }
        if is_pls {
            continue;
        }
        if let Some(url) = resolve(line) {
            let name = if title.is_empty() {
                format!("电台 {}", out.len() + 1)
            } else {
                std::mem::take(&mut title)
            };
            out.push(Station {
                id: url.clone(),
                name,
                urls: vec![url],
                ..Default::default()
            });
        }
    }
    for (_, (url, name)) in pls {
        if let Some(url) = resolve(&url) {
            out.push(Station {
                id: url.clone(),
                name: if name.is_empty() {
                    "网络电台".into()
                } else {
                    name
                },
                urls: vec![url],
                ..Default::default()
            })
        }
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn m3u_relative_and_query() {
        let s = parse_playlist(
            "#EXTM3U\n#EXTINF:-1,中文台\n/live?a=b\nhttps://example.com/two\nfile:///etc/passwd",
            "https://example.com/list.m3u",
        )
        .unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].name, "中文台");
        assert_eq!(s[0].urls[0], "https://example.com/live?a=b");
    }
    #[test]
    fn pls_out_of_order() {
        let s=parse_playlist("[playlist]\nTitle2=Two\nFile2=https://example.com/2\nFile1=https://example.com/1\nTitle1=One","").unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].name, "One");
        assert_eq!(s[1].name, "Two");
    }
    #[test]
    fn pls_headers_are_not_treated_as_relative_audio_urls() {
        let stations = parse_playlist(
            "[playlist]\nNumberOfEntries=1\nFile1=/live?a=b\nTitle1=One\nLength1=-1\nVersion=2",
            "https://example.com/list.pls",
        )
        .unwrap();
        assert_eq!(stations.len(), 1);
        assert_eq!(stations[0].urls[0], "https://example.com/live?a=b");
    }
    #[test]
    fn hls_is_not_directory() {
        assert!(
            parse_playlist(
                "#EXTM3U\n#EXT-X-TARGETDURATION:10\nseg.aac",
                "https://a/list"
            )
            .is_err()
        );
    }
}
