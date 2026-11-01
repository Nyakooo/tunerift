//! Bounded, cancellable network reader. Every request uses one explicit proxy policy.
use crate::config::stream_url;
use anyhow::{Context, Result, bail};
use std::{
    collections::VecDeque,
    io::{self, Read, Seek, SeekFrom},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use symphonia::core::io::MediaSource;
use tokio::runtime::Runtime;

pub struct Network {
    runtime: Runtime,
    client: reqwest::Client,
    cancel: Arc<AtomicBool>,
}
impl Network {
    pub fn new(proxy: &str, cancel: Arc<AtomicBool>) -> Result<Self> {
        crate::config::validate_proxy(proxy)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let mut b = reqwest::Client::builder()
            .no_proxy()
            .user_agent("Tunerift/0.1")
            .connect_timeout(Duration::from_secs(8))
            .read_timeout(Duration::from_secs(12));
        if !proxy.is_empty() {
            b = b.proxy(reqwest::Proxy::all(proxy)?)
        }
        Ok(Self {
            runtime,
            client: b.build()?,
            cancel,
        })
    }
    fn get(&self, url: &str) -> Result<reqwest::Response> {
        stream_url(url)?;
        let c = self.cancel.clone();
        self.runtime.block_on(async{let request=self.client.get(url).header("Icy-MetaData","0").send();tokio::pin!(request);loop{tokio::select!{r=&mut request=>return Ok(r?.error_for_status()?),_=tokio::time::sleep(Duration::from_millis(50))=>if c.load(Ordering::Relaxed){bail!("已停止")}}}})
    }
    fn chunk(&self, response: &mut reqwest::Response) -> Result<Option<Vec<u8>>> {
        let c = self.cancel.clone();
        self.runtime.block_on(async{let chunk=response.chunk();tokio::pin!(chunk);loop{tokio::select!{r=&mut chunk=>return Ok(r?.map(|b|b.to_vec())),_=tokio::time::sleep(Duration::from_millis(50))=>if c.load(Ordering::Relaxed){bail!("已停止")}}}})
    }
    fn bytes(&self, url: &str) -> Result<(String, Vec<u8>)> {
        let mut r = self.get(url)?;
        let final_url = r.url().to_string();
        let mut out = vec![];
        while let Some(c) = self.chunk(&mut r)? {
            out.extend(c);
            if out.len() > 16_000_000 {
                bail!("直播分片或播放列表过大")
            }
        }
        Ok((final_url, out))
    }
}
enum Input {
    Direct(reqwest::Response),
    Hls {
        url: String,
        playlist: m3u8_rs::MediaPlaylist,
        next: u64,
        initialized: bool,
        audio_pid: Option<u16>,
    },
}
pub struct RadioReader {
    network: Network,
    input: Input,
    pending: VecDeque<u8>,
}
impl RadioReader {
    pub fn open(url: &str, proxy: &str, cancel: Arc<AtomicBool>) -> Result<Self> {
        let network = Network::new(proxy, cancel)?;
        let mut url = url.to_string();
        for _ in 0..6 {
            let response = network.get(&url)?;
            let kind = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_lowercase();
            url = response.url().to_string();
            let path = url::Url::parse(&url)?.path().to_lowercase();
            if kind.contains("mpegurl")
                || kind.contains("scpls")
                || path.ends_with(".m3u")
                || path.ends_with(".m3u8")
                || path.ends_with(".pls")
            {
                let mut response = response;
                let mut bytes = vec![];
                while let Some(chunk) = network.chunk(&mut response)? {
                    bytes.extend(chunk);
                    if bytes.len() > 2_000_000 {
                        bail!("播放列表过大")
                    }
                }
                // Extended M3U station lists are not segmented HLS streams.
                if bytes.windows(7).any(|window| window == b"#EXT-X-") {
                    let playlist = m3u8_rs::parse_playlist_res(&bytes)
                        .map_err(|_| anyhow::anyhow!("HLS 播放列表格式无效"))?;
                    match playlist {
                        m3u8_rs::Playlist::MasterPlaylist(master) => {
                            let variant = master
                                .variants
                                .iter()
                                .filter(|v| !v.is_i_frame)
                                .min_by_key(|v| (v.resolution.is_some(), v.bandwidth))
                                .context("HLS 没有可用线路")?;
                            let alternate = variant.audio.as_ref().and_then(|group| {
                                master
                                    .alternatives
                                    .iter()
                                    .find(|a| a.group_id == *group && a.uri.is_some())
                            });
                            let next = alternate
                                .and_then(|a| a.uri.as_ref())
                                .unwrap_or(&variant.uri);
                            url = url::Url::parse(&url)?.join(next)?.to_string();
                            continue;
                        }
                        m3u8_rs::Playlist::MediaPlaylist(playlist) => {
                            let next = playlist.media_sequence
                                + if playlist.end_list {
                                    0
                                } else {
                                    playlist.segments.len().saturating_sub(3) as u64
                                };
                            return Ok(Self {
                                network,
                                input: Input::Hls {
                                    url,
                                    playlist,
                                    next,
                                    initialized: false,
                                    audio_pid: None,
                                },
                                pending: VecDeque::new(),
                            });
                        }
                    }
                }
                let text = String::from_utf8(bytes)?;
                let list = crate::directory::parse_playlist(&text, &url)?;
                url = list
                    .first()
                    .and_then(|s| s.urls.first())
                    .context("播放列表中没有 HTTP 音频地址")?
                    .clone();
            } else {
                return Ok(Self {
                    network,
                    input: Input::Direct(response),
                    pending: VecDeque::new(),
                });
            }
        }
        bail!("播放列表嵌套过多")
    }
    fn refill(&mut self) -> Result<bool> {
        if self.network.cancel.load(Ordering::Relaxed) {
            return Ok(false);
        }
        match &mut self.input {
            Input::Direct(response) => {
                if let Some(chunk) = self.network.chunk(response)? {
                    self.pending.extend(chunk);
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Input::Hls {
                url,
                playlist,
                next,
                initialized,
                audio_pid,
            } => loop {
                let offset = next.saturating_sub(playlist.media_sequence) as usize;
                if let Some(segment) = playlist.segments.get(offset) {
                    if segment
                        .key
                        .as_ref()
                        .is_some_and(|k| k.method != m3u8_rs::KeyMethod::None)
                    {
                        bail!("此 HLS 使用加密，请选择其他线路")
                    }
                    if segment.byte_range.is_some() {
                        bail!("暂不支持 HLS 字节范围分片，请选择其他线路")
                    }
                    let base = url::Url::parse(url)?;
                    if !*initialized {
                        if let Some(map) = &segment.map {
                            let (_, bytes) = self.network.bytes(base.join(&map.uri)?.as_str())?;
                            self.pending.extend(bytes)
                        }
                        *initialized = true;
                    }
                    let (_, bytes) = self.network.bytes(base.join(&segment.uri)?.as_str())?;
                    let audio = if bytes.first() == Some(&0x47) && bytes.get(188) == Some(&0x47) {
                        demux_ts(&bytes, audio_pid)?
                    } else {
                        bytes
                    };
                    self.pending.extend(audio);
                    *next += 1;
                    return Ok(true);
                }
                if playlist.end_list {
                    return Ok(false);
                }
                for _ in 0..(playlist.target_duration.clamp(1, 10) * 5) {
                    if self.network.cancel.load(Ordering::Relaxed) {
                        return Ok(false);
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                let (final_url, bytes) = self.network.bytes(url)?;
                let fresh = m3u8_rs::parse_media_playlist_res(&bytes)
                    .map_err(|_| anyhow::anyhow!("HLS 列表刷新失败"))?;
                if fresh.media_sequence > *next {
                    *next = fresh.media_sequence;
                }
                *url = final_url;
                *playlist = fresh;
            },
        }
    }
}
impl Read for RadioReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        while self.pending.is_empty() {
            if !self.refill().map_err(|e| io::Error::other(e.to_string()))? {
                return Ok(0);
            }
        }
        let n = out.len().min(self.pending.len());
        for b in &mut out[..n] {
            *b = self.pending.pop_front().unwrap();
        }
        Ok(n)
    }
}
impl Seek for RadioReader {
    fn seek(&mut self, _: SeekFrom) -> io::Result<u64> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "直播不支持跳转"))
    }
}
impl MediaSource for RadioReader {
    fn is_seekable(&self) -> bool {
        false
    }
    fn byte_len(&self) -> Option<u64> {
        None
    }
}

/// Extract MPEG audio / AAC ADTS PES payloads from 188-byte transport packets.
fn demux_ts(bytes: &[u8], audio_pid: &mut Option<u16>) -> Result<Vec<u8>> {
    let mut out = vec![];
    for packet in bytes.as_chunks::<188>().0 {
        if packet[0] != 0x47 || packet[1] & 0x80 != 0 {
            continue;
        }
        let pid = ((packet[1] as u16 & 0x1f) << 8) | packet[2] as u16;
        let control = (packet[3] >> 4) & 3;
        if control == 0 || control == 2 {
            continue;
        }
        let start = if control == 3 {
            5 + packet[4] as usize
        } else {
            4
        };
        if start >= 188 {
            continue;
        }
        let payload = &packet[start..];
        let unit_start = packet[1] & 0x40 != 0;
        if unit_start
            && payload.len() >= 9
            && payload[..3] == [0, 0, 1]
            && (0xc0..=0xdf).contains(&payload[3])
        {
            if audio_pid.is_none() {
                *audio_pid = Some(pid)
            }
            if *audio_pid == Some(pid) {
                let offset = 9 + payload[8] as usize;
                if offset <= payload.len() {
                    out.extend_from_slice(&payload[offset..])
                }
            }
        } else if !unit_start && *audio_pid == Some(pid) {
            out.extend_from_slice(payload)
        }
    }
    if audio_pid.is_none() {
        bail!("HLS TS 中未找到 MPEG/AAC 音轨，请换线路")
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transport_extracts_only_audio() {
        let mut packet = [0xff; 188];
        packet[..4].copy_from_slice(&[0x47, 0x41, 0x01, 0x10]);
        packet[4..13].copy_from_slice(&[0, 0, 1, 0xc0, 0, 0, 0x80, 0, 0]);
        packet[13..17].copy_from_slice(&[0xff, 0xf1, 0x50, 0x80]);
        let mut pid = None;
        let data = demux_ts(&packet, &mut pid).unwrap();
        assert_eq!(pid, Some(257));
        assert_eq!(&data[..4], &[0xff, 0xf1, 0x50, 0x80]);
    }
}
