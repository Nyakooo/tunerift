use anyhow::{Context, Result, bail};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use std::{
    io::{self, IsTerminal},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tunerift::{
    app::App,
    config::{Config, config_path},
    ui,
};

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
    }
}
fn main() {
    if let Err(e) = run() {
        eprintln!(
            "tunerift: {}",
            tunerift::i18n::message(tunerift::i18n::Language::English, &format!("{e:#}"))
        );
        std::process::exit(1)
    }
}
fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut portable = false;
    let mut explicit = None;
    let mut print_path = false;
    let mut print_cache_path = false;
    let mut check = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!(
                    "Tunerift — A terminal radio. Tune into the world.\n\nUsage: tunerift [--config PATH] [--portable] [--config-path] [--cache-path] [--check]\n\n--portable     Save config beside the executable\n--config-path  Print config path without opening the UI\n--cache-path   Print directory cache path\n--check        Check config and audio device\n\nMouse: click to select, double-click to play; click volume, settings and sources\nKeys: ↑↓ / j k Select, Enter Play, Space Play/Stop, / Search\n      m Minimal, s Source, r Route, f Favorite, d Delete (confirmed), p Settings\n      b Sources, g Favorites, + - Volume, q Quit, u Refresh, ? / F1 Help\nMenus: a Add, e Edit, d Delete; Settings: l Language (English / 简体中文)"
                );
                return Ok(());
            }
            "--version" | "-V" => {
                println!("tunerift {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--portable" => portable = true,
            "--config" => {
                explicit = Some(PathBuf::from(
                    args.next().context("--config requires a file path")?,
                ))
            }
            "--config-path" => print_path = true,
            "--cache-path" => print_cache_path = true,
            "--check" => check = true,
            _ => bail!("Unknown option {arg}; use --help for help"),
        }
    }
    let path = config_path(portable, explicit)?;
    if print_path {
        println!("{}", path.display());
        return Ok(());
    }
    if print_cache_path {
        println!(
            "{}",
            tunerift::cache::DirectoryCache::for_config(&path)
                .root
                .display()
        );
        return Ok(());
    }
    let config = Config::load(&path)?;
    if check {
        println!(
            "{}: {}",
            config.language.text("配置正常", "Config OK"),
            path.display()
        );
        let mut stream = rodio::OutputStreamBuilder::open_default_stream()
            .context("Audio device check failed")?;
        stream.log_on_drop(false);
        println!(
            "Audio device OK: {} Hz / {} channels",
            stream.config().sample_rate(),
            stream.config().channel_count()
        );
        return Ok(());
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!("Run in an interactive terminal; use --check to check the audio device")
    }
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    ctrlc::set_handler(move || flag.store(true, Ordering::Relaxed))?;
    let old_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        old_hook(info)
    }));
    let mut app = App::new(config, path);
    enable_raw_mode()?;
    let _guard = TerminalGuard;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(io::stdout()))?;
    terminal.clear()?;
    app.init();
    let result = (|| -> Result<()> {
        let mut redraw = true;
        while !app.quit && !stop.load(Ordering::Relaxed) {
            redraw |= app.tick();
            let live_meters = app.playing && !app.config.simple;
            if redraw || live_meters {
                terminal.draw(|f| ui::draw(f, &mut app))?;
                redraw = false;
            }
            if event::poll(Duration::from_millis(if live_meters { 80 } else { 200 }))? {
                redraw = true;
                match event::read()? {
                    Event::Key(k) if k.kind != KeyEventKind::Release => app.key(k),
                    Event::Mouse(m) => app.mouse(m),
                    _ => {}
                }
            }
        }
        Ok(())
    })();
    app.player.stop();
    app.save()?;
    result
}
