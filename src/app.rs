use crate::{
    audio::{AudioEvent, Player},
    config::{Config, Layout, Station, Subscription, Theme, stream_url, validate_proxy},
    directory,
};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use std::{
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Play,
    Stop,
    Simple,
    Settings,
    Search,
    Favorite,
    Favorites,
    Source(usize),
    Station(usize),
    Volume(u8),
    Route,
    SettingsRow(usize),
    FormField(usize),
    Apply,
    Cancel,
    AddStation,
    AddSource,
    RemoveSource(usize),
    EditSource(usize),
    EditStation(usize),
    RemoveStation(usize),
    DeleteSelected,
    ConfirmDelete,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormKind {
    Proxy,
    Station,
    Subscription,
}
pub struct Form {
    pub edit: Option<usize>,
    pub kind: FormKind,
    pub fields: Vec<String>,
    pub focus: usize,
    pub error: String,
}
#[derive(Clone)]
pub enum DeleteTarget {
    Favorite(Station),
    Custom(Station),
    Subscription(Subscription),
}
pub enum Modal {
    None,
    Search(String),
    Settings(usize),
    Form(Form),
    Sources,
    Stations,
    ConfirmDelete {
        target: DeleteTarget,
        previous: Box<Modal>,
        confirm: bool,
    },
}
struct DirectoryEvent {
    generation: u64,
    result: Result<Vec<Station>, String>,
}
pub struct App {
    pub config: Config,
    pub path: PathBuf,
    pub stations: Vec<Station>,
    pub selected: usize,
    pub route: usize,
    pub player: Player,
    pub status: String,
    pub playing: bool,
    pub connecting: bool,
    pub loading: bool,
    pub query: String,
    pub modal: Modal,
    pub logs: Vec<String>,
    pub hits: Vec<(Rect, Action)>,
    pub quit: bool,
    directory_tx: mpsc::Sender<DirectoryEvent>,
    directory_rx: mpsc::Receiver<DirectoryEvent>,
    directory_generation: u64,
    audio_tx: mpsc::Sender<AudioEvent>,
    audio_rx: mpsc::Receiver<AudioEvent>,
    dirty: bool,
    last_save: Instant,
    last_click: Option<(usize, Instant)>,
    selection_explicit: bool,
    directory_threads: Vec<thread::JoinHandle<()>>,
}
impl App {
    pub fn new(mut config: Config, path: PathBuf) -> Self {
        if config.source >= directory::source_names(&config).len() {
            config.source = 0
        }
        let (directory_tx, directory_rx) = mpsc::channel();
        let (audio_tx, audio_rx) = mpsc::channel();
        let volume = config.volume;
        let route = config.last_route;
        Self {
            config,
            path,
            stations: vec![],
            selected: 0,
            route,
            player: Player::new(volume),
            status: "就绪".into(),
            playing: false,
            connecting: false,
            loading: false,
            query: String::new(),
            modal: Modal::None,
            logs: vec!["已加载配置与上次会话".into()],
            hits: vec![],
            quit: false,
            directory_tx,
            directory_rx,
            directory_generation: 0,
            audio_tx,
            audio_rx,
            dirty: false,
            last_save: Instant::now(),
            last_click: None,
            selection_explicit: false,
            directory_threads: vec![],
        }
    }
    pub fn init(&mut self) {
        self.refresh();
    }
    pub fn note(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.status = message.clone();
        self.logs.push(message);
        if self.logs.len() > 50 {
            self.logs.remove(0);
        }
    }
    pub fn refresh(&mut self) {
        self.directory_threads.retain(|t| !t.is_finished());
        self.directory_generation += 1;
        let generation = self.directory_generation;
        let config = self.config.clone();
        let query = self.query.clone();
        let tx = self.directory_tx.clone();
        self.loading = true;
        self.note("正在加载电台目录…");
        self.directory_threads.push(thread::spawn(move || {
            let result = directory::fetch(&config, &query)
                .map_err(|_| "目录加载失败，请检查网络/订阅地址，或手动换源".into());
            let _ = tx.send(DirectoryEvent { generation, result });
        }));
    }
    pub fn tick(&mut self) -> bool {
        let mut changed = false;
        while let Ok(e) = self.directory_rx.try_recv() {
            if e.generation != self.directory_generation {
                continue;
            }
            changed = true;
            self.loading = false;
            match e.result {
                Ok(s) => {
                    self.stations = s;
                    self.selected = self
                        .config
                        .last_station
                        .as_ref()
                        .and_then(|last| self.stations.iter().position(|s| s.id == last.id))
                        .unwrap_or(0);
                    self.route = self
                        .selected_station()
                        .filter(|s| {
                            self.config
                                .last_station
                                .as_ref()
                                .is_some_and(|l| l.id == s.id)
                        })
                        .map(|s| self.config.last_route.min(s.urls.len().saturating_sub(1)))
                        .unwrap_or(0);
                    self.note(format!("已加载 {} 个电台", self.stations.len()));
                }
                Err(e) => self.note(e),
            }
        }
        while let Ok(e) = self.audio_rx.try_recv() {
            if e.generation != self.player.generation {
                continue;
            }
            changed = true;
            self.connecting = false;
            match e.result {
                Ok(m) => {
                    self.playing = true;
                    self.note(m)
                }
                Err(e) => {
                    self.playing = false;
                    self.note(format!("播放停止：{e} [Enter 重试 / R 换线 / S 换源]"));
                }
            }
        }
        if self.dirty
            && self.last_save.elapsed() > Duration::from_millis(500)
            && let Err(e) = self.save()
        {
            self.note(format!("配置保存失败：{e}"));
            self.dirty = false;
            changed = true;
        }
        changed
    }
    pub fn save(&mut self) -> Result<()> {
        self.config.save(&self.path)?;
        self.dirty = false;
        self.last_save = Instant::now();
        Ok(())
    }
    pub fn selected_station(&self) -> Option<&Station> {
        self.stations.get(self.selected)
    }
    pub fn active_station(&self) -> Option<&Station> {
        self.config.last_station.as_ref()
    }
    fn playback_selection(&self) -> Option<Station> {
        if self.selection_explicit {
            self.selected_station().cloned()
        } else {
            self.config
                .last_station
                .clone()
                .or_else(|| self.selected_station().cloned())
        }
    }
    fn playback_route(&self) -> usize {
        if self.selection_explicit {
            self.route
        } else {
            self.config.last_route
        }
    }
    pub fn start(&mut self, station: Station, route: usize) {
        if station.urls.is_empty() {
            self.note("电台没有播放地址");
            return;
        }
        let route = route.min(station.urls.len() - 1);
        self.config.last_station = Some(station.clone());
        self.config.last_route = route;
        self.dirty = true;
        self.connecting = true;
        self.playing = false;
        self.player.play(
            station.urls[route].clone(),
            self.config.proxy.clone(),
            self.audio_tx.clone(),
        );
        self.note(format!("正在连接 {} / 线路 {}…", station.name, route + 1));
    }
    fn change_volume(&mut self, value: u8) {
        self.config.volume = value.min(100);
        self.player.volume(self.config.volume);
        self.dirty = true;
    }
    pub fn action(&mut self, action: Action) {
        match action {
            Action::Play => {
                if self.playing || self.connecting {
                    self.player.stop();
                    self.playing = false;
                    self.connecting = false;
                    self.note("已停止")
                } else if let Some(s) = self.playback_selection() {
                    self.start(s, self.playback_route())
                } else {
                    self.note("先选择或添加一个电台")
                }
            }
            Action::Stop => {
                self.player.stop();
                self.playing = false;
                self.connecting = false;
                self.note("已停止")
            }
            Action::Simple => {
                self.config.simple = !self.config.simple;
                self.dirty = true
            }
            Action::Settings => self.modal = Modal::Settings(0),
            Action::Search => self.modal = Modal::Search(self.query.clone()),
            Action::Source(i) => {
                if matches!(self.modal, Modal::Sources) {
                    self.modal = Modal::None;
                }
                self.selection_explicit = true;
                if !self.loading || i != self.config.source {
                    self.config.source = i;
                    self.selected = 0;
                    self.route = 0;
                    self.query.clear();
                    self.stations.clear();
                    self.dirty = true;
                    self.refresh();
                }
            }
            Action::Station(i) => {
                self.selection_explicit = true;
                self.selected = i;
                self.route = if self
                    .selected_station()
                    .zip(self.config.last_station.as_ref())
                    .is_some_and(|(s, l)| s.id == l.id)
                {
                    self.config.last_route
                } else {
                    0
                };
            }
            Action::Favorite => {
                if let Some(s) = self.playback_selection() {
                    if self.config.favorites.iter().any(|f| f.id == s.id) {
                        self.request_delete(DeleteTarget::Favorite(s));
                        return;
                    } else {
                        self.config.favorites.push(s);
                        self.note("已收藏")
                    }
                    self.dirty = true;
                    if self.config.source == 2 {
                        self.refresh()
                    }
                }
            }
            Action::Favorites => self.action(Action::Source(2)),
            Action::Volume(v) => self.change_volume(v),
            Action::Route => {
                if let Some(s) = self.playback_selection() {
                    self.route = (self.route + 1) % s.urls.len().max(1);
                    let replay = self.playing || self.connecting;
                    if replay {
                        self.start(s, self.playback_route())
                    } else {
                        if self
                            .config
                            .last_station
                            .as_ref()
                            .is_some_and(|l| l.id == s.id)
                        {
                            self.config.last_route = self.route;
                            self.dirty = true
                        }
                        self.note(format!("已选择线路 {} / {}", self.route + 1, s.urls.len()))
                    }
                }
            }
            Action::SettingsRow(i) => self.settings_action(i),
            Action::FormField(i) => {
                if let Modal::Form(f) = &mut self.modal {
                    f.focus = i
                }
            }
            Action::Apply => self.apply_form(),
            Action::Cancel => self.cancel_modal(),
            Action::ConfirmDelete => self.confirm_delete(),
            Action::DeleteSelected => self.delete_selected(),
            Action::AddStation => {
                self.modal = Modal::Form(Form {
                    edit: None,
                    kind: FormKind::Station,
                    fields: vec![String::new(), String::new(), String::new()],
                    focus: 0,
                    error: String::new(),
                })
            }
            Action::AddSource => {
                self.modal = Modal::Form(Form {
                    edit: None,
                    kind: FormKind::Subscription,
                    fields: vec![String::new(), String::new()],
                    focus: 0,
                    error: String::new(),
                })
            }
            Action::EditSource(i) => {
                if let Some(sub) = self.config.subscriptions.get(i) {
                    self.modal = Modal::Form(Form {
                        edit: Some(i),
                        kind: FormKind::Subscription,
                        fields: vec![sub.name.clone(), sub.location.clone()],
                        focus: 0,
                        error: String::new(),
                    });
                }
            }
            Action::EditStation(i) => {
                if let Some(station) = self.config.custom_stations.get(i) {
                    self.modal = Modal::Form(Form {
                        edit: Some(i),
                        kind: FormKind::Station,
                        fields: vec![
                            station.name.clone(),
                            station.urls.first().cloned().unwrap_or_default(),
                            station.urls.get(1).cloned().unwrap_or_default(),
                        ],
                        focus: 0,
                        error: String::new(),
                    });
                }
            }
            Action::RemoveStation(i) => {
                if let Some(station) = self.config.custom_stations.get(i).cloned() {
                    self.request_delete(DeleteTarget::Custom(station));
                }
            }
            Action::RemoveSource(i) => {
                if let Some(source) = self.config.subscriptions.get(i).cloned() {
                    self.request_delete(DeleteTarget::Subscription(source));
                }
            }
        }
    }
    pub fn selected_is_favorite(&self) -> bool {
        self.playback_selection()
            .is_some_and(|s| self.config.favorites.iter().any(|f| f.id == s.id))
    }
    fn delete_selected(&mut self) {
        let Some(station) = self.selected_station().cloned() else {
            self.note("请先选择要删除的条目");
            return;
        };
        match self.config.source {
            2 => self.request_delete(DeleteTarget::Favorite(station)),
            3 => self.request_delete(DeleteTarget::Custom(station)),
            _ => self.note("公共目录不可删除，请在收藏或自定义列表中移除条目"),
        }
    }
    fn request_delete(&mut self, target: DeleteTarget) {
        if matches!(self.modal, Modal::ConfirmDelete { .. }) {
            return;
        }
        let previous = Box::new(std::mem::replace(&mut self.modal, Modal::None));
        self.modal = Modal::ConfirmDelete {
            target,
            previous,
            confirm: false,
        };
    }
    fn cancel_modal(&mut self) {
        let modal = std::mem::replace(&mut self.modal, Modal::None);
        if let Modal::ConfirmDelete { previous, .. } = modal {
            self.modal = *previous;
        }
    }
    fn confirm_delete(&mut self) {
        if !matches!(self.modal, Modal::ConfirmDelete { .. }) {
            return;
        }
        let Modal::ConfirmDelete {
            target, previous, ..
        } = std::mem::replace(&mut self.modal, Modal::None)
        else {
            return;
        };
        self.modal = *previous;
        let refresh_current = match target {
            DeleteTarget::Favorite(station) => {
                self.config.favorites.retain(|s| s.id != station.id);
                self.note(format!("已移除收藏：{}", station.name));
                self.config.source == 2
            }
            DeleteTarget::Custom(station) => {
                self.config.custom_stations.retain(|s| s.id != station.id);
                self.note(format!("已删除自定义电台：{}", station.name));
                self.config.source == 3
            }
            DeleteTarget::Subscription(source) => {
                let mut refresh = false;
                if let Some(i) = self
                    .config
                    .subscriptions
                    .iter()
                    .position(|s| s.name == source.name && s.location == source.location)
                {
                    self.config.subscriptions.remove(i);
                    if self.config.source == i + 4 {
                        self.config.source = 0;
                        refresh = true;
                    } else if self.config.source > i + 4 {
                        self.config.source -= 1;
                    }
                }
                self.note(format!("已删除订阅：{}", source.name));
                refresh
            }
        };
        self.dirty = true;
        if refresh_current {
            self.refresh();
        }
    }

    fn settings_action(&mut self, i: usize) {
        match i {
            0 => {
                let n = Layout::ALL
                    .iter()
                    .position(|v| v == &self.config.layout)
                    .unwrap();
                self.config.layout = Layout::ALL[(n + 1) % 5].clone()
            }
            1 => {
                let n = Theme::ALL
                    .iter()
                    .position(|v| v == &self.config.theme)
                    .unwrap();
                self.config.theme = Theme::ALL[(n + 1) % 5].clone()
            }
            2 => self.config.simple = !self.config.simple,
            3 => self.note("启动只恢复上次电台，需手动播放"),
            4 => self.config.spectrum = !self.config.spectrum,
            5 => {
                self.modal = Modal::Form(Form {
                    edit: None,
                    kind: FormKind::Proxy,
                    fields: vec![self.config.proxy.clone()],
                    focus: 0,
                    error: String::new(),
                });
                return;
            }
            6 => {
                self.modal = Modal::Stations;
                return;
            }
            7 => {
                self.action(Action::AddSource);
                return;
            }
            8 => {
                self.modal = Modal::Sources;
                return;
            }
            _ => {}
        }
        self.dirty = true;
    }
    fn apply_form(&mut self) {
        if let Modal::Search(query) = &self.modal {
            self.query = query.clone();
            self.selection_explicit = true;
            self.modal = Modal::None;
            self.refresh();
            return;
        }
        let Modal::Form(f) = &self.modal else { return };
        let kind = f.kind;
        let edit = f.edit;
        let fields = f.fields.clone();
        let result = (|| -> Result<()> {
            match kind {
                FormKind::Proxy => {
                    validate_proxy(fields[0].trim())?;
                    self.config.proxy = fields[0].trim().into();
                }
                FormKind::Station => {
                    anyhow::ensure!(!fields[0].trim().is_empty(), "请填写电台名称");
                    let mut urls = vec![stream_url(fields[1].trim())?.to_string()];
                    if !fields[2].trim().is_empty() {
                        urls.push(stream_url(fields[2].trim())?.to_string())
                    }
                    let s = Station {
                        id: format!("custom:{}", urls[0]),
                        name: fields[0].trim().into(),
                        urls,
                        ..Default::default()
                    };
                    if let Some(i) = edit {
                        let old_id = self.config.custom_stations[i].id.clone();
                        for favorite in &mut self.config.favorites {
                            if favorite.id == old_id {
                                *favorite = s.clone();
                            }
                        }
                        if self
                            .config
                            .last_station
                            .as_ref()
                            .is_some_and(|last| last.id == old_id)
                        {
                            self.config.last_station = Some(s.clone());
                        }
                        self.config.custom_stations[i] = s;
                    } else {
                        self.config.custom_stations.push(s);
                    }
                }
                FormKind::Subscription => {
                    anyhow::ensure!(
                        !fields[0].trim().is_empty() && !fields[1].trim().is_empty(),
                        "名称和订阅地址不能为空"
                    );
                    if fields[1].contains("://") {
                        stream_url(fields[1].trim())?;
                    }
                    let subscription = Subscription {
                        name: fields[0].trim().into(),
                        location: fields[1].trim().into(),
                    };
                    if let Some(i) = edit {
                        self.config.subscriptions[i] = subscription;
                    } else {
                        self.config.subscriptions.push(subscription);
                    }
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            if let Modal::Form(f) = &mut self.modal {
                f.error = e.to_string()
            }
            return;
        }
        self.modal = Modal::None;
        self.dirty = true;
        self.note("设置已应用");
        if kind == FormKind::Proxy {
            if (self.playing || self.connecting)
                && let Some(s) = self.config.last_station.clone()
            {
                self.start(s, self.config.last_route)
            }
            self.refresh()
        } else if kind == FormKind::Station {
            self.action(Action::Source(3));
        } else if kind == FormKind::Subscription && edit.is_some() {
            self.refresh();
        }
    }
    pub fn key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        match &mut self.modal {
            Modal::Search(text) => {
                match key.code {
                    KeyCode::Esc => self.modal = Modal::None,
                    KeyCode::Enter => {
                        self.selection_explicit = true;
                        self.query = text.clone();
                        self.modal = Modal::None;
                        self.refresh()
                    }
                    KeyCode::Backspace => {
                        text.pop();
                    }
                    KeyCode::Char(c) => {
                        text.push(c);
                    }
                    _ => {}
                }
                return;
            }
            Modal::Form(f) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('u') {
                    f.fields[f.focus].clear();
                    return;
                }
                match key.code {
                    KeyCode::Esc => self.modal = Modal::Settings(0),
                    KeyCode::Tab | KeyCode::Down => f.focus = (f.focus + 1) % f.fields.len(),
                    KeyCode::BackTab | KeyCode::Up => {
                        f.focus = (f.focus + f.fields.len() - 1) % f.fields.len()
                    }
                    KeyCode::Enter => self.apply_form(),
                    KeyCode::Backspace => {
                        f.fields[f.focus].pop();
                    }
                    KeyCode::Char(c) => {
                        f.fields[f.focus].push(c);
                    }
                    _ => {}
                }
                return;
            }
            Modal::Settings(i) => {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => self.modal = Modal::None,
                    KeyCode::Down | KeyCode::Char('j') => *i = (*i + 1) % 9,
                    KeyCode::Up | KeyCode::Char('k') => *i = (*i + 8) % 9,
                    KeyCode::Enter | KeyCode::Right => {
                        let row = *i;
                        self.settings_action(row)
                    }
                    _ => {}
                }
                return;
            }
            Modal::ConfirmDelete { confirm, .. } => {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('n') => self.cancel_modal(),
                    KeyCode::Tab | KeyCode::BackTab | KeyCode::Left | KeyCode::Right => {
                        *confirm = !*confirm
                    }
                    KeyCode::Char('y') => self.confirm_delete(),
                    KeyCode::Enter => {
                        if *confirm {
                            self.confirm_delete();
                        } else {
                            self.cancel_modal();
                        }
                    }
                    _ => {}
                }
                return;
            }
            Modal::Sources | Modal::Stations => {
                if key.code == KeyCode::Esc {
                    self.modal = Modal::None
                }
                return;
            }
            Modal::None => {}
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Down | KeyCode::Char('j') => self.action(Action::Station(
                (self.selected + 1).min(self.stations.len().saturating_sub(1)),
            )),
            KeyCode::Up | KeyCode::Char('k') => {
                self.action(Action::Station(self.selected.saturating_sub(1)))
            }
            KeyCode::Enter => {
                if let Some(s) = self.playback_selection() {
                    self.start(s, self.playback_route())
                }
            }
            KeyCode::Char(' ') => self.action(Action::Play),
            KeyCode::Char('/') => self.action(Action::Search),
            KeyCode::Char('m') => self.action(Action::Simple),
            KeyCode::Char('p') | KeyCode::Char(',') => self.action(Action::Settings),
            KeyCode::Char('f') => self.action(Action::Favorite),
            KeyCode::Delete | KeyCode::Char('d') => self.action(Action::DeleteSelected),
            KeyCode::Char('r') => self.action(Action::Route),
            KeyCode::Char('s') => self.action(Action::Source(
                (self.config.source + 1) % directory::source_names(&self.config).len(),
            )),
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.change_volume(self.config.volume.saturating_add(5))
            }
            KeyCode::Char('-') => self.change_volume(self.config.volume.saturating_sub(5)),
            KeyCode::Char('n') => self.action(Action::AddStation),
            KeyCode::Char('u') => self.refresh(),
            _ => {}
        }
    }
    pub fn mouse(&mut self, event: MouseEvent) {
        match event.kind {
            MouseEventKind::ScrollDown if matches!(self.modal, Modal::None) => self.action(
                Action::Station((self.selected + 1).min(self.stations.len().saturating_sub(1))),
            ),
            MouseEventKind::ScrollUp if matches!(self.modal, Modal::None) => {
                self.action(Action::Station(self.selected.saturating_sub(1)))
            }
            MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Drag(MouseButton::Left) => {
                if let Some((_, action)) = self
                    .hits
                    .iter()
                    .rev()
                    .find(|(r, _)| r.contains((event.column, event.row).into()))
                    .cloned()
                {
                    if matches!(event.kind, MouseEventKind::Drag(_))
                        && !matches!(action, Action::Volume(_))
                    {
                        return;
                    }
                    if let Action::Station(i) = action {
                        if self.last_click.is_some_and(|(last, time)| {
                            last == i && time.elapsed() < Duration::from_millis(400)
                        }) {
                            self.action(Action::Station(i));
                            if let Some(s) = self.selected_station().cloned() {
                                self.start(s, self.playback_route())
                            }
                            self.last_click = None;
                        } else {
                            self.action(Action::Station(i));
                            self.last_click = Some((i, Instant::now()));
                        }
                    } else {
                        self.action(action)
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};
    fn station(id: &str) -> Station {
        Station {
            id: id.into(),
            name: id.into(),
            urls: vec!["https://example.com/live".into()],
            ..Default::default()
        }
    }
    #[test]
    fn one_click_simple_preserves_layout_station_and_volume() {
        let c = Config {
            simple: false,
            layout: Layout::Btop,
            volume: 28,
            last_station: Some(station("remembered")),
            ..Default::default()
        };
        let mut a = App::new(c, "unused".into());
        let mut t = Terminal::new(TestBackend::new(100, 32)).unwrap();
        t.draw(|f| crate::ui::draw(f, &mut a)).unwrap();
        let rect = a
            .hits
            .iter()
            .find(|(_, act)| *act == Action::Simple)
            .unwrap()
            .0;
        a.mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: rect.x,
            row: rect.y,
            modifiers: KeyModifiers::NONE,
        });
        assert!(a.config.simple);
        assert_eq!(a.config.layout, Layout::Btop);
        assert_eq!(a.config.volume, 28);
        assert_eq!(a.config.last_station.unwrap().id, "remembered");
    }
    #[test]
    fn volume_and_settings_restore_on_next_start() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("session.toml");
        let mut a = App::new(Config::default(), path.clone());
        a.action(Action::Volume(37));
        a.action(Action::SettingsRow(0));
        a.action(Action::SettingsRow(1));
        a.action(Action::Simple);
        a.config.last_station = Some(station("last"));
        a.config.last_route = 0;
        a.save().unwrap();
        let b = App::new(Config::load(&path).unwrap(), path);
        assert_eq!(b.config.volume, 37);
        assert_eq!(b.config.layout, Layout::Receiver);
        assert_eq!(b.config.theme, Theme::Amber);
        assert!(!b.config.simple);
        assert_eq!(b.active_station().unwrap().id, "last");
        assert!(!b.playing);
    }
    #[test]
    fn invalid_proxy_does_not_replace_active_config() {
        let mut a = App::new(Config::default(), "unused".into());
        a.modal = Modal::Form(Form {
            edit: None,
            kind: FormKind::Proxy,
            fields: vec!["file:///secret".into()],
            focus: 0,
            error: String::new(),
        });
        a.action(Action::Apply);
        assert!(a.config.proxy.is_empty());
        assert!(matches!(&a.modal,Modal::Form(f)if !f.error.is_empty()));
    }
    #[test]
    fn text_input_does_not_trigger_global_shortcuts() {
        let mut a = App::new(Config::default(), "unused".into());
        a.modal = Modal::Search(String::new());
        a.key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(!a.quit);
        assert!(matches!(&a.modal,Modal::Search(s)if s=="q"));
    }
    #[test]
    fn restore_plays_last_station_even_if_absent_from_directory() {
        let c = Config {
            last_station: Some(station("remembered")),
            ..Default::default()
        };
        let mut a = App::new(c, "unused".into());
        a.stations = vec![station("other")];
        assert_eq!(a.playback_selection().unwrap().id, "remembered");
        a.action(Action::Station(0));
        assert_eq!(a.playback_selection().unwrap().id, "other");
    }
    #[test]
    fn editing_station_updates_favorites_and_last_session() {
        let s = station("old");
        let c = Config {
            custom_stations: vec![s.clone()],
            favorites: vec![s.clone()],
            last_station: Some(s),
            source: 3,
            ..Default::default()
        };
        let mut a = App::new(c, "unused".into());
        a.action(Action::EditStation(0));
        if let Modal::Form(f) = &mut a.modal {
            f.fields[0] = "Renamed".into();
            f.fields[1] = "https://example.com/new".into();
        }
        a.action(Action::Apply);
        assert_eq!(a.config.custom_stations.len(), 1);
        assert_eq!(a.config.favorites[0].name, "Renamed");
        assert_eq!(
            a.config.last_station.as_ref().unwrap().urls[0],
            "https://example.com/new"
        );
    }
    #[test]
    fn editing_subscription_replaces_instead_of_duplicating() {
        let mut a = App::new(
            Config {
                subscriptions: vec![Subscription {
                    name: "old".into(),
                    location: "https://example.com/old.m3u".into(),
                }],
                source: 3,
                ..Default::default()
            },
            "unused".into(),
        );
        a.action(Action::EditSource(0));
        if let Modal::Form(f) = &mut a.modal {
            f.fields[0] = "new".into();
            f.fields[1] = "/tmp/radio.m3u".into();
        }
        a.action(Action::Apply);
        assert_eq!(a.config.subscriptions.len(), 1);
        assert_eq!(a.config.subscriptions[0].name, "new");
    }
    #[test]
    fn selecting_station_is_not_playing_it() {
        let mut a = App::new(Config::default(), "unused".into());
        a.stations = vec![station("one"), station("two")];
        a.action(Action::Station(1));
        assert_eq!(a.selected, 1);
        assert!(!a.playing);
        assert!(a.config.last_station.is_none());
    }
}

#[cfg(test)]
mod deletion_regressions {
    use super::*;
    fn station() -> Station {
        Station {
            id: "one".into(),
            name: "中国之声".into(),
            urls: vec!["http://127.0.0.1:9/live".into()],
            ..Default::default()
        }
    }
    #[test]
    fn removing_custom_station_requires_confirmation() {
        let mut app = App::new(
            Config {
                custom_stations: vec![station()],
                ..Default::default()
            },
            "unused".into(),
        );
        app.action(Action::RemoveStation(0));
        assert_eq!(
            app.config.custom_stations.len(),
            1,
            "requesting deletion must not delete immediately"
        );
    }
    #[test]
    fn removing_favorite_requires_confirmation() {
        let s = station();
        let mut app = App::new(
            Config {
                favorites: vec![s.clone()],
                ..Default::default()
            },
            "unused".into(),
        );
        app.stations = vec![s];
        app.action(Action::Favorite);
        assert_eq!(
            app.config.favorites.len(),
            1,
            "unfavoriting must require confirmation too"
        );
    }
    #[test]
    fn enter_defaults_to_cancel_and_restores_management_view() {
        let mut app = App::new(
            Config {
                custom_stations: vec![station()],
                ..Default::default()
            },
            "unused".into(),
        );
        app.modal = Modal::Stations;
        app.action(Action::RemoveStation(0));
        app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.config.custom_stations.len(), 1);
        assert!(matches!(app.modal, Modal::Stations));
    }
    #[test]
    fn confirmed_custom_deletion_is_saved_and_only_removes_target() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut other = station();
        other.id = "two".into();
        other.name = "Keep me".into();
        let mut app = App::new(
            Config {
                source: 3,
                custom_stations: vec![station(), other],
                ..Default::default()
            },
            path.clone(),
        );
        app.action(Action::RemoveStation(0));
        app.action(Action::ConfirmDelete);
        app.save().unwrap();
        let saved = Config::load(&path).unwrap();
        assert_eq!(saved.custom_stations.len(), 1);
        assert_eq!(saved.custom_stations[0].id, "two");
    }
    #[test]
    fn favorite_removal_can_cancel_and_confirm() {
        let s = station();
        let mut app = App::new(
            Config {
                favorites: vec![s.clone()],
                ..Default::default()
            },
            "unused".into(),
        );
        app.stations = vec![s];
        app.action(Action::Favorite);
        app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.config.favorites.len(), 1);
        app.action(Action::Favorite);
        app.key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        assert!(app.config.favorites.is_empty());
    }
    #[test]
    fn deleting_subscription_requires_confirmation_and_keeps_local_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("stations.m3u");
        std::fs::write(&file, "#EXTM3U").unwrap();
        let mut app = App::new(
            Config {
                source: 3,
                subscriptions: vec![Subscription {
                    name: "local".into(),
                    location: file.display().to_string(),
                }],
                ..Default::default()
            },
            "unused".into(),
        );
        app.action(Action::RemoveSource(0));
        assert_eq!(app.config.subscriptions.len(), 1);
        app.action(Action::ConfirmDelete);
        assert!(app.config.subscriptions.is_empty());
        assert!(file.exists());
    }
    #[test]
    fn legacy_autoplay_does_not_start_audio_on_launch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path,"autoplay=true\nsource=3\n[last_station]\nid='one'\nname='中国之声'\nurls=['http://127.0.0.1:9/live']\n").unwrap();
        let mut app = App::new(Config::load(&path).unwrap(), path);
        app.init();
        assert!(
            !app.connecting && !app.playing,
            "launch must restore station without starting audio"
        );
        assert_eq!(app.active_station().unwrap().name, "中国之声");
    }
}
