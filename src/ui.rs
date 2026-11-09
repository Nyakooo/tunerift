use crate::{
    app::{Action, App, DeleteTarget, FormKind, Modal},
    config::{Layout as Screen, Theme},
    directory,
    effects::Effect,
    i18n::Language,
};
use crate::{lformat, ltext};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{BarChart, Block, BorderType, Borders, Clear, Paragraph, Wrap},
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
    let lang = a.config.language;
    let area = f.area();
    let p = palette(&a.config.theme);
    a.hits.clear();
    f.render_widget(
        Block::default().style(Style::default().bg(p.bg).fg(p.fg)),
        area,
    );
    if area.width < 30 || area.height < 10 {
        text(
            f,
            area,
            ltext!(
                lang,
                "终端过小，请调整到至少 30×10。\n按 Q 退出。",
                "Terminal too small. Resize to at least 30×10.\nPress Q to quit."
            ),
            p.fg,
        );
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
        lformat!(
            lang,
            "加载目录… | {}",
            "Loading directory… | {}",
            crate::i18n::message(lang, &a.status)
        )
    } else {
        crate::i18n::message(lang, &a.status)
    };
    let can_delete = matches!(a.config.source, 2 | 3) && a.selected_station().is_some();
    let delete_width = if can_delete { 11.min(footer.width) } else { 0 };
    text(
        f,
        Rect {
            height: 1,
            width: footer.width.saturating_sub(delete_width),
            ..footer
        },
        status,
        p.dim,
    );
    if can_delete {
        button(
            f,
            a,
            Rect {
                x: footer.right() - delete_width,
                width: delete_width,
                height: 1,
                ..footer
            },
            ltext!(lang, "[d 删除]", "[d Delete]"),
            Action::DeleteSelected,
            p,
        );
    }
    text(
        f,
        Rect {
            y: footer.y + 1,
            height: 1,
            ..footer
        },
        ltext!(
            lang,
            "↑↓选台 Enter播放 E音效 p设置 f收藏 /搜索 s换源 r换线 m极简 u刷新 ?帮助 q退出",
            "↑↓ Enter E Effects p Settings / Search f Fav s Src r Route m Min u Refresh ? Help q Quit"
        ),
        p.dim,
    );
    if !matches!(a.modal, Modal::None) {
        a.hits.clear();
        modal(f, a, area, p)
    } else {
        effect_menu(f, a, area, p);
    }
}
fn header(f: &mut Frame, a: &mut App, r: Rect, p: Palette) {
    let lang = a.config.language;
    let left = r
        .width
        .saturating_sub(if lang == crate::i18n::Language::English {
            60
        } else {
            56
        })
        .max(8);
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
            (ltext!(lang, "[/ 搜索]", "[/ Search]"), Action::Search),
            (ltext!(lang, "[p 设置]", "[p Settings]"), Action::Settings),
            (
                if a.config.simple {
                    ltext!(lang, "[m 专业]", "[m Full]")
                } else {
                    ltext!(lang, "[m 极简]", "[m Minimal]")
                },
                Action::Simple,
            ),
            (
                ltext!(lang, "[u 刷新]", "[u Refresh]"),
                Action::ForceRefresh,
            ),
            (ltext!(lang, "[? 帮助]", "[? Help]"), Action::Help),
        ],
        p,
    );
    if r.height > 1 {
        let tools_width = if r.width >= 65 { 34 } else { 0 };
        text(
            f,
            Rect {
                y: r.y + 1,
                height: 1,
                width: r.width - tools_width,
                ..r
            },
            crate::i18n::message(lang, &a.directory_info),
            p.dim,
        );
        if tools_width > 0 {
            row_buttons(
                f,
                a,
                Rect {
                    x: r.right() - tools_width,
                    y: r.y + 1,
                    width: tools_width,
                    height: 1,
                },
                vec![
                    (ltext!(lang, "[a 别名]", "[a Alias]"), Action::Alias),
                    (ltext!(lang, "[o 排序]", "[o Sort]"), Action::Sort),
                    ("[↑]", Action::MoveStation(-1)),
                    ("[↓]", Action::MoveStation(1)),
                ],
                p,
            );
        }
    }
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
    let lang = a.config.language;
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
        ltext!(
            lang,
            "NIGHT RECEIVER / 网络广播接收机",
            "NIGHT RECEIVER / Internet radio"
        ),
        p.accent,
        true,
    );
    let name = a
        .active_station()
        .map(|s| a.config.station_name(s))
        .unwrap_or(ltext!(
            lang,
            "选择电台 / 开始收听",
            "Select a station / Start listening"
        ));
    f.render_widget(
        Paragraph::new(vec![
            Line::from("INTERNET RADIO · LAST SESSION").style(Style::default().fg(p.dim)),
            Line::from("┆ ╎ ┆ ╎ ┃ ╎ ┆ ╎ ┆ ╎ ┃ ╎ ┆ ╎ ┆").style(Style::default().fg(p.secondary)),
            Line::from(name).style(Style::default().fg(p.accent).add_modifier(Modifier::BOLD)),
            Line::from(if a.playing {
                ltext!(lang, "● 正在收听", "● Playing")
            } else if a.connecting {
                ltext!(lang, "◌ 连接中", "◌ Connecting")
            } else {
                ltext!(lang, "■ 已恢复上次电台", "■ Last station restored")
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
        let label = format!("{:02} {}", i + 1, a.config.station_name(&a.stations[i]));
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
    let lang = a.config.language;
    let names = directory::source_names(&a.config);
    let title = lformat!(
        lang,
        "电台 / {}{}",
        "Stations / {}{}",
        names
            .get(a.config.source)
            .map(String::as_str)
            .unwrap_or(ltext!(lang, "未知源", "Unknown source")),
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
            ltext!(
                lang,
                "   STATION / 电台                          地区",
                "   STATION                                COUNTRY"
            ),
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
                ltext!(lang, "正在加载…", "Loading…")
            } else {
                ltext!(
                    lang,
                    "暂无电台。搜索 / 换源 / 在设置中添加电台。",
                    "No stations. Search, switch source, or add in Settings."
                )
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
                Span::raw(format!("{}  ", a.config.station_name(station))),
                Span::styled(station.country.clone(), Style::default().fg(p.dim)),
            ])
        } else {
            Line::from(format!("{prefix} {}", a.config.station_name(station)))
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
    let lang = a.config.language;
    if r.height == 0 {
        return;
    }
    let playing = a.playing || a.connecting;
    let label = if playing {
        ltext!(lang, "[空格 停止]", "[Space Stop]")
    } else {
        ltext!(lang, "[空格 播放]", "[Space Play]")
    };
    let favorite = if a.selected_is_favorite() {
        ltext!(lang, "[f 已收藏]", "[f Favorited]")
    } else {
        ltext!(lang, "[f 收藏]", "[f Favorite]")
    };
    let buttons = vec![
        (label, Action::Play),
        (favorite, Action::Favorite),
        (ltext!(lang, "[r 换线]", "[r Route]"), Action::Route),
    ];
    let controls_width: u16 = buttons
        .iter()
        .map(|(label, _)| Line::from(*label).width() as u16 + 2)
        .sum();
    let label = format!(
        "[ E {} ▾ ]",
        a.config.audio_effect.label(lang == Language::English)
    );
    let label_width = Line::from(label.as_str()).width() as u16;
    let (label, label_width) = if r.width >= controls_width + label_width {
        (label.as_str(), label_width)
    } else {
        ("E ▾", 3)
    };
    let effect_below = r.height > 1 && r.width < controls_width + label_width;
    let button_width = if effect_below {
        r.width
    } else {
        controls_width.min(r.width.saturating_sub(label_width))
    };
    row_buttons(
        f,
        a,
        Rect {
            width: button_width,
            ..r
        },
        buttons,
        p,
    );
    let anchor = if effect_below {
        Rect::new(r.x, r.y + 1, label_width, 1)
    } else {
        Rect::new(r.x + button_width, r.y, label_width.min(r.width), 1)
    };
    button(f, a, anchor, label, Action::ToggleEffects, p);

    let x = anchor.right().saturating_add(2);
    if effect_below || r.right().saturating_sub(x) >= 19 {
        volume(
            f,
            a,
            Rect {
                x,
                y: anchor.y,
                width: r.right().saturating_sub(x),
                height: 1,
            },
            p,
        )
    } else if r.height > 1 && !effect_below {
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
fn effect_menu(f: &mut Frame, a: &mut App, area: Rect, p: Palette) {
    let Some(offset) = a.effect_menu else { return };
    let Some(anchor) = a
        .hits
        .iter()
        .find_map(|(rect, action)| (*action == Action::ToggleEffects).then_some(*rect))
    else {
        a.effect_menu = None;
        return;
    };
    let effects = Effect::SELECTABLE;
    let english = a.config.language == Language::English;
    let title = ltext!(a.config.language, "音效", "Audio effects");
    let width = effects
        .iter()
        .map(|effect| Line::from(effect.label(english)).width() as u16 + 4)
        .max()
        .unwrap_or_default()
        .max(Line::from(title).width() as u16 + 4)
        .max(anchor.width)
        .min(area.width);
    let wanted = effects.len() as u16 + 2;
    let above = anchor.y.saturating_sub(area.y);
    let below = area.bottom().saturating_sub(anchor.bottom());
    let opens_above = below < wanted && above > below;
    let height = wanted.min(if opens_above { above } else { below });
    if height < 3 {
        a.effect_menu = None;
        return;
    }
    let y = if opens_above {
        anchor.y - height
    } else {
        anchor.bottom()
    };
    let rect = Rect::new(anchor.x.min(area.right() - width), y, width, height);
    a.hits.push((rect, Action::EffectMenuSurface));
    f.render_widget(Clear, rect);
    let inner = pane(f, rect, title, p.accent, true);
    let visible = usize::from(inner.height);
    let offset = offset.min(effects.len() - visible);
    a.effect_menu = Some(offset);
    for (row, &effect) in effects.iter().skip(offset).take(visible).enumerate() {
        let active = effect == a.config.audio_effect;
        let prefix = if active { "✓ " } else { "  " };
        let label = format!("{prefix}{}", effect.label(english));
        let item = Rect::new(inner.x, inner.y + row as u16, inner.width, 1);
        let style = if active {
            Style::default().fg(p.accent).bg(Color::Rgb(38, 49, 48))
        } else {
            Style::default().fg(p.fg)
        };
        f.render_widget(Paragraph::new(label).style(style), item);
        a.hits.push((item, Action::SetEffect(effect)));
    }
    if offset > 0 {
        text(f, Rect::new(rect.right() - 1, inner.y, 1, 1), "↑", p.accent);
    }
    if offset + visible < effects.len() {
        text(
            f,
            Rect::new(rect.right() - 1, inner.bottom() - 1, 1, 1),
            "↓",
            p.accent,
        );
    }
}
fn volume(f: &mut Frame, a: &mut App, r: Rect, p: Palette) {
    if r.width < 10 {
        return;
    }
    let label = format!("VOL +/- {:3}% ", a.config.volume);
    let label_width = Line::from(label.as_str()).width() as u16;
    text(
        f,
        Rect {
            width: label_width.min(r.width),
            ..r
        },
        label,
        p.dim,
    );
    let width = r.width.saturating_sub(label_width).min(18);
    let start = r.x + label_width;
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
    let lang = a.config.language;
    let inner = if boxed {
        pane(
            f,
            r,
            ltext!(lang, "1 / player · 上次会话", "1 / player · Last session"),
            p.secondary,
            true,
        )
    } else {
        r
    };
    if inner.height == 0 {
        return;
    }
    let name = a
        .active_station()
        .map(|s| a.config.station_name(s).to_string())
        .unwrap_or_else(|| {
            ltext!(
                lang,
                "选择电台，开始收听",
                "Select a station to start listening"
            )
            .into()
        });
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
                lformat!(
                    lang,
                    "线路 {}/{} · {}",
                    "Route {}/{} · {}",
                    a.config.last_route + 1,
                    s.urls.len(),
                    if a.config.proxy.is_empty() {
                        ltext!(lang, "直连", "Direct")
                    } else {
                        ltext!(lang, "代理已启用", "Proxy enabled")
                    }
                )
            })
            .unwrap_or(
                ltext!(
                    lang,
                    "配置与音量自动保存",
                    "Settings and volume saved automatically"
                )
                .into(),
            );
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
    let lang = a.config.language;
    let inner = pane(
        f,
        r,
        ltext!(lang, "目录源 / 手动切换", "Sources / Manual switching"),
        p.secondary,
        boxed,
    );
    let names = directory::source_names(&a.config);
    if !boxed {
        let next = (a.config.source + 1) % names.len();
        button(
            f,
            a,
            inner,
            &lformat!(
                lang,
                "[s 源：{} → 切换]",
                "[s Source: {} → Switch]",
                names[a.config.source]
            ),
            Action::Source(next),
            p,
        );
        return;
    }
    let available = inner.height.saturating_sub(3) as usize;
    for (i, name) in names.iter().take(available).enumerate() {
        let label = format!(
            "{} {}{}",
            if a.menu_cursor == i { "›" } else { " " },
            name,
            if a.config.source == i {
                ltext!(lang, " [当前]", " [Active]")
            } else {
                ""
            }
        );
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
        let label = lformat!(lang, "[换线路：{}]", "[r Route: {}]", a.route + 1);
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
            ltext!(lang, "[b 源管理]", "[b Sources]"),
            Action::ManageSources,
            p,
        );
    }
}
fn log_panel(f: &mut Frame, a: &mut App, r: Rect, p: Palette) {
    let lang = a.config.language;
    let inner = pane(
        f,
        r,
        ltext!(lang, "events / 实际事件", "events / Activity"),
        p.edge,
        true,
    );
    let n = inner.height as usize;
    let lines: Vec<Line> = a
        .logs
        .iter()
        .rev()
        .take(n)
        .rev()
        .map(|s| Line::from(crate::i18n::message(lang, s)))
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
    let lang = a.config.language;
    let r = centered(
        area,
        76,
        match &a.modal {
            Modal::Settings(_) => 19,
            Modal::Sources => directory::source_names(&a.config).len() as u16 + 7,
            Modal::Stations => a.config.custom_stations.len() as u16 + 7,
            Modal::Form(form) => form.fields.len() as u16 * 3 + 7,
            Modal::ConfirmDelete { .. } => 9,
            Modal::Help => 18,
            _ => 7,
        },
    );
    f.render_widget(Clear, r);
    f.render_widget(Block::default().style(Style::default().bg(p.bg)), r);
    let title = match &a.modal {
        Modal::Search(_) => ltext!(
            lang,
            "搜索电台 · Enter 查询 / Esc 取消",
            "Search stations · Enter Search / Esc Cancel"
        ),
        Modal::Settings(_) => ltext!(
            lang,
            "设置 · ↑↓/数字选择 · l 语言 · Esc 返回",
            "Settings · ↑↓/Digits · l Language · Esc Back"
        ),
        Modal::Sources => ltext!(
            lang,
            "目录源 · ↑↓选择 / Enter 切换 / Esc 返回",
            "Sources · ↑↓ Select / Enter Switch / Esc Back"
        ),
        Modal::Stations => ltext!(
            lang,
            "自定义电台 · ↑↓选择 / Enter 编辑 / Esc 返回",
            "Custom stations · ↑↓ Select / Enter Edit / Esc Back"
        ),
        Modal::Help => ltext!(
            lang,
            "快捷键 · ↑↓滚动 · Esc 返回",
            "Shortcuts · ↑↓ Scroll · Esc Back"
        ),
        Modal::ConfirmDelete { .. } => ltext!(
            lang,
            "删除确认 · 默认取消",
            "Confirm deletion · Cancel by default"
        ),
        Modal::Form(form) => match form.kind {
            FormKind::Alias => ltext!(lang, "电台别名", "Station alias"),
            FormKind::Proxy => ltext!(
                lang,
                "代理 · 搜索和播放共用",
                "Proxy · Used for search and playback"
            ),
            FormKind::Station => ltext!(lang, "添加电台", "Add station"),
            FormKind::Subscription => {
                ltext!(lang, "添加 M3U / PLS 订阅", "Add M3U / PLS subscription")
            }
        },
        Modal::None => return,
    };
    let inner = pane(f, r, title, p.accent, true);
    match &a.modal {
        Modal::Search(s) => {
            let names = directory::source_names(&a.config);
            let source = names
                .get(a.config.source)
                .map(String::as_str)
                .unwrap_or(ltext!(lang, "未知源", "Unknown source"));
            text(
                f,
                Rect { height: 1, ..inner },
                lformat!(
                    lang,
                    "当前源：{source} · 仅搜索此源",
                    "Source: {source} · Search this source only"
                ),
                p.secondary,
            );
            text(
                f,
                Rect {
                    y: inner.y + 1,
                    height: 1,
                    ..inner
                },
                format!("/ {s}"),
                p.fg,
            );
            if inner.height > 2 {
                row_buttons(
                    f,
                    a,
                    Rect {
                        y: inner.bottom() - 1,
                        height: 1,
                        ..inner
                    },
                    vec![
                        (
                            ltext!(lang, "[Enter 查询]", "[Enter Search]"),
                            Action::Apply,
                        ),
                        (ltext!(lang, "[Esc 取消]", "[Esc Cancel]"), Action::Cancel),
                    ],
                    p,
                );
            }
        }
        Modal::Settings(selected) => {
            let selected = *selected;
            let c = &a.config;
            let on = |v| {
                if v {
                    ltext!(lang, "开启", "On")
                } else {
                    ltext!(lang, "关闭", "Off")
                }
            };
            let rows = [
                lformat!(
                    lang,
                    "界面布局       {}",
                    "Layout         {}",
                    c.layout.name(lang)
                ),
                lformat!(
                    lang,
                    "配色主题       {}",
                    "Theme          {}",
                    c.theme.name(lang)
                ),
                lformat!(lang, "极简模式       {}", "Minimal mode   {}", on(c.simple)),
                lformat!(
                    lang,
                    "启动行为       {}",
                    "Startup        {}",
                    if c.startup_autoplay {
                        ltext!(lang, "恢复电台，自动播放", "Restore station and play")
                    } else {
                        ltext!(
                            lang,
                            "恢复电台，不自动播放",
                            "Restore station, playback stopped"
                        )
                    }
                ),
                lformat!(
                    lang,
                    "真实频谱       {}",
                    "Live spectrum  {}",
                    on(c.spectrum)
                ),
                lformat!(
                    lang,
                    "代理设置       {}",
                    "Proxy          {}",
                    if c.proxy.is_empty() {
                        ltext!(lang, "直连", "Direct")
                    } else {
                        ltext!(lang, "已配置", "Configured")
                    }
                ),
                ltext!(
                    lang,
                    "自定义电台 / 添加、编辑和删除",
                    "Custom stations / Add, edit and delete"
                )
                .into(),
                ltext!(
                    lang,
                    "添加网络或本地 M3U / PLS",
                    "Add online or local M3U / PLS"
                )
                .into(),
                ltext!(
                    lang,
                    "切换目录源 / 编辑和删除订阅",
                    "Switch source / Edit and delete subscriptions"
                )
                .into(),
                ltext!(
                    lang,
                    "清除目录和搜索缓存（上限 10 MB）",
                    "Clear directory and search cache (10 MB limit)"
                )
                .into(),
                lformat!(
                    lang,
                    "语言 / Language   {}",
                    "Language       {}",
                    if lang == crate::i18n::Language::English {
                        "English"
                    } else {
                        "简体中文"
                    }
                ),
            ];
            let mut rows = rows.to_vec();
            rows.push(lformat!(
                lang,
                "排序           {}",
                "Sort           {}",
                c.sort.name(lang)
            ));
            rows.push(
                ltext!(
                    lang,
                    "电台别名 / 编辑当前选中电台",
                    "Station alias / Edit selected station"
                )
                .into(),
            );
            let visible = inner.height.saturating_sub(2).max(1) as usize;
            let first = selected.saturating_sub(visible - 1);
            for (i, label) in rows.iter().enumerate().skip(first).take(visible) {
                let row = Rect {
                    y: inner.y + (i - first) as u16,
                    height: 1,
                    ..inner
                };
                text(
                    f,
                    row,
                    format!(
                        "{} [{}] {label}",
                        if selected == i { "›" } else { " " },
                        if i == 10 {
                            "l".into()
                        } else if i == 11 {
                            "o".into()
                        } else if i == 12 {
                            "a".into()
                        } else {
                            ((i + 1) % 10).to_string()
                        }
                    ),
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
                    lformat!(lang, "配置：{}", "Config: {}", a.path.display()),
                    p.dim,
                );
                text(
                    f,
                    Rect {
                        y: inner.bottom() - 1,
                        height: 1,
                        ..inner
                    },
                    ltext!(
                        lang,
                        "修改立即生效并保存；代理凭据仅保存在本地。",
                        "Changes apply and save immediately; proxy credentials stay local."
                    ),
                    p.dim,
                )
            }
        }
        Modal::Form(form) => {
            let labels: Vec<&str> = match form.kind {
                FormKind::Alias => vec![ltext!(
                    lang,
                    "别名（留空恢复原名）",
                    "Alias (empty = original name)"
                )],
                FormKind::Proxy => vec![ltext!(
                    lang,
                    "代理 URL（空 = 直连；明文显示）",
                    "Proxy URL (empty = direct; shown in full)"
                )],
                FormKind::Station => vec![
                    ltext!(lang, "电台名称", "Station name"),
                    ltext!(lang, "主线路 HTTP / HTTPS", "Primary HTTP / HTTPS URL"),
                    ltext!(lang, "备用线路（可选）", "Backup URL (optional)"),
                ],
                FormKind::Subscription => vec![
                    ltext!(lang, "订阅名称", "Subscription name"),
                    ltext!(
                        lang,
                        "HTTP(S) 地址或本地文件完整路径",
                        "HTTP(S) URL or full local file path"
                    ),
                ],
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
                let value = &form.fields[i];
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
                    crate::i18n::message(lang, &form.error),
                    Color::LightRed,
                );
                text(
                    f,
                    Rect {
                        y: inner.bottom() - 2,
                        height: 1,
                        ..inner
                    },
                    ltext!(
                        lang,
                        "Tab/↑↓ 切换字段 · Ctrl+u 清空当前字段",
                        "Tab/↑↓ Change field · Ctrl+u Clear field"
                    ),
                    p.dim,
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
                        (ltext!(lang, "[Enter 应用]", "[Enter Apply]"), Action::Apply),
                        (ltext!(lang, "[Esc 返回]", "[Esc Back]"), Action::Cancel),
                    ],
                    p,
                )
            }
        }
        Modal::Sources => {
            let names = directory::source_names(&a.config);
            let visible = inner.height.saturating_sub(2).max(1) as usize;
            let first = a.menu_cursor.saturating_sub(visible - 1);
            for (i, name) in names.iter().enumerate().skip(first).take(visible) {
                let row = Rect {
                    y: inner.y + (i - first) as u16,
                    height: 1,
                    ..inner
                };
                let remove_width = if i >= 4 { 16.min(row.width) } else { 0 };
                let select_row = Rect {
                    width: row.width.saturating_sub(remove_width),
                    ..row
                };
                let label = format!(
                    "{} {}{}",
                    if a.menu_cursor == i { "›" } else { " " },
                    name,
                    if a.config.source == i {
                        ltext!(lang, " [当前]", " [Active]")
                    } else {
                        ""
                    }
                );
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
                        ltext!(lang, "[e 编辑]", "[e Edit]"),
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
                        ltext!(lang, "[d 删除]", "[d Delete]"),
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
                ltext!(lang, "[a 添加订阅]", "[a Add subscription]"),
                Action::AddSource,
                p,
            );
        }
        Modal::Stations => {
            let names: Vec<_> = a
                .config
                .custom_stations
                .iter()
                .map(|s| a.config.station_name(s).to_string())
                .collect();
            let visible = inner.height.saturating_sub(2).max(1) as usize;
            let first = a.menu_cursor.saturating_sub(visible - 1);
            for (i, name) in names.iter().enumerate().skip(first).take(visible) {
                let row = Rect {
                    y: inner.y + (i - first) as u16,
                    height: 1,
                    ..inner
                };
                let width = row.width.saturating_sub(8);
                button(
                    f,
                    a,
                    Rect { width, ..row },
                    &lformat!(
                        lang,
                        "{} [e 编辑] {name}",
                        "{} [e Edit] {name}",
                        if i == a.menu_cursor { "›" } else { " " }
                    ),
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
                    ltext!(lang, "[d 删除]", "[d Delete]"),
                    Action::RemoveStation(i),
                    p,
                );
            }
            if names.is_empty() {
                text(
                    f,
                    inner,
                    ltext!(lang, "尚未添加自定义电台", "No custom stations yet"),
                    p.dim,
                );
            }
            button(
                f,
                a,
                Rect {
                    y: inner.bottom().saturating_sub(1),
                    height: 1,
                    ..inner
                },
                ltext!(lang, "[a 添加电台]", "[a Add station]"),
                Action::AddStation,
                p,
            );
        }
        Modal::ConfirmDelete {
            target, confirm, ..
        } => {
            let (question, explanation) = match target {
                DeleteTarget::Favorite(s) => (
                    lformat!(
                        lang,
                        "从收藏中移除「{}」？",
                        "Remove \"{}\" from favorites?",
                        a.config.station_name(s)
                    ),
                    ltext!(
                        lang,
                        "只移除收藏，不影响公共电台目录和当前播放。",
                        "Only removes the favorite. Public stations and playback are unaffected."
                    ),
                ),
                DeleteTarget::Custom(s) => (
                    lformat!(
                        lang,
                        "删除自定义电台「{}」？",
                        "Delete custom station \"{}\"?",
                        a.config.station_name(s)
                    ),
                    ltext!(
                        lang,
                        "只删除自定义列表中的条目，不会停止当前播放。",
                        "Only removes this custom entry. Current playback continues."
                    ),
                ),
                DeleteTarget::Cache => (
                    ltext!(
                        lang,
                        "清除目录和搜索缓存？",
                        "Clear directory and search cache?"
                    )
                    .into(),
                    ltext!(
                        lang,
                        "不影响收藏、配置和当前播放；旧的更新请求不会写回缓存。",
                        "Keeps favorites, settings and playback; pending refreshes cannot restore cache."
                    ),
                ),
                DeleteTarget::Subscription(s) => (
                    lformat!(
                        lang,
                        "删除订阅源「{}」？",
                        "Delete subscription \"{}\"?",
                        s.name
                    ),
                    ltext!(
                        lang,
                        "只移除该订阅，不会删除本地文件或远程电台。",
                        "Only removes the subscription. Local files and remote stations are kept."
                    ),
                ),
            };
            f.render_widget(
                Paragraph::new(question)
                    .wrap(Wrap { trim: false })
                    .style(Style::default().fg(p.fg)),
                Rect { height: 2, ..inner },
            );
            if inner.height > 3 {
                f.render_widget(
                    Paragraph::new(explanation)
                        .wrap(Wrap { trim: false })
                        .style(Style::default().fg(p.dim)),
                    Rect {
                        y: inner.y + 2,
                        height: 2,
                        ..inner
                    },
                );
            }
            let confirm = *confirm;
            let mut x = inner.x;
            for (label, action, selected) in [
                (
                    ltext!(lang, "[n 取消]", "[n Cancel]"),
                    Action::Cancel,
                    !confirm,
                ),
                (
                    ltext!(lang, "[y 确认删除]", "[y Delete]"),
                    Action::ConfirmDelete,
                    confirm,
                ),
            ] {
                let width =
                    (Line::from(label).width() as u16 + 2).min(inner.right().saturating_sub(x));
                if width == 0 {
                    break;
                }
                let area = Rect {
                    x,
                    y: inner.bottom().saturating_sub(1),
                    width,
                    height: 1,
                };
                let style = if selected {
                    Style::default().fg(p.bg).bg(p.accent)
                } else {
                    Style::default().fg(p.accent)
                };
                f.render_widget(Paragraph::new(label).style(style), area);
                a.hits.push((area, action));
                x += width;
            }
        }
        Modal::Help => {
            let help = ltext!(
                lang,
                "主界面\nAlt+↑/↓ 或 [ ] 移动电台（手动排序）\n↑↓ / j k 选台 · Enter 播放 · 空格 播放/停止\np 设置 · / 搜索 · f 收藏/移除 · g 收藏夹\na 别名 · o 排序 · s 切源 · b 源管理 · r 换线 · m 极简 · u 强制刷新\nn 添加电台 · d/Delete 删除 · + - 音量 · E / Shift+E 切换音效 · q 退出\n\n设置：↑↓ 或 1…9 / 0 选择，Enter 修改，l 切换语言，o 排序，a 别名\n目录源：↑↓选择，Enter 切换，a 添加，e 编辑，d 删除\n自定义电台：↑↓选择，Enter/e 编辑，a 添加，d 删除\n输入框：Tab/Shift+Tab 切换，Ctrl+u 清空，Enter 应用\n删除确认：y 确认，n/Esc 取消，Tab 切换，Enter 执行所选\n所有二级页面：Esc 返回，保留上级选中项\nF1 打开帮助，Esc 返回原页面",
                "Main screen\nAlt+↑/↓ or [ ] Move station (manual order)\n↑↓ / j k Select · Enter Play · Space Play/Stop\np Settings · / Search · f Favorite/Remove · g Favorites\na Alias · o Sort · s Source · b Manage sources · r Route · m Minimal · u Refresh\nn Add station · d/Delete Delete · + - Volume · E / Shift+E Cycle effects · q Quit\n\nSettings: ↑↓ or 1…9 / 0 Select, Enter Change, l Language, o Sort, a Alias\nSources: ↑↓ Select, Enter Switch, a Add, e Edit, d Delete\nCustom stations: ↑↓ Select, Enter/e Edit, a Add, d Delete\nForms: Tab/Shift+Tab Focus, Ctrl+u Clear, Enter Apply\nConfirm: y Delete, n/Esc Cancel, Tab Focus, Enter Activate\nSubmenus: Esc Back, preserves parent selection\nF1 Help, Esc Return to previous screen"
            );
            f.render_widget(
                Paragraph::new(help)
                    .scroll((a.menu_cursor as u16, 0))
                    .wrap(Wrap { trim: false })
                    .style(Style::default().fg(p.fg)),
                inner,
            );
        }
        Modal::None => {}
    }
    if !matches!(a.modal, Modal::Form(_)) {
        button(
            f,
            a,
            Rect {
                x: r.right().saturating_sub(11),
                y: r.y,
                width: 10.min(r.width),
                height: 1,
            },
            ltext!(lang, "[Esc 返回]", "[Esc Back]"),
            Action::Cancel,
            p,
        );
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
    fn effect_picker_supports_mouse_cycle_and_saved_selection() {
        use crossterm::event::{
            KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut a = App::new(Config::default(), path.clone());
        let mut terminal = Terminal::new(TestBackend::new(30, 10)).unwrap();
        let click = |a: &mut App, target: Action| {
            let (r, _) = a.hits.iter().find(|(_, action)| *action == target).unwrap();
            a.mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: r.x,
                row: r.y,
                modifiers: KeyModifiers::NONE,
            });
        };
        terminal.draw(|f| draw(f, &mut a)).unwrap();
        click(&mut a, Action::ToggleEffects);
        terminal.draw(|f| draw(f, &mut a)).unwrap();
        click(&mut a, Action::SetEffect(Effect::Livehouse));
        assert!(a.effect_menu.is_none());
        assert_eq!(a.config.audio_effect, Effect::Livehouse);

        a.key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
        assert_eq!(a.config.audio_effect, Effect::Space3D);
        a.key(KeyEvent::new(KeyCode::Char('E'), KeyModifiers::SHIFT));
        assert_eq!(a.config.audio_effect, Effect::Livehouse);

        a.save().unwrap();
        let saved = std::fs::read_to_string(path).unwrap();
        assert_eq!(
            toml::from_str::<Config>(&saved).unwrap().audio_effect,
            Effect::Livehouse
        );
    }
    #[test]
    fn owned_lists_offer_delete_and_confirmation_blocks_background_clicks() {
        for layout in Screen::ALL {
            for simple in [true, false] {
                let station = Station {
                    id: "test".into(),
                    name: "TestStation".into(),
                    urls: vec!["https://example.com/live".into()],
                    ..Default::default()
                };
                let mut a = App::new(
                    Config {
                        source: 2,
                        layout: layout.clone(),
                        simple,
                        favorites: vec![station.clone()],
                        ..Default::default()
                    },
                    "unused".into(),
                );
                a.stations = vec![station];
                let mut t = Terminal::new(TestBackend::new(90, 30)).unwrap();
                t.draw(|f| draw(f, &mut a)).unwrap();
                assert!(
                    a.hits
                        .iter()
                        .any(|(_, action)| *action == Action::DeleteSelected)
                );
                a.action(Action::DeleteSelected);
                t.draw(|f| draw(f, &mut a)).unwrap();
                assert!(
                    a.hits.iter().all(|(_, action)| matches!(
                        action,
                        Action::Cancel | Action::ConfirmDelete
                    ))
                );
                let screen: String = t
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .map(|c| c.symbol())
                    .collect();
                assert!(screen.contains("TestStation"));
            }
        }
    }
    #[test]
    fn language_setting_switches_immediately_and_preserves_station_names() {
        use crate::i18n::Language;
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("config.toml");
        let mut a = App::new(Config::default(), path.clone());
        a.stations = vec![Station {
            name: "中国之声".into(),
            ..Default::default()
        }];
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let render = |a: &mut App, terminal: &mut Terminal<TestBackend>| {
            let mut screen = String::new();
            terminal
                .draw(|f| {
                    draw(f, a);
                    screen = f.buffer_mut().content.iter().map(|c| c.symbol()).collect();
                })
                .unwrap();
            screen
        };
        let english = render(&mut a, &mut terminal);
        assert!(english.contains("Search") && english.replace(' ', "").contains("中国之声"));
        a.action(Action::Settings);
        let settings = render(&mut a, &mut terminal);
        assert!(settings.contains("[l] Language") && settings.contains("English"));
        a.key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
        assert_eq!(a.config.language, Language::Chinese);
        let switched = render(&mut a, &mut terminal);
        assert!(
            switched.replace(' ', "").contains("语言/Language"),
            "{switched}"
        );
        a.save().unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.language, Language::Chinese);
        a.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        let chinese = render(&mut a, &mut terminal);
        assert!(
            chinese.replace(' ', "").contains("搜索")
                && chinese.replace(' ', "").contains("中国之声")
        );
        a.action(Action::Settings);
        a.action(Action::SettingsRow(10));
        assert_eq!(a.config.language, Language::English);
        assert!(render(&mut a, &mut terminal).contains("English"));
    }
    #[test]
    fn english_generated_ui_has_no_untranslated_chinese_across_layouts_and_menus() {
        use crate::app::{Form, FormKind};
        let mut a = App::new(Config::default(), "unused.toml".into());
        a.config.simple = false;
        let mut t = Terminal::new(TestBackend::new(100, 32)).unwrap();
        for layout in Screen::ALL {
            a.config.layout = layout;
            for modal in [
                Modal::None,
                Modal::Settings(10),
                Modal::Sources,
                Modal::Stations,
                Modal::Help,
                Modal::Search(String::new()),
                Modal::Form(Form {
                    kind: FormKind::Station,
                    fields: vec![String::new(); 3],
                    focus: 0,
                    edit: None,
                    station_id: None,
                    error: "请填写电台名称".into(),
                }),
            ] {
                a.modal = modal;
                t.draw(|f| draw(f, &mut a)).unwrap();
                let screen: String = t
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .map(|c| c.symbol())
                    .collect();
                assert!(
                    !screen
                        .chars()
                        .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)),
                    "{:?}: {screen}",
                    a.config.layout
                );
                assert!(
                    a.hits
                        .iter()
                        .all(|(r, _)| r.right() <= 100 && r.bottom() <= 32)
                );
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
                .all(|(_, action)| matches!(action, Action::SettingsRow(_) | Action::Cancel))
        );
    }
    #[test]
    fn station_aliases_and_mouse_tools_are_available_in_every_layout() {
        for layout in Screen::ALL {
            for simple in [true, false] {
                let mut a = App::new(
                    Config {
                        layout: layout.clone(),
                        simple,
                        ..Default::default()
                    },
                    "unused".into(),
                );
                a.config.aliases.insert("one".into(), "AliasExample".into());
                a.stations = vec![Station {
                    id: "one".into(),
                    name: "OriginalExample".into(),
                    ..Default::default()
                }];
                let mut t = Terminal::new(TestBackend::new(120, 38)).unwrap();
                t.draw(|f| draw(f, &mut a)).unwrap();
                let screen: String = t
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .map(|c| c.symbol())
                    .collect();
                assert!(screen.contains("AliasExample"));
                assert!(!screen.contains("OriginalExample"));
                for expected in [
                    Action::Alias,
                    Action::Sort,
                    Action::MoveStation(-1),
                    Action::MoveStation(1),
                ] {
                    assert!(a.hits.iter().any(|(_, action)| action == &expected));
                }
            }
        }
    }
}
