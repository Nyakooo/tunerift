//! Dump actual Ratatui render buffers for visual review (no network/audio).
use ratatui::{Terminal, backend::TestBackend, style::Color};
use tunerift::{
    app::App,
    config::{Config, Layout, Station, Theme},
    ui,
};
fn color(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::LightRed => "#ff6666".into(),
        Color::Black => "#000000".into(),
        Color::White => "#eeeeee".into(),
        _ => "#c0c0c0".into(),
    }
}
fn main() -> anyhow::Result<()> {
    let station = |name: &str, country: &str| Station {
        id: name.into(),
        name: name.into(),
        country: country.into(),
        tags: "Ambient · Downtempo".into(),
        urls: vec![
            "https://example.com/live".into(),
            "https://example.com/backup".into(),
        ],
    };
    let stations = vec![
        station("Groove Salad", "美国"),
        station("中国之声", "中国"),
        station("Radio Paradise", "美国"),
        station("Drone Zone", "美国"),
        station("音乐之声", "中国"),
    ];
    for layout in Layout::ALL {
        for simple in [false, true] {
            if simple && layout != Layout::Airwave {
                continue;
            }
            let mut a = App::new(
                Config {
                    simple,
                    layout: layout.clone(),
                    theme: if layout == Layout::Btop {
                        Theme::Ocean
                    } else if layout == Layout::Receiver {
                        Theme::Amber
                    } else {
                        Theme::Mint
                    },
                    last_station: Some(stations[0].clone()),
                    ..Default::default()
                },
                "~/.config/tunerift/config.toml".into(),
            );
            a.stations = stations.clone();
            a.status = "已恢复会话 · 待播放".into();
            a.logs = vec![
                "[config] 已恢复音量 / 主题 / 代理 / 目录源".into(),
                "[session] Groove Salad / 主线路".into(),
                "[ready] 等待继续收听".into(),
            ];
            let mut terminal = Terminal::new(TestBackend::new(110, 30))?;
            terminal.draw(|f| ui::draw(f, &mut a))?;
            let b = terminal.backend().buffer();
            let cells:Vec<_>=b.content.iter().map(|cell|serde_json::json!({"s":cell.symbol(),"fg":color(cell.fg),"bg":color(cell.bg)})).collect();
            let id = if simple {
                "simple".to_string()
            } else {
                format!("{layout:?}").to_lowercase()
            };
            std::fs::create_dir_all("work/screens")?;
            std::fs::write(
                format!("work/screens/{id}.json"),
                serde_json::to_vec(&serde_json::json!({"width":110,"height":30,"cells":cells}))?,
            )?;
            println!("{id}:\n{b:?}");
        }
    }
    Ok(())
}
