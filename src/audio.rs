use crate::stream::RadioReader;
use anyhow::{Context, Result, bail};
use rodio::{OutputStreamBuilder, Sink, Source, buffer::SamplesBuffer};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, errors::Error, formats::FormatOptions,
    io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
};

#[derive(Clone)]
pub struct Levels {
    pub left: f32,
    pub right: f32,
    pub samples: Vec<f32>,
    pub updated: std::time::Instant,
}
impl Default for Levels {
    fn default() -> Self {
        Self {
            left: 0.,
            right: 0.,
            samples: vec![],
            updated: std::time::Instant::now(),
        }
    }
}
pub struct AudioEvent {
    pub generation: u64,
    pub result: Result<String, String>,
}
pub struct Player {
    cancel: Arc<AtomicBool>,
    volume: Arc<AtomicU8>,
    sink: Arc<Mutex<Option<Arc<Sink>>>>,
    pub levels: Arc<Mutex<Levels>>,
    pub generation: u64,
    threads: Vec<thread::JoinHandle<()>>,
}
impl Player {
    pub fn new(volume: u8) -> Self {
        Self {
            cancel: Arc::new(AtomicBool::new(false)),
            volume: Arc::new(AtomicU8::new(volume)),
            sink: Arc::new(Mutex::new(None)),
            levels: Arc::new(Mutex::new(Levels::default())),
            generation: 0,
            threads: vec![],
        }
    }
    pub fn play(&mut self, url: String, proxy: String, tx: mpsc::Sender<AudioEvent>) {
        self.stop();
        self.threads.retain(|t| !t.is_finished());
        self.cancel = Arc::new(AtomicBool::new(false));
        self.generation += 1;
        let generation = self.generation;
        let cancel = self.cancel.clone();
        let volume = self.volume.clone();
        let sink_slot = self.sink.clone();
        let levels = self.levels.clone();
        self.threads.push(thread::spawn(move || {
            let result = play_inner(
                &url,
                &proxy,
                cancel.clone(),
                volume,
                sink_slot,
                levels,
                || {
                    let _ = tx.send(AudioEvent {
                        generation,
                        result: Ok("正在播放".into()),
                    });
                },
            );
            if !cancel.load(Ordering::Relaxed) {
                let message = match result {
                    Ok(()) => "直播已结束，可重试或换线路".into(),
                    Err(e) => redact_error(&format!("{e:#}"), &url, &proxy),
                };
                let _ = tx.send(AudioEvent {
                    generation,
                    result: Err(message),
                });
            }
        }));
    }
    pub fn volume(&self, value: u8) {
        self.volume.store(value, Ordering::Relaxed);
        if let Ok(guard) = self.sink.lock()
            && let Some(sink) = guard.as_ref()
        {
            sink.set_volume(value as f32 / 100.)
        }
    }
    pub fn stop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.generation += 1;
        if let Ok(mut guard) = self.sink.lock()
            && let Some(sink) = guard.take()
        {
            sink.stop()
        }
        if let Ok(mut levels) = self.levels.lock() {
            *levels = Levels::default();
        }
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        self.stop();
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}
fn redact_error(text: &str, url: &str, proxy: &str) -> String {
    let mut text = text.replace(url, "[直播地址]");
    if !proxy.is_empty() {
        text = text.replace(proxy, "[代理地址]");
        if let Ok(u) = url::Url::parse(proxy)
            && let Some(pass) = u.password()
        {
            text = text.replace(pass, "***")
        }
    }
    text.chars().take(500).collect()
}
fn play_inner(
    url: &str,
    proxy: &str,
    cancel: Arc<AtomicBool>,
    volume: Arc<AtomicU8>,
    sink_slot: Arc<Mutex<Option<Arc<Sink>>>>,
    levels: Arc<Mutex<Levels>>,
    started: impl FnOnce(),
) -> Result<()> {
    let reader = RadioReader::open(url, proxy, cancel.clone()).context("直播连接失败")?;
    if cancel.load(Ordering::Relaxed) {
        return Ok(());
    }
    let mss = MediaSourceStream::new(Box::new(reader), Default::default());
    let mut format = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .context("无法识别音频格式")?
        .format;
    let track = format.default_track().context("没有可播放音轨")?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("此音频编码尚不支持，请更换线路")?;
    let mut output = OutputStreamBuilder::open_default_stream().context("无法打开声音设备")?;
    output.log_on_drop(false);
    let sink = Arc::new(Sink::connect_new(output.mixer()));
    sink.set_volume(volume.load(Ordering::Relaxed) as f32 / 100.);
    {
        let mut slot = sink_slot.lock().unwrap();
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        *slot = Some(sink.clone());
    }
    let mut started = Some(started);
    let mut bad_packets = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            sink.stop();
            return Ok(());
        }
        while sink.len() > 8 {
            if cancel.load(Ordering::Relaxed) {
                sink.stop();
                return Ok(());
            }
            thread::sleep(Duration::from_millis(15));
        }
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => {
                bad_packets = 0;
                d
            }
            Err(Error::DecodeError(_)) => {
                bad_packets += 1;
                if bad_packets > 50 {
                    bail!("连续音频解码失败，请更换线路")
                }
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        let spec = *decoded.spec();
        let channels = spec.channels.count() as u16;
        let mut samples = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        samples.copy_interleaved_ref(decoded);
        sink.append(Metered::new(
            SamplesBuffer::new(channels, spec.rate, samples.samples().to_vec()),
            levels.clone(),
        ));
        if let Some(started) = started.take() {
            started()
        }
    }
    while !sink.empty() {
        if cancel.load(Ordering::Relaxed) {
            sink.stop();
            break;
        }
        thread::sleep(Duration::from_millis(30));
    }
    Ok(())
}
struct Metered {
    source: SamplesBuffer,
    levels: Arc<Mutex<Levels>>,
    channels: u16,
    index: usize,
    peaks: [f32; 2],
    window: Vec<f32>,
}
impl Metered {
    fn new(source: SamplesBuffer, levels: Arc<Mutex<Levels>>) -> Self {
        let channels = source.channels();
        Self {
            source,
            levels,
            channels,
            index: 0,
            peaks: [0.; 2],
            window: Vec::with_capacity(1024),
        }
    }
}
impl Iterator for Metered {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let sample = self.source.next()?;
        let channel = self.index % self.channels as usize;
        self.peaks[channel.min(1)] = self.peaks[channel.min(1)].max(sample.abs());
        if channel == 0 {
            self.window.push(sample)
        }
        self.index += 1;
        if self.window.len() >= 1024 {
            if let Ok(mut levels) = self.levels.try_lock() {
                levels.left = self.peaks[0];
                levels.right = if self.channels == 1 {
                    self.peaks[0]
                } else {
                    self.peaks[1]
                };
                levels.samples.clone_from(&self.window);
                levels.updated = std::time::Instant::now();
            }
            self.window.clear();
            self.peaks = [0.; 2];
        }
        Some(sample)
    }
}
impl Source for Metered {
    fn current_span_len(&self) -> Option<usize> {
        self.source.current_span_len()
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.source.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.source.total_duration()
    }
}
pub fn spectrum(samples: &[f32]) -> Vec<u64> {
    use rustfft::{FftPlanner, num_complex::Complex};
    if samples.len() < 1024 {
        return vec![0; 24];
    };
    let mut buffer: Vec<_> = samples
        .iter()
        .take(1024)
        .enumerate()
        .map(|(i, s)| {
            Complex::new(
                s * (0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / 1023.).cos()),
                0.,
            )
        })
        .collect();
    FftPlanner::new()
        .plan_fft_forward(1024)
        .process(&mut buffer);
    (0..24)
        .map(|i| {
            let low = (1.0_f32 * (512_f32).powf(i as f32 / 24.)) as usize;
            let high = ((512_f32).powf((i + 1) as f32 / 24.)) as usize;
            let power = buffer[low..high.max(low + 1).min(512)]
                .iter()
                .map(|c| c.norm())
                .fold(0_f32, f32::max)
                / 256.;
            ((20. * power.max(0.00001).log10() + 60.) / 60. * 100.).clamp(0., 100.) as u64
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fft_sine_has_energy_silence_does_not() {
        assert!(spectrum(&vec![0.; 1024]).iter().all(|v| *v == 0));
        let s: Vec<_> = (0..1024)
            .map(|i| (std::f32::consts::TAU * 32. * i as f32 / 1024.).sin())
            .collect();
        let bins = spectrum(&s);
        let peak = bins.iter().enumerate().max_by_key(|(_, v)| *v).unwrap().0;
        assert!((11..16).contains(&peak));
    }
    #[test]
    fn errors_hide_credentials() {
        let text = redact_error(
            "failed socks5://user:secret@host:1 secret",
            "https://stream",
            "socks5://user:secret@host:1",
        );
        assert!(!text.contains("secret"));
    }
}
