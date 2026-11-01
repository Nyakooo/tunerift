//! Silent integration smoke test using a real audio output device.
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
use tunerift::audio::Player;
fn main() -> anyhow::Result<()> {
    let url = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("URL required"))?;
    let proxy = std::env::args().nth(2).unwrap_or_default();
    let (mut player, (tx, rx)) = (Player::new(0), mpsc::channel());
    player.play(url, proxy, tx);
    let deadline = Instant::now() + Duration::from_secs(25);
    let mut started = false;
    while Instant::now() < deadline {
        if let Ok(e) = rx.recv_timeout(Duration::from_millis(100)) {
            match e.result {
                Ok(m) => {
                    println!("{m}");
                    started = true;
                }
                Err(e) => anyhow::bail!("{e}"),
            }
        }
        if started && player.levels.lock().unwrap().samples.len() >= 1024 {
            println!("Decoded samples reached the audio device.");
            player.stop();
            return Ok(());
        }
    }
    anyhow::bail!("Playback timeout")
}
