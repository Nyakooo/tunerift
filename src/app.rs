use crate::{
    audio::{AudioEvent, Player},
    cache::{self, DirectoryCache},
    config::{Config, Layout, Station, Subscription, Theme, stream_url, validate_proxy},
    directory,
    effects::Effect,
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
    ToggleEffects,
    EffectMenuSurface,
    SetEffect(Effect),
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
    ForceRefresh,
    ClearCache,
    Help,
    ManageSources,
    Sort,
    Alias,
    MoveStation(isize),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormKind {
    Proxy,
    Station,
    Subscription,
    Alias,
}
pub struct Form {
    pub edit: Option<usize>,
    pub station_id: Option<String>,
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
    Cache,
}
pub enum Modal {
    None,
    Search(String),
    Settings(usize),
    Form(Form),
    Sources,
    Help,
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
    saved_at: u64,
    cache_written: bool,
}
pub struct App {
    pub config: Config,
    pub path: PathBuf,
    pub stations: Vec<Station>,
    directory_stations: Vec<Station>,
    pub selected: usize,
    pub route: usize,
    pub player: Player,
    pub status: String,
    pub playing: bool,
    pub connecting: bool,
    pub loading: bool,
    pub query: String,
    pub modal: Modal,
    pub effect_menu: Option<usize>,
    pub logs: Vec<String>,
    pub hits: Vec<(Rect, Action)>,
    pub quit: bool,
    pub directory_info: String,
    pub menu_cursor: usize,
    cache: DirectoryCache,
    showing_cache: bool,
    directory_identity: String,
    settings_cursor: usize,
    modal_stack: Vec<(Modal, usize)>,
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
        let cache = DirectoryCache::for_config(&path);
        let player = Player::new(config.volume);
        player.set_effect(config.audio_effect);
        let route = config.last_route;
        Self {
            config,
            path,
            stations: vec![],
            directory_stations: vec![],
            selected: 0,
            route,
            player,
            status: "就绪".into(),
            playing: false,
            connecting: false,
            loading: false,
            query: String::new(),
            modal: Modal::None,
            effect_menu: None,
            logs: vec!["已加载配置与上次会话".into()],
            hits: vec![],
            quit: false,
            directory_info: "目录尚未加载".into(),
            menu_cursor: 0,
            cache,
            showing_cache: false,
            directory_identity: String::new(),
            settings_cursor: 0,
            modal_stack: vec![],
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
        if self.config.startup_autoplay
            && let Some(station) = self.config.last_station.clone()
        {
            self.start(station, self.config.last_route);
        }
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
        self.load_directory(false);
    }
    pub fn force_refresh(&mut self) {
        self.load_directory(true);
    }
    fn install_stations(&mut self, stations: Vec<Station>) {
        self.directory_stations = stations;
        let mut stations = self.directory_stations.clone();
        self.config.sort.apply(
            &mut stations,
            &self.config.favorites,
            &self.config.aliases,
            self.config
                .station_order
                .get(&self.config.source_key())
                .map(Vec::as_slice)
                .unwrap_or_default(),
        );
        let old_id = self.selected_station().map(|s| s.id.clone());
        let preferred = old_id
            .as_deref()
            .or_else(|| self.config.last_station.as_ref().map(|s| s.id.as_str()));
        let selected = preferred
            .and_then(|id| stations.iter().position(|s| s.id == id))
            .unwrap_or(self.selected.min(stations.len().saturating_sub(1)));
        let old_route = self.route;
        self.stations = stations;
        self.selected = selected;
        self.route = self
            .selected_station()
            .map(|s| {
                let route = if old_id.as_ref() == Some(&s.id) {
                    old_route
                } else if self
                    .config
                    .last_station
                    .as_ref()
                    .is_some_and(|l| l.id == s.id)
                {
                    self.config.last_route
                } else {
                    0
                };
                route.min(s.urls.len().saturating_sub(1))
            })
            .unwrap_or(0);
    }
    fn load_directory(&mut self, force: bool) {
        self.directory_threads.retain(|t| !t.is_finished());
        let config = self.config.clone();
        let query = self.query.trim().to_string();
        let key = cache::key(&config, &query);
        let identity = key
            .as_ref()
            .map(|key| key.0.clone())
            .unwrap_or_else(|| format!("local:{}:{query}", config.source));
        if self.loading && key.is_some() && self.directory_identity == identity {
            if force {
                self.note("目录正在联网刷新，请稍候…");
            }
            return;
        }
        self.directory_generation += 1;
        let generation = self.directory_generation;
        if self.directory_identity != identity {
            self.stations.clear();
            self.directory_stations.clear();
            self.selected = 0;
            self.route = 0;
        }
        self.directory_identity = identity;
        self.showing_cache = false;
        if let Some(cached) = key.as_ref().and_then(|key| self.cache.read(key, &query)) {
            let fresh = cached.fresh;
            self.install_stations(cached.stations);
            self.showing_cache = true;
            self.directory_info = format!(
                "缓存{} · 更新于 {}",
                if fresh { "" } else { "（已过期）" },
                cache::timestamp(cached.saved_at)
            );
            if fresh && !force {
                self.loading = false;
                self.note(format!("缓存加载 {} 个电台", self.stations.len()));
                return;
            }
        } else {
            self.directory_info = if key.is_some() {
                "联网获取目录…"
            } else {
                "本地列表"
            }
            .into();
        }
        let tx = self.directory_tx.clone();
        let cache = self.cache.clone();
        let epoch = cache.epoch();
        self.loading = true;
        self.note(if self.showing_cache {
            "已显示缓存，正在联网更新…"
        } else {
            "正在加载电台目录…"
        });
        self.directory_threads.push(thread::spawn(move || {
            let result = directory::fetch(&config, &query)
                .map_err(|_| "目录加载失败，请检查网络/订阅地址，或手动换源".into());
            let cache_written = match (&key, &result) {
                (Some(key), Ok(stations)) => cache.write(key, stations, epoch).unwrap_or(false),
                _ => true,
            };
            let _ = tx.send(DirectoryEvent {
                generation,
                result,
                saved_at: cache::now(),
                cache_written,
            });
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
                Ok(stations) => {
                    self.install_stations(stations);
                    self.showing_cache = false;
                    self.directory_info = if cache::key(&self.config, &self.query).is_some() {
                        format!(
                            "联网目录 · 更新于 {}{}",
                            cache::timestamp(e.saved_at),
                            if e.cache_written {
                                ""
                            } else {
                                "（未写入缓存）"
                            }
                        )
                    } else {
                        "本地列表".into()
                    };
                    self.note(format!("已加载 {} 个电台", self.stations.len()));
                }
                Err(error) => {
                    if self.showing_cache {
                        self.note(format!("{error}；继续显示缓存，按 u 重试"));
                    } else {
                        self.directory_info = if self.stations.is_empty() {
                            "目录更新失败"
                        } else {
                            "内存目录 · 更新失败"
                        }
                        .into();
                        self.note(error);
                    }
                }
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
                .as_ref()
                .map(|last| {
                    self.stations
                        .iter()
                        .find(|station| station.id == last.id)
                        .unwrap_or(last)
                        .clone()
                })
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
    fn open_modal(&mut self, next: Modal) {
        self.effect_menu = None;
        let previous = std::mem::replace(&mut self.modal, Modal::None);
        if !matches!(previous, Modal::None) {
            self.modal_stack.push((previous, self.menu_cursor));
        }
        self.menu_cursor = if matches!(next, Modal::Sources) {
            self.config.source
        } else {
            0
        };
        self.modal = next;
    }
    fn close_all_modals(&mut self) {
        self.modal = Modal::None;
        self.modal_stack.clear();
    }
    pub fn action(&mut self, action: Action) {
        match action {
            Action::MoveStation(direction) => {
                let target = self.selected.saturating_add_signed(direction);
                if target >= self.stations.len() || target == self.selected {
                    return;
                }
                let mut visible: Vec<_> = self.stations.iter().map(|s| s.id.clone()).collect();
                visible.swap(self.selected, target);
                let key = self.config.source_key();
                let order = self.config.station_order.entry(key).or_default();
                let mut known: std::collections::HashSet<_> = order.iter().cloned().collect();
                for station in &self.directory_stations {
                    if known.insert(station.id.clone()) {
                        order.push(station.id.clone());
                    }
                }
                let visible_ids: std::collections::HashSet<_> =
                    visible.iter().map(String::as_str).collect();
                let mut positions = visible.iter();
                for id in order.iter_mut() {
                    if visible_ids.contains(id.as_str())
                        && let Some(next) = positions.next()
                    {
                        *id = next.clone();
                    }
                }
                self.config.sort = crate::config::SortOrder::Manual;
                self.install_stations(self.directory_stations.clone());
                self.dirty = true;
                self.note("已更新手动顺序");
            }
            Action::Alias => {
                if let Some(station) = self
                    .selected_station()
                    .or_else(|| self.active_station())
                    .cloned()
                {
                    let alias = self
                        .config
                        .aliases
                        .get(&station.id)
                        .cloned()
                        .unwrap_or_default();
                    self.open_modal(Modal::Form(Form {
                        edit: None,
                        station_id: Some(station.id),
                        kind: FormKind::Alias,
                        fields: vec![alias],
                        focus: 0,
                        error: String::new(),
                    }));
                }
            }
            Action::Sort => {
                self.config.sort = self.config.sort.next_order();
                self.install_stations(self.directory_stations.clone());
                self.dirty = true;
                self.note(format!(
                    "排序：{}",
                    self.config.sort.name(crate::i18n::Language::Chinese)
                ));
            }
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
            Action::Help => self.open_modal(Modal::Help),
            Action::ManageSources => self.open_modal(Modal::Sources),
            Action::Settings => self.open_modal(Modal::Settings(self.settings_cursor)),
            Action::Search => self.open_modal(Modal::Search(self.query.clone())),
            Action::Source(i) => {
                if matches!(self.modal, Modal::Sources) {
                    self.close_all_modals();
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
                    self.install_stations(self.directory_stations.clone());
                    if self.config.source == 2 {
                        self.refresh()
                    }
                }
            }
            Action::Favorites => self.action(Action::Source(2)),
            Action::Volume(v) => self.change_volume(v),
            Action::ToggleEffects => {
                self.effect_menu = if self.effect_menu.is_some() {
                    None
                } else {
                    Some(0)
                };
            }
            Action::EffectMenuSurface => {}
            Action::SetEffect(effect) => {
                self.effect_menu = None;
                self.player.set_effect(effect);
                self.config.audio_effect = effect;
                self.dirty = true;
            }
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
            Action::ForceRefresh => self.force_refresh(),
            Action::ClearCache => self.request_delete(DeleteTarget::Cache),
            Action::ConfirmDelete => self.confirm_delete(),
            Action::DeleteSelected => self.delete_selected(),
            Action::AddStation => {
                self.open_modal(Modal::Form(Form {
                    edit: None,
                    station_id: None,
                    kind: FormKind::Station,
                    fields: vec![String::new(), String::new(), String::new()],
                    focus: 0,
                    error: String::new(),
                }));
            }
            Action::AddSource => {
                self.open_modal(Modal::Form(Form {
                    edit: None,
                    station_id: None,
                    kind: FormKind::Subscription,
                    fields: vec![String::new(), String::new()],
                    focus: 0,
                    error: String::new(),
                }));
            }
            Action::EditSource(i) => {
                if matches!(self.modal, Modal::Sources) {
                    self.menu_cursor = i + 4;
                }
                if let Some(sub) = self.config.subscriptions.get(i) {
                    self.open_modal(Modal::Form(Form {
                        edit: Some(i),
                        station_id: None,
                        kind: FormKind::Subscription,
                        fields: vec![sub.name.clone(), sub.location.clone()],
                        focus: 0,
                        error: String::new(),
                    }));
                }
            }
            Action::EditStation(i) => {
                if matches!(self.modal, Modal::Stations) {
                    self.menu_cursor = i;
                }
                if let Some(station) = self.config.custom_stations.get(i) {
                    self.open_modal(Modal::Form(Form {
                        edit: Some(i),
                        station_id: None,
                        kind: FormKind::Station,
                        fields: vec![
                            station.name.clone(),
                            station.urls.first().cloned().unwrap_or_default(),
                            station.urls.get(1).cloned().unwrap_or_default(),
                        ],
                        focus: 0,
                        error: String::new(),
                    }));
                }
            }
            Action::RemoveStation(i) => {
                if matches!(self.modal, Modal::Stations) {
                    self.menu_cursor = i;
                }
                if let Some(station) = self.config.custom_stations.get(i).cloned() {
                    self.request_delete(DeleteTarget::Custom(station));
                }
            }
            Action::RemoveSource(i) => {
                if matches!(self.modal, Modal::Sources) {
                    self.menu_cursor = i + 4;
                }
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
        } else if let Some((parent, cursor)) = self.modal_stack.pop() {
            self.modal = parent;
            self.menu_cursor = cursor;
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
                if let Some(i) = self
                    .config
                    .favorites
                    .iter()
                    .position(|s| s.id == station.id)
                {
                    self.config.favorites.remove(i);
                }
                self.note(format!("已移除收藏：{}", station.name));
                self.config.source == 2
            }
            DeleteTarget::Custom(station) => {
                if let Some(i) = self
                    .config
                    .custom_stations
                    .iter()
                    .position(|s| s == &station)
                {
                    self.config.custom_stations.remove(i);
                }
                self.note(format!("已删除自定义电台：{}", station.name));
                self.config.source == 3
            }
            DeleteTarget::Cache => {
                match self.cache.clear() {
                    Ok(()) => {
                        self.directory_generation += 1;
                        self.loading = false;
                        self.showing_cache = false;
                        self.directory_info = "内存列表 · 磁盘缓存已清除".into();
                        self.note("目录缓存已清除，收藏与设置保持不变");
                    }
                    Err(_) => self.note("缓存清除失败，请检查缓存目录权限"),
                }
                false
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
        let count = match self.modal {
            Modal::Sources => directory::source_names(&self.config).len(),
            Modal::Stations => self.config.custom_stations.len(),
            _ => usize::MAX,
        };
        self.menu_cursor = self.menu_cursor.min(count.saturating_sub(1));
        self.dirty = true;
        if refresh_current {
            self.refresh();
        } else {
            self.install_stations(self.directory_stations.clone());
        }
    }

    fn settings_action(&mut self, i: usize) {
        self.settings_cursor = i;
        if matches!(self.modal, Modal::Settings(_)) {
            self.modal = Modal::Settings(i);
        }
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
            3 => self.config.startup_autoplay = !self.config.startup_autoplay,
            4 => self.config.spectrum = !self.config.spectrum,
            5 => {
                self.open_modal(Modal::Form(Form {
                    edit: None,
                    station_id: None,
                    kind: FormKind::Proxy,
                    fields: vec![self.config.proxy.clone()],
                    focus: 0,
                    error: String::new(),
                }));
                return;
            }
            6 => {
                self.open_modal(Modal::Stations);
                return;
            }
            7 => {
                self.action(Action::AddSource);
                return;
            }
            8 => {
                self.open_modal(Modal::Sources);
                return;
            }
            9 => {
                self.action(Action::ClearCache);
                return;
            }
            10 => self.config.language.toggle(),
            12 => {
                self.action(Action::Alias);
                return;
            }
            11 => {
                self.action(Action::Sort);
                return;
            }
            _ => {}
        }
        self.dirty = true;
    }
    fn apply_form(&mut self) {
        if let Modal::Search(query) = &self.modal {
            self.query = query.trim().to_string();
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
                FormKind::Alias => {
                    let id = f
                        .station_id
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("No station selected"))?;
                    let value = fields[0].trim();
                    if value.is_empty() {
                        self.config.aliases.remove(id);
                    } else {
                        self.config.aliases.insert(id.clone(), value.into());
                    }
                }
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
                        if old_id != s.id {
                            if let Some(alias) = self.config.aliases.remove(&old_id) {
                                self.config.aliases.insert(s.id.clone(), alias);
                            }
                            for order in self.config.station_order.values_mut() {
                                for id in order {
                                    if *id == old_id {
                                        *id = s.id.clone();
                                    }
                                }
                            }
                        }
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
        self.cancel_modal();
        self.dirty = true;
        self.note("设置已应用");
        if kind == FormKind::Alias {
            self.install_stations(self.directory_stations.clone());
        } else if kind == FormKind::Proxy {
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
        if self.effect_menu.take().is_some() && key.code == KeyCode::Esc {
            return;
        }
        if key.code == KeyCode::F(1) {
            if !matches!(self.modal, Modal::Help) {
                self.action(Action::Help);
            }
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        match &mut self.modal {
            Modal::Search(text) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('u') {
                    text.clear();
                    return;
                }
                match key.code {
                    KeyCode::Esc => self.cancel_modal(),
                    KeyCode::Enter => {
                        self.selection_explicit = true;
                        self.query = text.trim().to_string();
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
                    KeyCode::Esc => self.cancel_modal(),
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
                    KeyCode::Esc | KeyCode::Char('q') => self.cancel_modal(),
                    KeyCode::Down | KeyCode::Char('j') => {
                        *i = (*i + 1) % 13;
                        self.settings_cursor = *i;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        *i = (*i + 12) % 13;
                        self.settings_cursor = *i;
                    }
                    KeyCode::Char('l') => self.settings_action(10),
                    KeyCode::Char('o') => self.settings_action(11),
                    KeyCode::Char('a') => self.settings_action(12),
                    KeyCode::Char(c @ '0'..='9') => {
                        let row = if c == '0' {
                            9
                        } else {
                            c as usize - '1' as usize
                        };
                        self.settings_action(row);
                    }
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
            Modal::Help => {
                match key.code {
                    KeyCode::Esc => self.cancel_modal(),
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.menu_cursor = (self.menu_cursor + 1).min(40)
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.menu_cursor = self.menu_cursor.saturating_sub(1)
                    }
                    KeyCode::PageDown => self.menu_cursor = (self.menu_cursor + 8).min(40),
                    KeyCode::PageUp => self.menu_cursor = self.menu_cursor.saturating_sub(8),
                    KeyCode::Home => self.menu_cursor = 0,
                    _ => {}
                }
                return;
            }
            Modal::Sources | Modal::Stations => {
                let sources = matches!(self.modal, Modal::Sources);
                let count = if sources {
                    directory::source_names(&self.config).len()
                } else {
                    self.config.custom_stations.len()
                };
                match key.code {
                    KeyCode::Esc => self.cancel_modal(),
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.menu_cursor = (self.menu_cursor + 1).min(count.saturating_sub(1))
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.menu_cursor = self.menu_cursor.saturating_sub(1)
                    }
                    KeyCode::Char('a') => self.action(if sources {
                        Action::AddSource
                    } else {
                        Action::AddStation
                    }),
                    KeyCode::Enter if count > 0 => self.action(if sources {
                        Action::Source(self.menu_cursor)
                    } else {
                        Action::EditStation(self.menu_cursor)
                    }),
                    KeyCode::Char('e') if count > 0 => {
                        if sources {
                            if self.menu_cursor >= 4 {
                                self.action(Action::EditSource(self.menu_cursor - 4));
                            }
                        } else {
                            self.action(Action::EditStation(self.menu_cursor));
                        }
                    }
                    KeyCode::Delete | KeyCode::Char('d') if count > 0 => {
                        if sources {
                            if self.menu_cursor >= 4 {
                                self.action(Action::RemoveSource(self.menu_cursor - 4));
                            }
                        } else {
                            self.action(Action::RemoveStation(self.menu_cursor));
                        }
                    }
                    _ => {}
                }
                return;
            }
            Modal::None => {}
        }
        match key.code {
            KeyCode::Char('[') => self.action(Action::MoveStation(-1)),
            KeyCode::Char(']') => self.action(Action::MoveStation(1)),
            KeyCode::Up if key.modifiers.contains(KeyModifiers::ALT) => {
                self.action(Action::MoveStation(-1))
            }
            KeyCode::Down if key.modifiers.contains(KeyModifiers::ALT) => {
                self.action(Action::MoveStation(1))
            }
            KeyCode::Char('a') => self.action(Action::Alias),
            KeyCode::Char('o') => self.action(Action::Sort),
            KeyCode::Char('?') => self.action(Action::Help),
            KeyCode::Char('g') => self.action(Action::Favorites),
            KeyCode::Char('b') => self.action(Action::ManageSources),
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
            KeyCode::Char('e' | 'E')
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                let effects = Effect::SELECTABLE;
                let current = effects
                    .iter()
                    .position(|&effect| effect == self.config.audio_effect)
                    .unwrap_or(0);
                let backward =
                    key.code == KeyCode::Char('E') || key.modifiers.contains(KeyModifiers::SHIFT);
                let step = if backward { effects.len() - 1 } else { 1 };
                self.action(Action::SetEffect(effects[(current + step) % effects.len()]));
            }
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
            KeyCode::Char('u') => self.force_refresh(),
            _ => {}
        }
    }
    pub fn mouse(&mut self, event: MouseEvent) {
        if let Some(offset) = self.effect_menu.as_mut() {
            match event.kind {
                MouseEventKind::ScrollDown => {
                    *offset = (*offset + 1).min(Effect::SELECTABLE.len() - 1)
                }
                MouseEventKind::ScrollUp => *offset = offset.saturating_sub(1),
                MouseEventKind::Down(MouseButton::Left) => {
                    self.last_click = None;
                    let action = self
                        .hits
                        .iter()
                        .rev()
                        .find(|(r, _)| r.contains((event.column, event.row).into()))
                        .map(|(_, action)| action.clone());
                    match action {
                        Some(
                            action @ (Action::ToggleEffects
                            | Action::EffectMenuSurface
                            | Action::SetEffect(_)),
                        ) => self.action(action),
                        _ => self.effect_menu = None,
                    }
                }
                _ => {}
            }
            return;
        }
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
            station_id: None,
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
    fn restored_station_uses_refreshed_directory_address() {
        let old = station("remembered");
        let mut refreshed = old.clone();
        refreshed.urls = vec!["https://example.com/refreshed".into()];
        let mut a = App::new(
            Config {
                last_station: Some(old),
                ..Default::default()
            },
            "unused".into(),
        );
        a.stations = vec![refreshed.clone()];
        assert_eq!(a.playback_selection().unwrap().urls, refreshed.urls);
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
    fn deleting_one_custom_entry_does_not_remove_another_with_same_url() {
        let one = station();
        let mut two = one.clone();
        two.name = "Second preset".into();
        let mut app = App::new(
            Config {
                source: 3,
                custom_stations: vec![one, two],
                ..Default::default()
            },
            "unused".into(),
        );
        app.action(Action::RemoveStation(1));
        app.action(Action::ConfirmDelete);
        assert_eq!(app.config.custom_stations.len(), 1);
        assert_eq!(app.config.custom_stations[0].name, "中国之声");
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

#[cfg(test)]
mod cache_and_navigation_tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    fn station(name: &str, url: &str) -> Station {
        Station {
            id: url.into(),
            name: name.into(),
            urls: vec![url.into()],
            ..Default::default()
        }
    }
    fn keypress(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn server(status: &str, body: &str) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_string();
        let body = body.to_string();
        let worker = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = vec![];
            loop {
                let mut b = [0];
                socket.read_exact(&mut b).unwrap();
                request.push(b[0]);
                if request.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).unwrap();
        });
        (format!("http://{address}/list.m3u"), worker)
    }
    fn app_for_subscription(location: String, root: &std::path::Path) -> App {
        let config = Config {
            source: 4,
            subscriptions: vec![Subscription {
                name: "test".into(),
                location,
            }],
            ..Default::default()
        };
        let mut app = App::new(config, root.join("config.toml"));
        app.cache = DirectoryCache::new(root.join("cache"));
        app
    }
    fn finish_loading(app: &mut App) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while app.loading && Instant::now() < deadline {
            app.tick();
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            !app.loading,
            "directory request did not finish: {}",
            app.status
        );
    }
    #[test]
    fn fresh_cache_avoids_network_and_survives_app_restart() {
        let d = tempfile::tempdir().unwrap();
        let location = "http://unresolvable.invalid/list.m3u".into();
        let mut a = app_for_subscription(location, d.path());
        let key = cache::key(&a.config, "").unwrap();
        let saved = station("cached", "https://example.com/live");
        a.cache
            .write(&key, std::slice::from_ref(&saved), 0)
            .unwrap();
        a.refresh();
        assert!(!a.loading);
        assert!(a.directory_threads.is_empty());
        assert_eq!(a.stations, vec![saved.clone()]);
        let mut b = App::new(a.config.clone(), a.path.clone());
        b.cache = DirectoryCache::new(d.path().join("cache"));
        b.refresh();
        assert_eq!(b.stations, vec![saved]);
        assert!(b.directory_info.contains("缓存"));
    }
    #[test]
    fn force_refresh_bypasses_fresh_cache() {
        let d = tempfile::tempdir().unwrap();
        let (url, server) = server(
            "200 OK",
            "#EXTM3U\n#EXTINF:-1,New\nhttps://example.com/new\n",
        );
        let mut a = app_for_subscription(url, d.path());
        let key = cache::key(&a.config, "").unwrap();
        a.cache
            .write(&key, &[station("Old", "https://example.com/old")], 0)
            .unwrap();
        a.refresh();
        assert_eq!(a.stations[0].name, "Old");
        assert!(a.directory_threads.is_empty());
        a.action(Action::ForceRefresh);
        finish_loading(&mut a);
        server.join().unwrap();
        assert_eq!(a.stations[0].name, "New");
        assert!(a.directory_info.contains("联网"));
        assert_eq!(a.cache.read(&key, "").unwrap().stations[0].name, "New");
    }
    #[test]
    fn stale_cache_displays_immediately_and_preserves_selected_station_on_update() {
        let d = tempfile::tempdir().unwrap();
        let (url, server) = server(
            "200 OK",
            "#EXTM3U\n#EXTINF:-1,B updated\nhttps://example.com/b\n#EXTINF:-1,A\nhttps://example.com/a\n",
        );
        let mut a = app_for_subscription(url, d.path());
        let key = cache::key(&a.config, "").unwrap();
        a.cache
            .write_at(
                &key,
                &[
                    station("A", "https://example.com/a"),
                    station("B", "https://example.com/b"),
                ],
                0,
                cache::now() - 7200,
            )
            .unwrap();
        a.refresh();
        assert!(a.loading);
        assert_eq!(a.stations[1].name, "B");
        assert!(a.directory_info.contains("已过期"));
        a.action(Action::Station(1));
        finish_loading(&mut a);
        server.join().unwrap();
        assert_eq!(a.selected_station().unwrap().name, "B updated");
        assert_eq!(a.selected, 0);
    }
    #[test]
    fn failed_refresh_keeps_stale_cache() {
        let d = tempfile::tempdir().unwrap();
        let (url, server) = server("500 Internal Server Error", "");
        let mut a = app_for_subscription(url, d.path());
        let key = cache::key(&a.config, "").unwrap();
        let old = station("Old", "https://example.com/old");
        a.cache
            .write_at(&key, std::slice::from_ref(&old), 0, cache::now() - 7200)
            .unwrap();
        a.refresh();
        finish_loading(&mut a);
        server.join().unwrap();
        assert_eq!(a.stations, vec![old]);
        assert!(a.status.contains("继续显示缓存"));
        assert!(a.cache.read(&key, "").is_some());
    }
    #[test]
    fn local_lists_are_not_disk_cached() {
        let d = tempfile::tempdir().unwrap();
        let mut a = App::new(
            Config {
                source: 3,
                custom_stations: vec![station("local", "https://example.com/local")],
                ..Default::default()
            },
            d.path().join("config.toml"),
        );
        a.cache = DirectoryCache::new(d.path().join("cache"));
        a.refresh();
        finish_loading(&mut a);
        assert!(!a.cache.root.exists());
        assert_eq!(a.directory_info, "本地列表");
    }
    #[test]
    fn clear_cache_requires_confirmation_and_keeps_favorites() {
        let d = tempfile::tempdir().unwrap();
        let mut a = App::new(Config::default(), d.path().join("config.toml"));
        a.cache = DirectoryCache::new(d.path().join("cache"));
        let key = cache::key(&a.config, "").unwrap();
        let saved = station("saved", "https://example.com/live");
        a.cache
            .write(&key, std::slice::from_ref(&saved), 0)
            .unwrap();
        a.config.favorites.push(saved.clone());
        a.action(Action::Settings);
        a.action(Action::SettingsRow(9));
        assert!(a.cache.read(&key, "").is_some());
        a.key(keypress(KeyCode::Char('y')));
        assert!(a.cache.read(&key, "").is_none());
        assert_eq!(a.config.favorites, vec![saved]);
        assert!(matches!(a.modal, Modal::Settings(9)));
    }
    #[test]
    fn startup_setting_toggles_in_place_and_persists() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("config.toml");
        let mut a = App::new(Config::default(), path.clone());
        assert!(!a.config.startup_autoplay);
        a.action(Action::Settings);
        a.action(Action::SettingsRow(3));
        assert!(matches!(a.modal, Modal::Settings(3)));
        assert!(a.config.startup_autoplay);
        a.save().unwrap();
        assert!(Config::load(&path).unwrap().startup_autoplay);
        a.key(keypress(KeyCode::Enter));
        assert!(!a.config.startup_autoplay);
        assert!(matches!(a.modal, Modal::Settings(3)));
        a.save().unwrap();
        assert!(!Config::load(&path).unwrap().startup_autoplay);
        a.action(Action::Cancel);
        a.action(Action::Settings);
        assert!(matches!(a.modal, Modal::Settings(3)));
    }
    #[test]
    fn proxy_editor_returns_to_clicked_settings_row() {
        let mut a = App::new(Config::default(), "unused".into());
        a.action(Action::Settings);
        a.action(Action::SettingsRow(5));
        assert!(matches!(a.modal, Modal::Form(_)));
        a.key(keypress(KeyCode::Esc));
        assert!(matches!(a.modal, Modal::Settings(5)));
    }
    #[test]
    fn add_subscription_shortcut_and_nested_return_preserve_both_levels() {
        let c = Config {
            source: 2,
            subscriptions: vec![Subscription {
                name: "test".into(),
                location: "https://example.com/list.m3u".into(),
            }],
            ..Default::default()
        };
        let mut a = App::new(c, "unused".into());
        a.action(Action::Settings);
        a.action(Action::SettingsRow(8));
        a.menu_cursor = 4;
        a.key(keypress(KeyCode::Char('a')));
        assert!(matches!(&a.modal,Modal::Form(f)if f.kind==FormKind::Subscription));
        a.key(keypress(KeyCode::Esc));
        assert!(matches!(a.modal, Modal::Sources));
        assert_eq!(a.menu_cursor, 4);
        a.key(keypress(KeyCode::Esc));
        assert!(matches!(a.modal, Modal::Settings(8)));
    }
    #[test]
    fn station_manager_add_edit_delete_keys_follow_selection() {
        let mut a = App::new(
            Config {
                source: 3,
                custom_stations: vec![
                    station("One", "https://example.com/one"),
                    station("Two", "https://example.com/two"),
                ],
                ..Default::default()
            },
            "unused".into(),
        );
        a.action(Action::Settings);
        a.action(Action::SettingsRow(6));
        a.key(keypress(KeyCode::Down));
        a.key(keypress(KeyCode::Char('e')));
        assert!(matches!(&a.modal,Modal::Form(f)if f.fields[0]=="Two"));
        a.key(keypress(KeyCode::Esc));
        assert_eq!(a.menu_cursor, 1);
        a.key(keypress(KeyCode::Char('d')));
        assert!(
            matches!(&a.modal,Modal::ConfirmDelete{target:DeleteTarget::Custom(s),..}if s.name=="Two")
        );
        a.key(keypress(KeyCode::Char('n')));
        assert_eq!(a.config.custom_stations.len(), 2);
        a.key(keypress(KeyCode::Esc));
        assert!(matches!(a.modal, Modal::Settings(6)));
    }
    #[test]
    fn help_returns_to_original_input_without_losing_text() {
        let mut a = App::new(Config::default(), "unused".into());
        a.action(Action::AddSource);
        if let Modal::Form(f) = &mut a.modal {
            f.fields[0] = "draft".into();
            f.focus = 1;
        }
        a.key(keypress(KeyCode::F(1)));
        assert!(matches!(a.modal, Modal::Help));
        a.key(keypress(KeyCode::Esc));
        assert!(matches!(&a.modal,Modal::Form(f)if f.fields[0]=="draft"&&f.focus==1));
    }
}

#[cfg(test)]
mod inflight_cache_tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    #[test]
    fn clearing_cache_discards_inflight_results_and_cannot_repopulate_disk() {
        let d = tempfile::tempdir().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (connected_tx, connected_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut headers = vec![];
            loop {
                let mut b = [0];
                socket.read_exact(&mut b).unwrap();
                headers.push(b[0]);
                if headers.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            connected_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            let body = "#EXTM3U\n#EXTINF:-1,New\nhttps://example.com/new\n";
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let config = Config {
            source: 4,
            subscriptions: vec![Subscription {
                name: "test".into(),
                location: format!("http://{address}/list.m3u"),
            }],
            ..Default::default()
        };
        let mut app = App::new(config, d.path().join("config.toml"));
        app.cache = DirectoryCache::new(d.path().join("cache"));
        let key = cache::key(&app.config, "").unwrap();
        app.cache
            .write(
                &key,
                &[Station {
                    id: "old".into(),
                    name: "Old".into(),
                    urls: vec!["https://example.com/old".into()],
                    ..Default::default()
                }],
                0,
            )
            .unwrap();
        app.force_refresh();
        connected_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        let generation = app.directory_generation;
        app.force_refresh();
        assert_eq!(app.directory_generation, generation);
        assert_eq!(app.directory_threads.len(), 1);
        app.action(Action::ClearCache);
        app.action(Action::ConfirmDelete);
        release_tx.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while app.directory_threads.iter().any(|t| !t.is_finished()) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(app.directory_threads.iter().all(|t| t.is_finished()));
        app.tick();
        server.join().unwrap();
        assert!(app.cache.read(&key, "").is_none());
        assert_eq!(app.stations[0].name, "Old");
        assert!(app.directory_info.contains("已清除"));
        assert!(!app.loading);
    }
}

#[cfg(test)]
mod sorting_alias_tests {
    use super::*;
    use crate::config::SortOrder;
    fn station(id: &str, name: &str) -> Station {
        Station {
            id: id.into(),
            name: name.into(),
            urls: vec![format!("https://example.com/{id}")],
            ..Default::default()
        }
    }
    fn ids(app: &App) -> Vec<&str> {
        app.stations.iter().map(|s| s.id.as_str()).collect()
    }
    #[test]
    fn sorting_preserves_identity_route_and_restores_directory_order() {
        let mut a = App::new(Config::default(), "unused".into());
        let mut z = station("z", "Zulu");
        z.urls.push("https://example.com/backup".into());
        a.install_stations(vec![z, station("a", "Alpha")]);
        a.action(Action::Station(0));
        a.route = 1;
        a.action(Action::Sort);
        assert_eq!(ids(&a), ["a", "z"]);
        assert_eq!(a.selected_station().unwrap().id, "z");
        assert_eq!(a.route, 1);
        a.config.sort = SortOrder::Default;
        a.install_stations(a.directory_stations.clone());
        assert_eq!(ids(&a), ["z", "a"]);
    }
    #[test]
    fn alias_is_bound_to_id_even_if_refresh_changes_selection_and_survives_restart() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("config.toml");
        let mut a = App::new(Config::default(), path.clone());
        a.install_stations(vec![station("one", "Original"), station("two", "Other")]);
        a.action(Action::Alias);
        if let Modal::Form(f) = &mut a.modal {
            f.fields[0] = "My radio".into();
        }
        a.install_stations(vec![station("two", "Other"), station("one", "Original")]);
        a.action(Action::Apply);
        a.save().unwrap();
        let config = Config::load(&path).unwrap();
        assert_eq!(config.aliases["one"], "My radio");
        assert!(!config.aliases.contains_key("two"));
        let original = a.stations.iter().find(|s| s.id == "one").unwrap();
        assert_eq!(original.name, "Original");
        assert_eq!(a.config.station_name(original), "My radio");
        a.action(Action::Alias);
        if let Modal::Form(f) = &mut a.modal {
            f.fields[0].clear();
        }
        a.action(Action::Apply);
        assert!(!a.config.aliases.contains_key("one"));
    }
    #[test]
    fn aliases_affect_name_sort_without_modifying_directory_data() {
        let mut a = App::new(
            Config {
                sort: SortOrder::NameAsc,
                ..Default::default()
            },
            "unused".into(),
        );
        a.config.aliases.insert("z".into(), "AAA".into());
        a.install_stations(vec![station("a", "Alpha"), station("z", "Zulu")]);
        assert_eq!(ids(&a), ["z", "a"]);
        assert_eq!(a.stations[0].name, "Zulu");
    }
    #[test]
    fn manual_order_preserves_hidden_entries_and_survives_reload() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("config.toml");
        let mut a = App::new(Config::default(), path.clone());
        let stations = vec![station("a", "A"), station("b", "B"), station("c", "C")];
        a.install_stations(stations.clone());
        a.action(Action::Station(2));
        a.action(Action::MoveStation(-1));
        assert_eq!(ids(&a), ["a", "c", "b"]);
        assert_eq!(a.selected_station().unwrap().id, "c");
        a.install_stations(vec![stations[0].clone(), stations[1].clone()]);
        a.action(Action::Station(1));
        a.action(Action::MoveStation(-1));
        a.install_stations(stations.clone());
        assert_eq!(ids(&a), ["b", "c", "a"]);
        a.save().unwrap();
        let mut restored = App::new(Config::load(&path).unwrap(), path);
        restored.install_stations(stations);
        assert_eq!(ids(&restored), ["b", "c", "a"]);
        restored.config.source = 1;
        restored.install_stations(vec![station("a", "A"), station("b", "B")]);
        assert_eq!(ids(&restored), ["a", "b"]);
    }
    #[test]
    fn country_and_favorites_sorting_are_stable_and_aliases_are_searchable_locally() {
        let mut a = App::new(Config::default(), "unused".into());
        let mut first = station("first", "A");
        first.country = "US".into();
        let mut second = station("second", "B");
        second.country = "CN".into();
        let unknown = station("unknown", "C");
        a.config.sort = SortOrder::Country;
        a.install_stations(vec![unknown.clone(), first.clone(), second.clone()]);
        assert_eq!(ids(&a), ["second", "first", "unknown"]);
        a.config.favorites.push(first.clone());
        a.config.sort = SortOrder::FavoritesFirst;
        a.install_stations(vec![unknown, first.clone(), second]);
        assert_eq!(ids(&a), ["first", "unknown", "second"]);
        a.config.source = 2;
        a.config.aliases.insert(first.id, "My favorite".into());
        let results = crate::directory::fetch(&a.config, "my favorite").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "A");
    }
    #[test]
    fn alt_arrows_move_station_and_preserve_selection() {
        let mut app = App::new(Config::default(), "unused".into());
        app.install_stations(vec![station("a", "A"), station("b", "B")]);
        app.key(KeyEvent::new(KeyCode::Down, KeyModifiers::ALT));
        assert_eq!(ids(&app), ["b", "a"]);
        assert_eq!(app.selected_station().unwrap().id, "a");
        app.key(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));
        assert_eq!(ids(&app), ["a", "b"]);
        assert_eq!(app.selected_station().unwrap().id, "a");
    }
}
