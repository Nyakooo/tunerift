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
        eprintln!("tunerift: {e:#}");
        std::process::exit(1)
    }
}
fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut portable = false;
    let mut explicit = None;
    let mut print_path = false;
    let mut check = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!(
                    "Tunerift — A terminal radio. Tune into the world.\n\nUsage: tunerift [--config PATH] [--portable] [--config-path] [--check]\n\n--portable     配置保存在可执行文件旁\n--config-path  打印配置路径，不启动界面\n--check        检查配置和声音设备\n\n鼠标：单击选台，双击播放；点击音量条、设置、切源\n键盘：↑↓ / J K 选台，Enter 播放，空格 停止，/ 搜索\n      M 极简，S 切源，R 换线，F 收藏，D 删除（需确认），P 设置，+ - 音量，Q 退出"
                );
                return Ok(());
            }
            "--version" | "-V" => {
                println!("tunerift {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--portable" => portable = true,
            "--config" => {
                explicit = Some(PathBuf::from(args.next().context("--config 需要文件路径")?))
            }
            "--config-path" => print_path = true,
            "--check" => check = true,
            _ => bail!("未知参数 {arg}，使用 --help 查看帮助"),
        }
    }
    let path = config_path(portable, explicit)?;
    if print_path {
        println!("{}", path.display());
        return Ok(());
    }
    let config = Config::load(&path)?;
    if check {
        println!("配置正常：{}", path.display());
        let mut stream =
            rodio::OutputStreamBuilder::open_default_stream().context("声音设备检查失败")?;
        stream.log_on_drop(false);
        println!(
            "声音设备正常：{} Hz / {} 声道",
            stream.config().sample_rate(),
            stream.config().channel_count()
        );
        return Ok(());
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!("请在交互式终端中运行；可用 --check 检查声音设备")
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
