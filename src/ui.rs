use crate::{
    app::{Action, App, FormKind, Modal},
    config::{Layout as Screen, Theme},
    directory,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{BarChart, Block, BorderType, Borders, Clear, Paragraph},
};

#[derive(Clone, Copy)]
struct Palette {
    fg: Color,
    dim: Color,
    accent: Color,
    secondary: Color,
    edge: Color,
    bg: Color,
}
fn palette(theme: &Theme) -> Palette {
    let (accent, secondary, bg) = match theme {
        Theme::Mint => (
            Color::Rgb(166, 216, 145),
            Color::Rgb(100, 207, 199),
            Color::Rgb(17, 24, 22),
        ),
        Theme::Amber => (
            Color::Rgb(237, 185, 97),
            Color::Rgb(205, 153, 108),
            Color::Rgb(30, 24, 17),
        ),
        Theme::Violet => (
            Color::Rgb(195, 165, 246),
            Color::Rgb(120, 190, 220),
            Color::Rgb(23, 21, 34),
        ),
        Theme::Ocean => (
            Color::Rgb(99, 215, 223),
            Color::Rgb(185, 156, 243),
            Color::Rgb(16, 19, 29),
        ),
        Theme::Mono => (Color::Gray, Color::White, Color::Black),
    };
    Palette {
        fg: Color::Rgb(215, 223, 220),
        dim: Color::Rgb(145, 160, 157),
        accent,
        secondary,
        edge: Color::Rgb(70, 85, 83),
        bg,
    }
}
fn inset(r: Rect, h: u16, v: u16) -> Rect {
    Rect {
        x: r.x + h.min(r.width),
        y: r.y + v.min(r.height),
        width: r.width.saturating_sub(h * 2),
        height: r.height.saturating_sub(v * 2),
    }
}
fn text(f: &mut Frame, r: Rect, s: impl Into<String>, color: Color) {
    f.render_widget(
        Paragraph::new(s.into()).style(Style::default().fg(color)),
        r,
    )
}
fn pane(f: &mut Frame, r: Rect, title: &str, color: Color, boxed: bool) -> Rect {
    if boxed {
        let b = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(color))
            .title(format!(" {title} "));
        let inner = b.inner(r);
        f.render_widget(b, r);
        inner
    } else {
        if r.height > 0 {
            text(f, Rect { height: 1, ..r }, title, color)
        }
        Rect {
            y: r.y + 1,
            height: r.height.saturating_sub(1),
            ..r
        }
    }
}
fn button(f: &mut Frame, a: &mut App, r: Rect, label: &str, action: Action, p: Palette) {
    if r.width == 0 || r.height == 0 {
        return;
    }
    let r = Rect { height: 1, ..r };
    f.render_widget(
        Paragraph::new(label).style(Style::default().fg(p.accent)),
        r,
    );
    a.hits.push((r, action));
}
fn row_buttons(f: &mut Frame, a: &mut App, r: Rect, buttons: Vec<(&str, Action)>, p: Palette) {
    let mut x = r.x;
    for (label, action) in buttons {
        let width = (Line::from(label).width() as u16 + 2).min(r.right().saturating_sub(x));
        if width == 0 {
            break;
        }
        button(
            f,
            a,
            Rect {
                x,
                width,
                height: 1,
                y: r.y,
            },
            label,
            action,
            p,
        );
        x += width;
    }
}
pub fn draw(f: &mut Frame, a: &mut App) {
    let area = f.area();
    let p = palette(&a.config.theme);
    a.hits.clear();
    f.render_widget(
        Block::default().style(Style::default().bg(p.bg).fg(p.fg)),
        area,
    );
    if area.width < 30 || area.height < 10 {
        text(f, area, "终端过小，请调整到至少 30×10。\n按 Q 退出。", p.fg);
        return;
    }
    let body = inset(area, 1, 0);
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(body);
    header(f, a, parts[0], p);
    let simple = a.config.simple || area.width < 65 || area.height < 20;
    if simple {
        simple_view(f, a, parts[1], p)
    } else {
        match a.config.layout {
            Screen::Airwave => airwave(f, a, parts[1], p),
            Screen::Receiver => receiver(f, a, parts[1], p),
            Screen::Ops => ops(f, a, parts[1], p),
            Screen::Tracker => tracker(f, a, parts[1], p),
            Screen::Btop => btop(f, a, parts[1], p),
        }
    }
    let footer = parts[2];
    let status = if a.loading {
        format!("加载目录… | {}", a.status)
    } else {
        a.status.clone()
    };
    text(
        f,
        Rect {
            height: 1,
            ..footer
        },
        status,
        p.dim,
    );
    text(
        f,
        Rect {
            y: footer.y + 1,
            height: 1,
            ..footer
        },
        "↑↓ 选台  Enter 播放  空格 停止  / 搜索  S 换源  R 换线  M 极简  Q 退出",
        p.dim,
    );
    if !matches!(a.modal, Modal::None) {
        a.hits.clear();
        modal(f, a, area, p)
    }
}
fn header(f: &mut Frame, a: &mut App, r: Rect, p: Palette) {
    let left = r.width.saturating_sub(47).max(8);
    text(
        f,
        Rect {
            width: left,
            height: 1,
            ..r
        },
        "◉ tunerift",
        p.accent,
    );
    row_buttons(
        f,
        a,
        Rect {
            x: r.x + left,
            width: r.width - left,
            height: 1,
            ..r
        },
        vec![
            ("[搜索]", Action::Search),
            ("[设置]", Action::Settings),
            (
                if a.config.simple {
                    "[专业]"
                } else {
                    "[极简]"
                },
                Action::Simple,
            ),
        ],
        p,
    );
}
fn simple_view(f: &mut Frame, a: &mut App, r: Rect, p: Palette) {
    let parts = Layout::vertical([Constraint::Min(3), Constraint::Length(4)]).split(r);
    stations(f, a, parts[0], p, false, false);
    player(f, a, parts[1], p, false, false);
}
fn airwave(f: &mut Frame, a: &mut App, r: Rect, p: Palette) {
    let cols = Layout::horizontal([Constraint::Percentage(46), Constraint::Percentage(54)])
        .spacing(3)
        .split(inset(r, 1, 1));
    player(f, a, cols[0], p, false, true);
    stations(f, a, cols[1], p, false, false);
}
fn receiver(f: &mut Frame, a: &mut App, r: Rect, p: Palette) {
    let parts = Layout::vertical([
        Constraint::Length(8),
        Constraint::Min(3),
        Constraint::Length(2),
    ])
    .spacing(1)
    .split(r);
    let inner = pane(
        f,
        parts[0],
        "NIGHT RECEIVER / 网络广播接收机",
        p.accent,
        true,
    );
    let name = a
        .active_station()
        .map(|s| s.name.as_str())
        .unwrap_or("选择电台 / 开始收听");
    f.render_widget(
        Paragraph::new(vec![
            Line::from("INTERNET RADIO · LAST SESSION").style(Style::default().fg(p.dim)),
            Line::from("┆ ╎ ┆ ╎ ┃ ╎ ┆ ╎ ┆ ╎ ┃ ╎ ┆ ╎ ┆").style(Style::default().fg(p.secondary)),
            Line::from(name).style(Style::default().fg(p.accent).add_modifier(Modifier::BOLD)),
            Line::from(if a.playing {
                "● 正在收听"
            } else if a.connecting {
                "◌ 连接中"
            } else {
                "■ 已恢复上次电台"
            }),
        ])
        .alignment(Alignment::Center),
        inner,
    );
    let rows = parts[1].height / 3;
    let capacity = rows as usize * 3;
    let start = a.selected.checked_div(capacity).unwrap_or(0) * capacity;
    for n in 0..capacity {
        let i = start + n;
        if i >= a.stations.len() {
            break;
        }
        let col = n % 3;
        let row = n / 3;
        let w = parts[1].width / 3;
        let cell = Rect {
            x: parts[1].x + col as u16 * w,
            y: parts[1].y + row as u16 * 3,
            width: w,
            height: 3,
        };
        let label = format!("{:02} {}", i + 1, a.stations[i].name);
        let color = if i == a.selected { p.accent } else { p.edge };
        let inner = pane(f, cell, &format!("PRESET {:02}", i + 1), color, true);
        button(f, a, inner, &label, Action::Station(i), p)
    }
    controls(f, a, parts[2], p);
}
fn ops(f: &mut Frame, a: &mut App, r: Rect, p: Palette) {
    let parts = Layout::vertical([Constraint::Min(5), Constraint::Length(6)])
        .spacing(1)
        .split(r);
    let cols = Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
        .spacing(1)
        .split(parts[0]);
    stations(f, a, cols[0], p, true, true);
    player(f, a, cols[1], p, true, true);
    log_panel(f, a, parts[1], p);
}
fn tracker(f: &mut Frame, a: &mut App, r: Rect, p: Palette) {
    let parts = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(4),
        Constraint::Length(2),
    ])
    .spacing(1)
    .split(r);
    let top = parts[0];
    player(f, a, top, p, false, false);
    stations(f, a, parts[1], p, false, true);
    sources(f, a, parts[2], p, false);
}
fn btop(f: &mut Frame, a: &mut App, r: Rect, p: Palette) {
    let parts = Layout::vertical([
        Constraint::Length(if a.config.spectrum { 9 } else { 7 }),
        Constraint::Min(4),
        Constraint::Length(4),
    ])
    .spacing(0)
    .split(r);
    player(f, a, parts[0], p, true, true);
    let cols = Layout::horizontal([Constraint::Percentage(66), Constraint::Percentage(34)])
        .spacing(1)
        .split(parts[1]);
    stations(f, a, cols[0], p, true, true);
    sources(f, a, cols[1], p, true);
    log_panel(f, a, parts[2], p);
}
fn stations(f: &mut Frame, a: &mut App, r: Rect, p: Palette, boxed: bool, dense: bool) {
    let names = directory::source_names(&a.config);
    let title = format!(
        "电台 / {}{}",
        names
            .get(a.config.source)
            .map(String::as_str)
            .unwrap_or("未知源"),
        if a.query.is_empty() {
            String::new()
        } else {
            format!(" / {}", a.query)
        }
    );
    let mut inner = pane(f, r, &title, p.accent, boxed);
    if dense && inner.height > 1 {
        text(
            f,
            Rect { height: 1, ..inner },
            "   STATION / 电台                          地区",
            p.dim,
        );
        inner.y += 1;
        inner.height -= 1
    }
    if a.stations.is_empty() {
        text(
            f,
            inner,
            if a.loading {
                "正在加载…"
            } else {
                "暂无电台。搜索 / 换源 / 在设置中添加电台。"
            },
            p.dim,
        );
        return;
    }
    let page = inner.height as usize;
    if page == 0 {
        return;
    }
    let start = a.selected / page * page;
    for offset in 0..page {
        let i = start + offset;
        let Some(station) = a.stations.get(i) else {
            break;
        };
        let favorite = a.config.favorites.iter().any(|s| s.id == station.id);
        let prefix = if a.selected == i {
            "›"
        } else if favorite {
            "☆"
        } else {
            " "
        };
        let line = if dense {
            Line::from(vec![
                Span::styled(
                    format!("{prefix} {:02} ", i + 1),
                    Style::default().fg(p.dim),
                ),
                Span::raw(format!("{}  ", station.name)),
                Span::styled(station.country.clone(), Style::default().fg(p.dim)),
            ])
        } else {
            Line::from(format!("{prefix} {}", station.name))
        };
        let row = Rect {
            y: inner.y + offset as u16,
            height: 1,
            ..inner
        };
        let style = if a.selected == i {
            Style::default()
                .fg(p.accent)
                .bg(Color::Rgb(38, 49, 48))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(p.fg)
        };
        f.render_widget(Paragraph::new(line).style(style), row);
        a.hits.push((row, Action::Station(i)));
    }
}
fn controls(f: &mut Frame, a: &mut App, r: Rect, p: Palette) {
    if r.height == 0 {
        return;
    }
    let playing = a.playing || a.connecting;
    let label = if playing {
        "[■ 停止]"
    } else {
        "[▶ 播放]"
    };
    row_buttons(
        f,
        a,
        r,
        vec![
            (label, Action::Play),
            ("[☆ 收藏]", Action::Favorite),
            ("[换线]", Action::Route),
        ],
        p,
    );
    let x = r.x + 34;
    if r.width > 50 {
        volume(
            f,
            a,
            Rect {
                x,
                y: r.y,
                width: r.right().saturating_sub(x),
                height: 1,
            },
            p,
        )
    } else if r.height > 1 {
        volume(
            f,
            a,
            Rect {
                y: r.y + 1,
                height: 1,
                ..r
            },
            p,
        )
    }
}
fn volume(f: &mut Frame, a: &mut App, r: Rect, p: Palette) {
    if r.width < 10 {
        return;
    }
    let label = format!("VOL {:3}% ", a.config.volume);
    text(
        f,
        Rect {
            width: label.len() as u16,
            ..r
        },
        label,
        p.dim,
    );
    let width = r.width.saturating_sub(9).min(18);
    let start = r.x + 9;
    for i in 0..width {
        let val = ((i + 1) as f32 / width as f32 * 100.).round() as u8;
        let filled = val <= a.config.volume;
        let cell = Rect {
            x: start + i,
            y: r.y,
            width: 1,
            height: 1,
        };
        text(
            f,
            cell,
            if filled { "━" } else { "┄" },
            if filled { p.accent } else { p.edge },
        );
        a.hits
            .push((cell, Action::Volume(if i == 0 { 0 } else { val })));
    }
}
fn player(f: &mut Frame, a: &mut App, r: Rect, p: Palette, boxed: bool, details: bool) {
    let inner = if boxed {
        pane(f, r, "1 / player · 上次会话", p.secondary, true)
    } else {
        r
    };
    if inner.height == 0 {
        return;
    }
    let name = a
        .active_station()
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "选择电台，开始收听".into());
    let state = if a.playing {
        "● PLAYING"
    } else if a.connecting {
        "◌ CONNECTING"
    } else {
        "■ READY"
    };
    let side_by_side = boxed && inner.width >= 70;
    let text_area = if side_by_side {
        Rect {
            width: inner.width / 2,
            ..inner
        }
    } else {
        inner
    };
    text(
        f,
        Rect {
            height: 1,
            ..text_area
        },
        format!("{state} / {name}"),
        p.accent,
    );
    if inner.height >= 3 {
        controls(
            f,
            a,
            Rect {
                y: inner.y + 2,
                height: if inner.height >= 4 { 2 } else { 1 },
                ..text_area
            },
            p,
        )
    }
    if details && inner.height >= 6 {
        let station = a.active_station();
        let tags = station.map(|s| s.tags.clone()).unwrap_or_default();
        text(
            f,
            Rect {
                y: inner.y + 4,
                height: 1,
                ..text_area
            },
            tags,
            p.dim,
        );
        let route = station
            .map(|s| {
                format!(
                    "线路 {}/{} · {}",
                    a.config.last_route + 1,
                    s.urls.len(),
                    if a.config.proxy.is_empty() {
                        "直连"
                    } else {
                        "代理已启用"
                    }
                )
            })
            .unwrap_or("配置与音量自动保存".into());
        text(
            f,
            Rect {
                y: inner.y + 5,
                height: 1,
                ..text_area
            },
            route,
            p.dim,
        );
    }
    let meter_area = if side_by_side {
        Rect {
            x: inner.x + inner.width / 2 + 1,
            width: inner.width - inner.width / 2 - 1,
            ..inner
        }
    } else if inner.height > 8 {
        Rect {
            y: inner.y + 7,
            height: inner.height - 7,
            ..inner
        }
    } else {
        return;
    };
    let levels = a
        .player
        .levels
        .lock()
        .ok()
        .filter(|l| l.updated.elapsed().as_millis() < 250);
    let (l, r) = if a.playing {
        levels
            .as_ref()
            .map(|l| (l.left, l.right))
            .unwrap_or((0., 0.))
    } else {
        (0., 0.)
    };
    let width = meter_area.width.saturating_sub(3).min(34) as usize;
    let meter = |v: f32| {
        let n = (v.clamp(0., 1.) * width as f32) as usize;
        format!("{}{}", "▰".repeat(n), "▱".repeat(width - n))
    };
    text(
        f,
        Rect {
            height: 1,
            ..meter_area
        },
        format!("L {}", meter(l)),
        p.accent,
    );
    if meter_area.height > 1 {
        text(
            f,
            Rect {
                y: meter_area.y + 1,
                height: 1,
                ..meter_area
            },
            format!("R {}", meter(r)),
            p.secondary,
        )
    }
    if a.config.spectrum && meter_area.height >= 5 {
        let samples = if a.playing {
            levels.map(|l| l.samples.clone()).unwrap_or_default()
        } else {
            vec![]
        };
        let values = crate::audio::spectrum(&samples);
        let data: Vec<_> = values.iter().map(|v| ("", *v)).collect();
        let bars = BarChart::default()
            .data(&data)
            .max(100)
            .bar_width(1)
            .bar_gap(1)
            .bar_style(Style::default().fg(p.secondary))
            .value_style(Style::default().fg(p.secondary).bg(p.bg));
        f.render_widget(
            bars,
            Rect {
                y: meter_area.y + 3,
                height: meter_area.height - 3,
                ..meter_area
            },
        );
    }
}
fn sources(f: &mut Frame, a: &mut App, r: Rect, p: Palette, boxed: bool) {
    let inner = pane(f, r, "目录源 / 手动切换", p.secondary, boxed);
    let names = directory::source_names(&a.config);
    if !boxed {
        let next = (a.config.source + 1) % names.len();
        button(
            f,
            a,
            inner,
            &format!("[源：{} → 切换]", names[a.config.source]),
            Action::Source(next),
            p,
        );
        return;
    }
    let available = inner.height.saturating_sub(3) as usize;
    for (i, name) in names.iter().take(available).enumerate() {
        let label = format!("{} {}", if a.config.source == i { "›" } else { " " }, name);
        button(
            f,
            a,
            Rect {
                y: inner.y + i as u16,
                height: 1,
                ..inner
            },
            &label,
            Action::Source(i),
            p,
        )
    }
    if inner.height >= 2 {
        let label = format!("[换线路：{}]", a.route + 1);
        button(
            f,
            a,
            Rect {
                y: inner.bottom() - 2,
                height: 1,
                ..inner
            },
            &label,
            Action::Route,
            p,
        );
        button(
            f,
            a,
            Rect {
                y: inner.bottom() - 1,
                height: 1,
                ..inner
            },
            "[管理源 / 添加]",
            Action::Settings,
            p,
        );
    }
}
fn log_panel(f: &mut Frame, a: &mut App, r: Rect, p: Palette) {
    let inner = pane(f, r, "events / 实际事件", p.edge, true);
    let n = inner.height as usize;
    let lines: Vec<Line> = a
        .logs
        .iter()
        .rev()
        .take(n)
        .rev()
        .map(|s| Line::from(s.clone()))
        .collect();
    f.render_widget(
        Paragraph::new(lines).style(Style::default().fg(p.dim)),
        inner,
    );
}
fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width.saturating_sub(2));
    let h = h.min(area.height.saturating_sub(2));
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}
fn modal(f: &mut Frame, a: &mut App, area: Rect, p: Palette) {
    let r = centered(
        area,
        76,
        match &a.modal {
            Modal::Settings(_) => 16,
            Modal::Sources => directory::source_names(&a.config).len() as u16 + 7,
            Modal::Stations => a.config.custom_stations.len() as u16 + 7,
            Modal::Form(form) => form.fields.len() as u16 * 3 + 7,
            _ => 7,
        },
    );
    f.render_widget(Clear, r);
    f.render_widget(Block::default().style(Style::default().bg(p.bg)), r);
    let title = match &a.modal {
        Modal::Search(_) => "搜索电台 · Enter 查询 / Esc 取消",
        Modal::Settings(_) => "设置 · 点击 / Enter 修改 / Esc 返回",
        Modal::Sources => "目录源 · 点击选择或管理 / Esc 返回",
        Modal::Stations => "自定义电台 · 编辑 / 删除 / Esc 返回",
        Modal::Form(form) => match form.kind {
            FormKind::Proxy => "代理 · 搜索和播放共用",
            FormKind::Station => "添加电台",
            FormKind::Subscription => "添加 M3U / PLS 订阅",
        },
        Modal::None => return,
    };
    let inner = pane(f, r, title, p.accent, true);
    match &a.modal {
        Modal::Search(s) => {
            text(f, inner, format!("/ {s}"), p.fg);
            text(
                f,
                Rect {
                    y: inner.y + 2,
                    height: 1,
                    ..inner
                },
                "名称搜索；本地列表也匹配地区与风格",
                p.dim,
            )
        }
        Modal::Settings(selected) => {
            let selected = *selected;
            let c = &a.config;
            let on = |v| if v { "开启" } else { "关闭" };
            let rows = [
                format!("界面布局       {}", c.layout.name()),
                format!("配色主题       {}", c.theme.name()),
                format!("极简模式       {}", on(c.simple)),
                format!("启动自动播放   {}", on(c.autoplay)),
                format!("真实频谱       {}", on(c.spectrum)),
                format!(
                    "代理设置       {}",
                    if c.proxy.is_empty() {
                        "直连"
                    } else {
                        "已设置（隐藏凭据）"
                    }
                ),
                "自定义电台 / 添加、编辑和删除".into(),
                "添加网络或本地 M3U / PLS".into(),
                "切换目录源 / 编辑和删除订阅".into(),
            ];
            for (i, label) in rows
                .iter()
                .enumerate()
                .take(inner.height.saturating_sub(2) as usize)
            {
                let row = Rect {
                    y: inner.y + i as u16,
                    height: 1,
                    ..inner
                };
                text(
                    f,
                    row,
                    format!("{} {label}", if selected == i { "›" } else { " " }),
                    if selected == i { p.accent } else { p.fg },
                );
                a.hits.push((row, Action::SettingsRow(i)));
            }
            if inner.height > 10 {
                text(
                    f,
                    Rect {
                        y: inner.bottom() - 2,
                        height: 1,
                        ..inner
                    },
                    format!("配置：{}", a.path.display()),
                    p.dim,
                );
                text(
                    f,
                    Rect {
                        y: inner.bottom() - 1,
                        height: 1,
                        ..inner
                    },
                    "修改立即生效并保存；代理凭据仅保存在本地。",
                    p.dim,
                )
            }
        }
        Modal::Form(form) => {
            let labels: Vec<&str> = match form.kind {
                FormKind::Proxy => vec!["代理 URL（空 = 直连；输入时隐藏）"],
                FormKind::Station => vec!["电台名称", "主线路 HTTP / HTTPS", "备用线路（可选）"],
                FormKind::Subscription => vec!["订阅名称", "HTTP(S) 地址或本地文件完整路径"],
            };
            for (i, label) in labels.iter().enumerate() {
                let y = inner.y + i as u16 * 3;
                if y + 1 >= inner.bottom() {
                    break;
                }
                text(
                    f,
                    Rect {
                        y,
                        height: 1,
                        ..inner
                    },
                    *label,
                    p.dim,
                );
                let value = if form.kind == FormKind::Proxy {
                    "•".repeat(form.fields[i].chars().count())
                } else {
                    form.fields[i].clone()
                };
                let row = Rect {
                    y: y + 1,
                    height: 1,
                    ..inner
                };
                text(
                    f,
                    row,
                    format!("{} {}", if form.focus == i { "›" } else { " " }, value),
                    if form.focus == i { p.accent } else { p.fg },
                );
                a.hits.push((row, Action::FormField(i)));
            }
            if inner.height >= 3 {
                text(
                    f,
                    Rect {
                        y: inner.bottom() - 3,
                        height: 1,
                        ..inner
                    },
                    &form.error,
                    Color::LightRed,
                );
                row_buttons(
                    f,
                    a,
                    Rect {
                        y: inner.bottom() - 1,
                        height: 1,
                        ..inner
                    },
                    vec![
                        ("[应用 / Enter]", Action::Apply),
                        ("[取消 / Esc]", Action::Cancel),
                    ],
                    p,
                )
            }
        }
        Modal::Sources => {
            let names = directory::source_names(&a.config);
            for (i, name) in names
                .iter()
                .enumerate()
                .take(inner.height.saturating_sub(2) as usize)
            {
                let row = Rect {
                    y: inner.y + i as u16,
                    height: 1,
                    ..inner
                };
                let remove_width = if i >= 4 { 16.min(row.width) } else { 0 };
                let select_row = Rect {
                    width: row.width.saturating_sub(remove_width),
                    ..row
                };
                let label = format!("{} {}", if a.config.source == i { "›" } else { " " }, name);
                button(f, a, select_row, &label, Action::Source(i), p);
                if i >= 4 {
                    button(
                        f,
                        a,
                        Rect {
                            x: select_row.right(),
                            width: remove_width / 2,
                            ..row
                        },
                        "[编辑]",
                        Action::EditSource(i - 4),
                        p,
                    );
                    button(
                        f,
                        a,
                        Rect {
                            x: select_row.right() + remove_width / 2,
                            width: remove_width / 2,
                            ..row
                        },
                        "[删除]",
                        Action::RemoveSource(i - 4),
                        p,
                    );
                }
            }
            button(
                f,
                a,
                Rect {
                    y: inner.bottom().saturating_sub(1),
                    height: 1,
                    ..inner
                },
                "[添加订阅]",
                Action::AddSource,
                p,
            );
        }
        Modal::Stations => {
            let names: Vec<_> = a
                .config
                .custom_stations
                .iter()
                .map(|s| s.name.clone())
                .collect();
            for (i, name) in names
                .iter()
                .enumerate()
                .take(inner.height.saturating_sub(2) as usize)
            {
                let row = Rect {
                    y: inner.y + i as u16,
                    height: 1,
                    ..inner
                };
                let width = row.width.saturating_sub(8);
                button(
                    f,
                    a,
                    Rect { width, ..row },
                    &format!("[编辑] {name}"),
                    Action::EditStation(i),
                    p,
                );
                button(
                    f,
                    a,
                    Rect {
                        x: row.x + width,
                        width: 8.min(row.width),
                        ..row
                    },
                    "[删除]",
                    Action::RemoveStation(i),
                    p,
                );
            }
            if names.is_empty() {
                text(f, inner, "尚未添加自定义电台", p.dim);
            }
            button(
                f,
                a,
                Rect {
                    y: inner.bottom().saturating_sub(1),
                    height: 1,
                    ..inner
                },
                "[添加电台]",
                Action::AddStation,
                p,
            );
        }
        Modal::None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Station};
    use ratatui::{Terminal, backend::TestBackend};
    #[test]
    fn all_layouts_render_and_hits_stay_inside_terminal() {
        for layout in Screen::ALL {
            for (w, h) in [(40, 15), (80, 24), (120, 38)] {
                let c = Config {
                    layout: layout.clone(),
                    simple: false,
                    ..Default::default()
                };
                let mut a = App::new(c, "unused.toml".into());
                a.stations = (0..20)
                    .map(|i| Station {
                        id: i.to_string(),
                        name: format!("测试电台 {i}"),
                        urls: vec!["https://example.com".into()],
                        ..Default::default()
                    })
                    .collect();
                let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
                t.draw(|f| draw(f, &mut a)).unwrap();
                assert!(!a.hits.is_empty());
                for (r, _) in &a.hits {
                    assert!(r.right() <= w && r.bottom() <= h, "{:?}: {:?}", layout, r);
                }
            }
        }
    }
    #[test]
    fn settings_hide_underlying_mouse_targets() {
        let mut a = App::new(Config::default(), "unused.toml".into());
        a.modal = Modal::Settings(0);
        let mut t = Terminal::new(TestBackend::new(90, 30)).unwrap();
        t.draw(|f| draw(f, &mut a)).unwrap();
        assert!(
            a.hits
                .iter()
                .all(|(_, action)| matches!(action, Action::SettingsRow(_)))
        );
    }
}
