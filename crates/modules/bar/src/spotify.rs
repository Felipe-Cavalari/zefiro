//! Painel do chip Spotify no estilo Raycast (ADR-0024), portável: a pilha de camadas (raiz, busca,
//! biblioteca e listas), a linha escolhida, o filtro local, o menu de ações e os atalhos. Cada
//! entrada devolve `Effect`s que o app executa (rede, área de transferência, config); o desenho fica
//! em `spotify_view.rs`.

use std::collections::HashMap;
use zefiro_integration_spotify::api::{self, Kind, PlayTarget, Request, Source};
use zefiro_integration_spotify::model::{ApiError, Item, Link, Page, Player, inherit};

use crate::vscode::{edit_query, matches};

/// Linhas visíveis de uma lista (a rolagem anda de uma em uma).
pub const VISIBLE: usize = 10;
/// Faltando isto para o fim, a próxima página é pedida.
const PREFETCH: usize = 3;
/// Passos do submenu de volume; os dois últimos são relativos (−10 e +10).
pub const VOLUME_STEPS: [i8; 7] = [0, 25, 50, 75, 100, -10, 10];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Auth { NeedsClientId, #[default] NeedsLogin, Waiting, PortBusy, Ready }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter { #[default] All, Tracks, Artists, Albums, Playlists, Podcasts }

impl Filter {
    pub const ALL: [Filter; 6] = [Filter::All, Filter::Tracks, Filter::Artists, Filter::Albums, Filter::Playlists, Filter::Podcasts];

    pub fn kinds(&self) -> Vec<Kind> {
        match self {
            Filter::All => vec![Kind::Track, Kind::Artist, Kind::Album, Kind::Playlist, Kind::Show, Kind::Episode],
            Filter::Tracks => vec![Kind::Track],
            Filter::Artists => vec![Kind::Artist],
            Filter::Albums => vec![Kind::Album],
            Filter::Playlists => vec![Kind::Playlist],
            Filter::Podcasts => vec![Kind::Show, Kind::Episode],
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Filter::All => "Tudo",
            Filter::Tracks => "Músicas",
            Filter::Artists => "Artistas",
            Filter::Albums => "Álbuns",
            Filter::Playlists => "Playlists",
            Filter::Podcasts => "Podcasts",
        }
    }

    /// "Tudo" pede 5 de cada tipo; um tipo só, 10 por página (o máximo da API de 2026).
    pub fn limit(&self) -> u32 {
        if *self == Filter::All { 5 } else { 10 }
    }

    fn step(self, back: bool) -> Filter {
        let i = Filter::ALL.iter().position(|f| *f == self).unwrap_or(0);
        let n = Filter::ALL.len();
        Filter::ALL[if back { (i + n - 1) % n } else { (i + 1) % n }]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Root,
    Search,
    Library,
    /// `context` é o que "Tocar …" toca e o contexto das faixas; `parent` é quem abriu a lista
    /// (capa e nome das faixas de um álbum).
    List { source: Source, title: String, context: Option<String>, playlist_id: Option<String>, editable: bool, parent: Option<Box<Item>> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    pub screen: Screen,
    /// `None` = carregando.
    pub items: Option<Vec<Item>>,
    pub next_offset: Option<u32>,
    pub after: Option<String>,
    /// Posição em `rows()`.
    pub selected: usize,
    pub scroll: usize,
    pub filter: String,
    pub error: Option<ApiError>,
    /// Playlist de outro dono: a API não mostra as músicas.
    pub foreign: bool,
    loading_more: bool,
}

impl Layer {
    pub fn new(screen: Screen, items: Option<Vec<Item>>) -> Layer {
        Layer { screen, items, next_offset: None, after: None, selected: 0, scroll: 0, filter: String::new(), error: None, foreign: false, loading_more: false }
    }
}

/// Uma lista já lida: os itens, o próximo `offset` e o cursor (Artistas).
type Cached = (Vec<Item>, Option<u32>, Option<String>);

/// As entradas fixas da raiz e da biblioteca.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry { Library, Queue, Devices, Liked, Playlists, Albums, Artists, Shows }

impl Entry {
    pub fn title(&self) -> &'static str {
        match self {
            Entry::Library => "Sua biblioteca",
            Entry::Queue => "Fila",
            Entry::Devices => "Aparelhos",
            Entry::Liked => "Músicas curtidas",
            Entry::Playlists => "Playlists",
            Entry::Albums => "Álbuns",
            Entry::Artists => "Artistas",
            Entry::Shows => "Podcasts",
        }
    }

    fn source(&self) -> Option<Source> {
        match self {
            Entry::Library => None,
            Entry::Queue => Some(Source::Queue),
            Entry::Devices => Some(Source::Devices),
            Entry::Liked => Some(Source::Liked),
            Entry::Playlists => Some(Source::Playlists),
            Entry::Albums => Some(Source::Albums),
            Entry::Artists => Some(Source::Artists),
            Entry::Shows => Some(Source::Shows),
        }
    }
}

/// Uma linha da camada do topo. `Item` e `Recent` são índices em `items` e em `recent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row { Card, Entry(Entry), Recent(usize), Item(usize), Play, Note }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Play,
    Queue,
    ToggleLike,
    AddToPlaylist,
    RemoveFromThisPlaylist,
    GoAlbum,
    GoArtist,
    GoShow,
    CopyLink,
    CopyArtistTitle,
    CopyEmbed,
    Open,
    PlayShuffled,
    PlayHere,
    Volume,
    PlayPause,
    Next,
    Previous,
    Restart,
    Shuffle,
    Repeat,
    Back15,
    Forward15,
    Logout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuTarget { Row(usize), Card }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubMenu { Volume, Playlists(Option<Vec<Item>>) }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu { pub target: MenuTarget, pub actions: Vec<Action>, pub selected: usize, pub sub: Option<SubMenu> }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key { Up, Down, Enter, CtrlEnter, ShiftEnter, Tab, CtrlTab, CtrlShiftTab, Esc, CtrlL, CtrlP, CtrlC }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Pedir ao serviço o player, a fila e as tocadas há pouco.
    Refresh,
    Load { depth: usize, source: Source, offset: u32, after: Option<String> },
    /// O app dispara 300 ms depois da última e só a de `seq` mais novo.
    Search { seq: u64, query: String, filter: Filter, offset: u32 },
    Send(Request),
    ToggleLike(String),
    CheckLiked(String),
    LoadPlaylists,
    Copy(String),
    Close,
    Login,
    Logout,
    SaveClientId(String),
    CopyRedirect,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SpotifyPanel {
    pub auth: Auth,
    pub client_field: String,
    pub stack: Vec<Layer>,
    pub query: String,
    pub filter: Filter,
    pub menu: Option<Menu>,
    pub player: Option<Player>,
    /// Quando o `player` chegou (ms), para a barra de progresso andar entre leituras.
    pub player_at_ms: u64,
    /// Agora (ms), atualizado pelo app antes de cada entrada.
    pub now_ms: u64,
    pub up_next: Option<String>,
    pub recent: Vec<Item>,
    /// Aviso de erro no rodapé ("Sem conexão com o Spotify"…), até a próxima tecla.
    pub message: Option<String>,
    /// Confirmação no rodapé ("Copiado"…).
    pub toast: Option<String>,
    /// (uri, curtida) da última conferência do coração.
    pub liked: Option<(String, bool)>,
    pub seq: u64,
    /// Listas já lidas, por fonte, até 10 min de painel fechado (`idle`).
    cache: HashMap<Source, Cached>,
    /// O aviso que entra quando a ação mandada der certo ("Adicionada a Mix").
    pending_toast: Option<String>,
}

fn copy(text: Option<String>, fx: &mut Vec<Effect>, toast: &mut Option<String>) {
    if let Some(t) = text {
        fx.push(Effect::Copy(t));
        *toast = Some("Copiado".into());
    }
}

fn artist_title(item: &Item) -> String {
    let artists = item.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ");
    match (artists.is_empty(), &item.show) {
        (false, _) => format!("{artists} – {}", item.name),
        (true, Some(show)) => format!("{} – {}", show.name, item.name),
        (true, None) => item.name.clone(),
    }
}

/// Item avulso para abrir uma lista a partir de um link (álbum de uma faixa, artista…).
fn parent_from(kind: Kind, link: &Link, image: Option<String>) -> Item {
    Item { kind, uri: link.uri.clone(), name: link.name.clone(), subtitle: String::new(), duration_ms: None, image, album: None, artists: Vec::new(), show: None, editable: false, owner: None, total: None, active: false }
}

impl SpotifyPanel {
    pub fn new(auth: Auth) -> SpotifyPanel {
        SpotifyPanel { auth, ..Default::default() }
    }

    fn top(&self) -> &Layer {
        self.stack.last().expect("a raiz sempre existe")
    }

    fn top_mut(&mut self) -> &mut Layer {
        self.stack.last_mut().expect("a raiz sempre existe")
    }

    /// Ao abrir: raiz com o campo vazio. As listas lidas continuam em memória.
    pub fn opened(&mut self) -> Vec<Effect> {
        self.stack = vec![Layer::new(Screen::Root, Some(Vec::new()))];
        self.query.clear();
        self.menu = None;
        self.message = None;
        self.toast = None;
        if self.auth == Auth::Ready { vec![Effect::Refresh] } else { Vec::new() }
    }

    /// 10 min de painel fechado: esquece as listas e as tocadas há pouco.
    pub fn idle(&mut self) {
        self.cache.clear();
        self.recent.clear();
    }

    /// O texto do campo: a busca na raiz, o filtro local numa lista.
    pub fn field_text(&self) -> &str {
        match self.top().screen {
            Screen::Root | Screen::Search => &self.query,
            _ => &self.top().filter,
        }
    }

    /// Itens da camada do topo, sem o filtro.
    pub fn items(&self) -> &[Item] {
        self.top().items.as_deref().unwrap_or_default()
    }

    pub fn rows(&self) -> Vec<Row> {
        let layer = self.top();
        match &layer.screen {
            Screen::Root => {
                let mut rows = vec![Row::Card, Row::Entry(Entry::Library), Row::Entry(Entry::Queue), Row::Entry(Entry::Devices)];
                rows.extend((0..self.recent.len()).map(Row::Recent));
                rows
            }
            Screen::Library => [Entry::Liked, Entry::Playlists, Entry::Albums, Entry::Artists, Entry::Shows].into_iter().filter(|e| matches(&layer.filter, e.title(), "")).map(Row::Entry).collect(),
            Screen::Search => (0..self.items().len()).map(Row::Item).collect(),
            Screen::List { context, .. } => {
                let mut rows = Vec::new();
                if context.is_some() && layer.filter.is_empty() {
                    rows.push(Row::Play);
                }
                if layer.foreign {
                    rows.push(Row::Note);
                }
                rows.extend(self.items().iter().enumerate().filter(|(_, it)| matches(&layer.filter, &it.name, &it.subtitle)).map(|(i, _)| Row::Item(i)));
                rows
            }
        }
    }

    /// Escolhe a linha `i` (mouse ou teste) e a mantém à vista.
    pub fn select(&mut self, i: usize) {
        let count = self.rows().len();
        let layer = self.top_mut();
        layer.selected = i.min(count.saturating_sub(1));
        if layer.selected < layer.scroll {
            layer.scroll = layer.selected;
        } else if layer.selected >= layer.scroll + VISIBLE {
            layer.scroll = layer.selected + 1 - VISIBLE;
        }
    }

    fn selected_row(&self) -> Option<Row> {
        self.rows().get(self.top().selected).copied()
    }

    /// O item de uma linha (das listas ou das tocadas há pouco).
    fn row_item(&self, row: Row) -> Option<&Item> {
        match row {
            Row::Item(i) => self.items().get(i),
            Row::Recent(i) => self.recent.get(i),
            Row::Card => self.player.as_ref().and_then(|p| p.item.as_ref()),
            _ => None,
        }
    }

    fn progress_now(&self) -> u64 {
        match &self.player {
            Some(p) if p.playing => p.progress_ms + self.now_ms.saturating_sub(self.player_at_ms),
            Some(p) => p.progress_ms,
            None => 0,
        }
    }

    pub fn actions_for(&self, target: MenuTarget) -> Vec<Action> {
        use Action::*;
        let (row, item) = match target {
            MenuTarget::Card => (Row::Card, self.player.as_ref().and_then(|p| p.item.as_ref())),
            MenuTarget::Row(i) => match self.rows().get(i).copied() {
                Some(Row::Card) => return self.actions_for(MenuTarget::Card),
                Some(row) => (row, self.row_item(row)),
                None => return Vec::new(),
            },
        };
        if row == Row::Card {
            let mut out = vec![PlayPause, Next, Previous, Restart];
            match item.map(|i| i.kind) {
                Some(Kind::Track) => out.extend([ToggleLike, AddToPlaylist]),
                Some(Kind::Episode) => out.extend([Back15, Forward15]),
                _ => {}
            }
            out.extend([Shuffle, Repeat, Volume]);
            match item.map(|i| i.kind) {
                Some(Kind::Track) => out.extend([GoAlbum, GoArtist, CopyLink, CopyArtistTitle, CopyEmbed]),
                Some(Kind::Episode) => out.extend([GoShow, CopyLink]),
                _ => {}
            }
            out.push(Logout);
            return out;
        }
        let Some(item) = item else { return Vec::new() };
        match item.kind {
            Kind::Track => {
                let mut out = vec![Play, Queue, ToggleLike, AddToPlaylist];
                if matches!(&self.top().screen, Screen::List { editable: true, playlist_id: Some(_), .. }) {
                    out.push(RemoveFromThisPlaylist);
                }
                out.extend([GoAlbum, GoArtist, CopyLink, CopyArtistTitle, CopyEmbed]);
                out
            }
            Kind::Episode => vec![Play, Queue, GoShow, CopyLink],
            Kind::Album | Kind::Playlist | Kind::Artist | Kind::Show => vec![Open, Play, PlayShuffled, CopyLink],
            Kind::Device => vec![PlayHere, Volume],
        }
    }

    /// Texto de uma ação no menu e o atalho dela.
    pub fn action_label(&self, action: Action, target: MenuTarget) -> (String, &'static str) {
        let uri = match target {
            MenuTarget::Card => self.player.as_ref().and_then(|p| p.item.as_ref()).map(|i| i.uri.clone()),
            MenuTarget::Row(i) => self.rows().get(i).and_then(|r| self.row_item(*r)).map(|i| i.uri.clone()),
        };
        let liked = matches!((&self.liked, &uri), (Some((u, true)), Some(v)) if u == v);
        let playing = self.player.as_ref().is_some_and(|p| p.playing);
        let shuffle = self.player.as_ref().is_some_and(|p| p.shuffle);
        let repeat = self.player.as_ref().map_or(api::Repeat::Off, |p| p.repeat);
        match action {
            Action::Play => ("Tocar".into(), "↵"),
            Action::Queue => ("Pôr na fila".into(), "Ctrl+↵"),
            Action::ToggleLike => (if liked { "Descurtir" } else { "Curtir" }.into(), "Ctrl+L"),
            Action::AddToPlaylist => ("Adicionar a playlist…".into(), "Ctrl+P"),
            Action::RemoveFromThisPlaylist => ("Tirar desta playlist".into(), ""),
            Action::GoAlbum => ("Ir para o álbum".into(), ""),
            Action::GoArtist => ("Ir para o artista".into(), ""),
            Action::GoShow => ("Ir para o podcast".into(), ""),
            Action::CopyLink => ("Copiar link".into(), "Ctrl+C"),
            Action::CopyArtistTitle => ("Copiar \"artista – título\"".into(), ""),
            Action::CopyEmbed => ("Copiar código de incorporação".into(), ""),
            Action::Open => ("Abrir".into(), "↵"),
            Action::PlayShuffled => ("Tocar no aleatório".into(), "⇧↵"),
            Action::PlayHere => ("Tocar aqui".into(), "↵"),
            Action::Volume => ("Volume…".into(), ""),
            Action::PlayPause => (if playing { "Pausar" } else { "Tocar" }.into(), "Espaço"),
            Action::Next => ("Próxima".into(), ""),
            Action::Previous => ("Anterior".into(), ""),
            Action::Restart => ("Recomeçar".into(), ""),
            Action::Shuffle => (if shuffle { "Aleatório: ligado" } else { "Aleatório: desligado" }.into(), ""),
            Action::Repeat => (
                match repeat {
                    api::Repeat::Off => "Repetir: desligado",
                    api::Repeat::Context => "Repetir: tudo",
                    api::Repeat::Track => "Repetir: esta faixa",
                }
                .into(),
                "",
            ),
            Action::Back15 => ("−15 s".into(), ""),
            Action::Forward15 => ("+15 s".into(), ""),
            Action::Logout => ("Sair da conta do Spotify".into(), ""),
        }
    }

    fn push_list(&mut self, source: Source, title: String, context: Option<String>, playlist_id: Option<String>, editable: bool, parent: Option<Box<Item>>) -> Vec<Effect> {
        let cacheable = !matches!(source, Source::Queue | Source::Devices);
        let screen = Screen::List { source: source.clone(), title, context, playlist_id, editable, parent };
        let depth = self.stack.len();
        if cacheable && let Some((items, next, after)) = self.cache.get(&source).cloned() {
            let mut layer = Layer::new(screen, Some(items));
            layer.next_offset = next;
            layer.after = after;
            self.stack.push(layer);
            return Vec::new();
        }
        self.stack.push(Layer::new(screen, None));
        vec![Effect::Load { depth, source, offset: 0, after: None }]
    }

    /// Abre a lista de um álbum, playlist, artista ou podcast.
    fn open(&mut self, item: &Item) -> Vec<Effect> {
        let Some(id) = api::id_of(&item.uri).map(str::to_string) else { return Vec::new() };
        let (source, playlist_id) = match item.kind {
            Kind::Album => (Source::Album(id), None),
            Kind::Playlist => (Source::Playlist(id.clone()), Some(id)),
            Kind::Artist => (Source::Artist(id), None),
            Kind::Show => (Source::Show(id), None),
            _ => return Vec::new(),
        };
        self.push_list(source, item.name.clone(), Some(item.uri.clone()), playlist_id, item.editable, Some(Box::new(item.clone())))
    }

    fn open_entry(&mut self, entry: Entry) -> Vec<Effect> {
        match entry.source() {
            None => {
                self.stack.push(Layer::new(Screen::Library, Some(Vec::new())));
                Vec::new()
            }
            Some(source) => self.push_list(source, entry.title().into(), None, None, false, None),
        }
    }

    /// Tocar a linha: no contexto da lista (playlist, álbum, podcast), a lista carregada a partir
    /// dela (curtidas) ou só ela (busca, fila).
    fn play_row(&self, i: usize) -> Vec<Effect> {
        let Some(item) = self.items().get(i) else { return Vec::new() };
        let target = match (&self.top().screen, item.kind) {
            (_, Kind::Album | Kind::Playlist | Kind::Artist | Kind::Show) => PlayTarget::Context { uri: item.uri.clone(), offset_uri: None },
            (Screen::List { context: Some(ctx), source, .. }, _) if !matches!(source, Source::Artist(_)) => PlayTarget::Context { uri: ctx.clone(), offset_uri: Some(item.uri.clone()) },
            (Screen::List { source: Source::Liked, .. }, _) => PlayTarget::Uris(self.items()[i..].iter().take(100).map(|t| t.uri.clone()).collect()),
            _ => PlayTarget::Uris(vec![item.uri.clone()]),
        };
        vec![Effect::Send(api::play(&target, None))]
    }

    fn play_pause(&self) -> Vec<Effect> {
        let playing = self.player.as_ref().is_some_and(|p| p.playing);
        vec![Effect::Send(if playing { api::pause() } else { api::play(&PlayTarget::Resume, None) })]
    }

    /// Enter (ação principal) na linha escolhida; `shift` toca no aleatório.
    fn primary(&mut self, shift: bool) -> Vec<Effect> {
        let Some(row) = self.selected_row() else { return Vec::new() };
        match row {
            Row::Card => self.play_pause(),
            Row::Entry(e) => self.open_entry(e),
            Row::Note => Vec::new(),
            Row::Play => {
                let Screen::List { context: Some(ctx), .. } = &self.top().screen else { return Vec::new() };
                let play = Effect::Send(api::play(&PlayTarget::Context { uri: ctx.clone(), offset_uri: None }, None));
                if shift { vec![Effect::Send(api::shuffle(true)), play] } else { vec![play] }
            }
            Row::Recent(i) => {
                let Some(item) = self.recent.get(i).cloned() else { return Vec::new() };
                if shift { self.run(Action::PlayShuffled, MenuTarget::Row(self.top().selected), &item) } else { self.open(&item) }
            }
            Row::Item(i) => {
                let Some(item) = self.items().get(i).cloned() else { return Vec::new() };
                match item.kind {
                    Kind::Album | Kind::Playlist | Kind::Artist | Kind::Show if shift => self.run(Action::PlayShuffled, MenuTarget::Row(self.top().selected), &item),
                    Kind::Album | Kind::Playlist | Kind::Artist | Kind::Show => self.open(&item),
                    Kind::Device => vec![Effect::Send(api::transfer(&item.uri))],
                    Kind::Track | Kind::Episode => self.play_row(i),
                }
            }
        }
    }

    fn target_item(&self, target: MenuTarget) -> Option<Item> {
        match target {
            MenuTarget::Card => self.player.as_ref().and_then(|p| p.item.clone()),
            MenuTarget::Row(i) => self.rows().get(i).and_then(|r| self.row_item(*r)).cloned(),
        }
    }

    fn open_menu(&mut self, target: MenuTarget) -> Vec<Effect> {
        let actions = self.actions_for(target);
        if actions.is_empty() {
            return Vec::new();
        }
        self.menu = Some(Menu { target, actions, selected: 0, sub: None });
        match self.target_item(target) {
            Some(item) if item.kind == Kind::Track => vec![Effect::CheckLiked(item.uri)],
            _ => Vec::new(),
        }
    }

    fn menu_target_of_selection(&self) -> MenuTarget {
        match self.selected_row() {
            Some(Row::Card) => MenuTarget::Card,
            _ => MenuTarget::Row(self.top().selected),
        }
    }

    /// Executa `action` sobre `item`.
    fn run(&mut self, action: Action, target: MenuTarget, item: &Item) -> Vec<Effect> {
        let mut fx = Vec::new();
        let progress = self.progress_now();
        let mut toast = None;
        match action {
            Action::Play => match target {
                MenuTarget::Row(i) => match self.rows().get(i) {
                    Some(Row::Item(idx)) if matches!(item.kind, Kind::Track | Kind::Episode) => fx = self.play_row(*idx),
                    _ => fx.push(Effect::Send(api::play(&if matches!(item.kind, Kind::Track | Kind::Episode) { PlayTarget::Uris(vec![item.uri.clone()]) } else { PlayTarget::Context { uri: item.uri.clone(), offset_uri: None } }, None))),
                },
                MenuTarget::Card => fx = self.play_pause(),
            },
            Action::Queue => fx.push(Effect::Send(api::queue(&item.uri))),
            Action::ToggleLike => {
                self.cache.remove(&Source::Liked);
                fx.push(Effect::ToggleLike(item.uri.clone()));
            }
            Action::AddToPlaylist => {
                if let Some(menu) = &mut self.menu {
                    menu.sub = Some(SubMenu::Playlists(None));
                    menu.selected = 0;
                } else {
                    self.menu = Some(Menu { target, actions: vec![Action::AddToPlaylist], selected: 0, sub: Some(SubMenu::Playlists(None)) });
                }
                return vec![Effect::LoadPlaylists];
            }
            Action::RemoveFromThisPlaylist => {
                if let Screen::List { playlist_id: Some(id), title, source, .. } = self.top().screen.clone() {
                    fx.push(Effect::Send(api::playlist_remove(&id, &item.uri)));
                    self.pending_toast = Some(format!("Tirada de {title}"));
                    let layer = self.top_mut();
                    if let Some(items) = &mut layer.items
                        && let Some(pos) = items.iter().position(|it| it.uri == item.uri)
                    {
                        items.remove(pos);
                    }
                    self.cache.remove(&source);
                }
            }
            Action::GoAlbum => {
                if let Some(album) = &item.album {
                    let parent = parent_from(Kind::Album, album, item.image.clone());
                    self.menu = None;
                    return self.open(&parent);
                }
            }
            Action::GoArtist => {
                if let Some(artist) = item.artists.first() {
                    let parent = parent_from(Kind::Artist, artist, None);
                    self.menu = None;
                    return self.open(&parent);
                }
            }
            Action::GoShow => {
                if let Some(show) = &item.show {
                    let parent = parent_from(Kind::Show, show, item.image.clone());
                    self.menu = None;
                    return self.open(&parent);
                }
            }
            Action::CopyLink => copy(api::link(&item.uri), &mut fx, &mut toast),
            Action::CopyArtistTitle => copy(Some(artist_title(item)), &mut fx, &mut toast),
            Action::CopyEmbed => copy(api::embed(&item.uri), &mut fx, &mut toast),
            Action::Open => {
                self.menu = None;
                return self.open(item);
            }
            Action::PlayShuffled => fx.extend([Effect::Send(api::shuffle(true)), Effect::Send(api::play(&PlayTarget::Context { uri: item.uri.clone(), offset_uri: None }, None))]),
            Action::PlayHere => fx.push(Effect::Send(api::transfer(&item.uri))),
            Action::Volume => {
                if let Some(menu) = &mut self.menu {
                    menu.sub = Some(SubMenu::Volume);
                    menu.selected = 0;
                }
                return Vec::new();
            }
            Action::PlayPause => fx = self.play_pause(),
            Action::Next => fx.push(Effect::Send(api::next())),
            Action::Previous => fx.push(Effect::Send(api::previous())),
            Action::Restart => fx.push(Effect::Send(api::seek(0))),
            Action::Shuffle => fx.push(Effect::Send(api::shuffle(!self.player.as_ref().is_some_and(|p| p.shuffle)))),
            Action::Repeat => fx.push(Effect::Send(api::repeat(self.player.as_ref().map_or(api::Repeat::Off, |p| p.repeat).next()))),
            Action::Back15 => fx.push(Effect::Send(api::seek(progress.saturating_sub(15_000)))),
            Action::Forward15 => fx.push(Effect::Send(api::seek(progress + 15_000))),
            Action::Logout => fx.push(Effect::Logout),
        }
        if toast.is_some() {
            self.toast = toast;
        }
        self.menu = None;
        fx
    }

    /// Escolha no submenu aberto.
    fn run_sub(&mut self) -> Vec<Effect> {
        let Some(menu) = self.menu.clone() else { return Vec::new() };
        let Some(item) = self.target_item(menu.target) else { return Vec::new() };
        let fx = match &menu.sub {
            Some(SubMenu::Volume) => {
                let Some(&step) = VOLUME_STEPS.get(menu.selected) else { return Vec::new() };
                let (device, current) = match item.kind {
                    Kind::Device => (Some(item.uri.as_str()), 50),
                    _ => (None, self.player.as_ref().and_then(|p| p.device.as_ref()).and_then(|d| d.volume).unwrap_or(50)),
                };
                let target = if menu.selected >= 5 { (i16::from(current) + i16::from(step)).clamp(0, 100) as u8 } else { step as u8 };
                vec![Effect::Send(api::volume(target, device))]
            }
            Some(SubMenu::Playlists(Some(list))) => {
                let Some(pl) = list.get(menu.selected) else { return Vec::new() };
                let Some(id) = api::id_of(&pl.uri) else { return Vec::new() };
                self.pending_toast = Some(format!("Adicionada a {}", pl.name));
                self.cache.remove(&Source::Playlist(id.to_string()));
                vec![Effect::Send(api::playlist_add(id, &item.uri))]
            }
            _ => return Vec::new(),
        };
        self.menu = None;
        fx
    }

    fn menu_len(&self) -> usize {
        match &self.menu {
            Some(Menu { sub: Some(SubMenu::Volume), .. }) => VOLUME_STEPS.len(),
            Some(Menu { sub: Some(SubMenu::Playlists(list)), .. }) => list.as_ref().map_or(0, Vec::len),
            Some(m) => m.actions.len(),
            None => 0,
        }
    }

    /// Volume do aparelho de uma linha: só os passos absolutos (a API não diz o volume dele aqui).
    pub fn volume_steps(&self) -> &'static [i8] {
        match self.menu.as_ref().and_then(|m| self.target_item(m.target)) {
            Some(item) if item.kind == Kind::Device => &VOLUME_STEPS[..5],
            _ => &VOLUME_STEPS,
        }
    }

    fn search(&mut self, offset: u32) -> Vec<Effect> {
        if offset == 0 {
            self.seq += 1;
        }
        vec![Effect::Search { seq: self.seq, query: self.query.clone(), filter: self.filter, offset }]
    }

    pub fn key(&mut self, k: Key) -> Vec<Effect> {
        self.message = None;
        match self.auth {
            Auth::NeedsClientId => {
                return match k {
                    Key::Enter if !self.client_field.trim().is_empty() => vec![Effect::SaveClientId(self.client_field.trim().to_string())],
                    Key::Esc => vec![Effect::Close],
                    _ => Vec::new(),
                };
            }
            Auth::NeedsLogin | Auth::PortBusy => {
                return match k {
                    Key::Enter => vec![Effect::Login],
                    Key::Esc => vec![Effect::Close],
                    _ => Vec::new(),
                };
            }
            Auth::Waiting => return if k == Key::Esc { vec![Effect::Close] } else { Vec::new() },
            Auth::Ready => {}
        }
        if self.menu.is_some() {
            return self.menu_key(k);
        }
        match k {
            Key::Up | Key::Down => {
                let len = self.rows().len();
                let cur = self.top().selected;
                let next = if k == Key::Up { cur.saturating_sub(1) } else { (cur + 1).min(len.saturating_sub(1)) };
                self.select(next);
                Vec::new()
            }
            Key::Enter => self.primary(false),
            Key::ShiftEnter => self.primary(true),
            Key::CtrlEnter => match self.selected_row().and_then(|r| self.row_item(r)).cloned() {
                Some(item) if matches!(item.kind, Kind::Track | Kind::Episode) => vec![Effect::Send(api::queue(&item.uri))],
                _ => Vec::new(),
            },
            Key::Tab => {
                let target = self.menu_target_of_selection();
                self.open_menu(target)
            }
            Key::CtrlTab | Key::CtrlShiftTab => {
                if self.top().screen != Screen::Search {
                    return Vec::new();
                }
                self.filter = self.filter.step(k == Key::CtrlShiftTab);
                self.set_filter(self.filter)
            }
            Key::Esc => {
                if self.stack.len() == 1 {
                    return vec![Effect::Close];
                }
                if self.top().screen == Screen::Search {
                    self.query.clear();
                }
                self.stack.pop();
                Vec::new()
            }
            Key::CtrlL => match self.selected_row().and_then(|r| self.row_item(r)).cloned() {
                Some(item) if item.kind == Kind::Track => {
                    self.cache.remove(&Source::Liked);
                    vec![Effect::ToggleLike(item.uri)]
                }
                _ => Vec::new(),
            },
            Key::CtrlP => match self.selected_row().and_then(|r| self.row_item(r)).cloned() {
                Some(item) if item.kind == Kind::Track => {
                    let target = self.menu_target_of_selection();
                    self.run(Action::AddToPlaylist, target, &item)
                }
                _ => Vec::new(),
            },
            Key::CtrlC => match self.selected_row().and_then(|r| self.row_item(r)).cloned() {
                Some(item) => {
                    let mut fx = Vec::new();
                    copy(api::link(&item.uri), &mut fx, &mut self.toast);
                    fx
                }
                None => Vec::new(),
            },
        }
    }

    fn menu_key(&mut self, k: Key) -> Vec<Effect> {
        let len = self.menu_len();
        let Some(menu) = &mut self.menu else { return Vec::new() };
        match k {
            Key::Up => menu.selected = menu.selected.saturating_sub(1),
            Key::Down => menu.selected = (menu.selected + 1).min(len.saturating_sub(1)),
            Key::Esc => {
                if menu.sub.is_some() && menu.actions.len() > 1 {
                    menu.sub = None;
                    menu.selected = 0;
                } else {
                    self.menu = None;
                }
            }
            Key::Enter => {
                if menu.sub.is_some() {
                    return self.run_sub();
                }
                let (target, action) = (menu.target, menu.actions.get(menu.selected).copied());
                let (Some(action), Some(item)) = (action, self.target_item(target)) else { return Vec::new() };
                return self.run(action, target, &item);
            }
            _ => {}
        }
        Vec::new()
    }

    /// Troca o filtro da busca e busca de novo.
    pub fn set_filter(&mut self, filter: Filter) -> Vec<Effect> {
        self.filter = filter;
        if self.top().screen != Screen::Search {
            return Vec::new();
        }
        let layer = self.top_mut();
        layer.items = None;
        layer.selected = 0;
        layer.scroll = 0;
        layer.next_offset = None;
        self.search(0)
    }

    /// `WM_CHAR`. Espaço com o campo vazio toca ou pausa.
    pub fn char(&mut self, code: u32, paste: Option<&str>) -> Vec<Effect> {
        self.message = None;
        match self.auth {
            Auth::NeedsClientId => {
                edit_query(&mut self.client_field, code, paste);
                let trimmed = self.client_field.trim().to_string();
                self.client_field = trimmed;
                return Vec::new();
            }
            Auth::Ready => {}
            _ => return Vec::new(),
        }
        if self.menu.is_some() {
            return Vec::new();
        }
        if code == 32 && self.field_text().is_empty() {
            return self.play_pause();
        }
        match self.top().screen {
            Screen::Root | Screen::Search => {
                if !edit_query(&mut self.query, code, paste) {
                    return Vec::new();
                }
                let on_root = self.top().screen == Screen::Root;
                if self.query.is_empty() {
                    if !on_root {
                        self.stack.pop();
                    }
                    return Vec::new();
                }
                if on_root {
                    self.stack.push(Layer::new(Screen::Search, None));
                } else {
                    let layer = self.top_mut();
                    layer.items = None;
                    layer.selected = 0;
                    layer.scroll = 0;
                }
                self.search(0)
            }
            _ => {
                let layer = self.top_mut();
                if edit_query(&mut layer.filter, code, paste) {
                    layer.selected = 0;
                    layer.scroll = 0;
                }
                Vec::new()
            }
        }
    }

    /// Clique (`right` = botão direito) numa área do painel; `None` = fora de tudo.
    pub fn click(&mut self, hit: Option<crate::spotify_view::Hit>, right: bool) -> Vec<Effect> {
        use crate::spotify_view::{CardButton, Hit};
        self.message = None;
        let Some(hit) = hit else {
            self.menu = None;
            return Vec::new();
        };
        match hit {
            Hit::Row(i) => {
                self.menu = None;
                self.select(i);
                if right {
                    let target = self.menu_target_of_selection();
                    self.open_menu(target)
                } else {
                    self.primary(false)
                }
            }
            Hit::Filter(f) => self.set_filter(f),
            Hit::Back => {
                self.menu = None;
                self.key(Key::Esc)
            }
            Hit::MenuItem(i) => {
                if let Some(menu) = &mut self.menu {
                    menu.selected = i;
                }
                self.menu_key(Key::Enter)
            }
            Hit::Card(button) => {
                self.menu = None;
                match button {
                    CardButton::Previous => vec![Effect::Send(api::previous())],
                    CardButton::Next => vec![Effect::Send(api::next())],
                    CardButton::PlayPause => self.play_pause(),
                    CardButton::Like => match self.player.as_ref().and_then(|p| p.item.as_ref()) {
                        Some(item) if item.kind == Kind::Track => {
                            self.cache.remove(&Source::Liked);
                            vec![Effect::ToggleLike(item.uri.clone())]
                        }
                        _ => Vec::new(),
                    },
                }
            }
            Hit::ClientField => Vec::new(),
            Hit::CopyRedirect => {
                self.toast = Some("Copiado".into());
                vec![Effect::CopyRedirect]
            }
            Hit::SaveClientId => self.key(Key::Enter),
            Hit::Login => vec![Effect::Login],
        }
    }

    /// Mouse por cima: a linha (ou o item do menu) ganha o destaque. `true` se mudou.
    pub fn hover(&mut self, hit: Option<crate::spotify_view::Hit>) -> bool {
        use crate::spotify_view::Hit;
        match hit {
            Some(Hit::MenuItem(i)) => match &mut self.menu {
                Some(m) if m.selected != i => {
                    m.selected = i;
                    true
                }
                _ => false,
            },
            Some(Hit::Row(i)) if self.menu.is_none() && self.top().selected != i => {
                self.select(i);
                true
            }
            _ => false,
        }
    }

    /// Roda: rola a lista; perto do fim de uma lista paginada, pede a próxima página uma vez.
    pub fn wheel(&mut self, delta: i32) -> Vec<Effect> {
        if self.menu.is_some() {
            return Vec::new();
        }
        let count = self.rows().len();
        let depth = self.stack.len() - 1;
        let screen = self.top().screen.clone();
        let one_kind = self.filter != Filter::All;
        let layer = self.top_mut();
        let max = count.saturating_sub(VISIBLE);
        layer.scroll = if delta > 0 { layer.scroll.saturating_sub(1) } else { (layer.scroll + 1).min(max) };
        layer.selected = layer.selected.clamp(layer.scroll, (layer.scroll + VISIBLE).min(count).saturating_sub(1).max(layer.scroll));
        if layer.loading_more || layer.scroll + VISIBLE + PREFETCH < count || !layer.filter.is_empty() {
            return Vec::new();
        }
        match screen {
            Screen::Search if one_kind => {
                let offset = layer.next_offset;
                match offset {
                    Some(offset) => {
                        layer.loading_more = true;
                        self.search(offset)
                    }
                    None => Vec::new(),
                }
            }
            Screen::List { source, .. } => {
                let (offset, after) = (layer.next_offset, layer.after.clone());
                match (&source, offset, after) {
                    (Source::Artists, _, Some(after)) => {
                        layer.loading_more = true;
                        vec![Effect::Load { depth, source, offset: 0, after: Some(after) }]
                    }
                    (_, Some(offset), _) => {
                        layer.loading_more = true;
                        vec![Effect::Load { depth, source, offset, after: None }]
                    }
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        }
    }

    /// Chegou uma página da lista na profundidade `depth`.
    pub fn loaded(&mut self, depth: usize, offset: u32, page: Result<Option<Page>, ApiError>) {
        let Some(layer) = self.stack.get_mut(depth) else { return };
        let Screen::List { source, parent, .. } = &layer.screen else { return };
        let (source, parent) = (source.clone(), parent.clone());
        let more = offset > 0 || (source == Source::Artists && layer.loading_more);
        layer.loading_more = false;
        match page {
            Ok(Some(mut page)) => {
                if let Some(parent) = &parent {
                    inherit(&mut page, parent);
                }
                let items = layer.items.get_or_insert_with(Vec::new);
                if !more {
                    items.clear();
                }
                items.extend(page.items);
                layer.next_offset = page.next_offset;
                layer.after = page.after;
                layer.error = None;
                if !matches!(source, Source::Queue | Source::Devices) {
                    self.cache.insert(source, (items.clone(), layer.next_offset, layer.after.clone()));
                }
            }
            Ok(None) => {
                layer.items = Some(Vec::new());
                layer.foreign = true;
            }
            Err(e) => {
                if layer.items.is_none() {
                    layer.items = Some(Vec::new());
                }
                layer.error = Some(e);
                self.message = Some(error_message(e));
            }
        }
    }

    /// Chegou a busca `seq`; as de `seq` antigo são descartadas.
    pub fn searched(&mut self, seq: u64, offset: u32, result: Result<Vec<(Kind, Page)>, ApiError>) {
        if seq != self.seq {
            return;
        }
        let Some(layer) = self.stack.iter_mut().rev().find(|l| l.screen == Screen::Search) else { return };
        layer.loading_more = false;
        match result {
            Ok(groups) => {
                let next = groups.iter().find_map(|(_, p)| p.next_offset);
                let items = groups.into_iter().flat_map(|(_, p)| p.items);
                if offset == 0 {
                    layer.items = Some(items.collect());
                } else {
                    layer.items.get_or_insert_with(Vec::new).extend(items);
                }
                layer.next_offset = next;
                layer.error = None;
            }
            Err(e) => {
                layer.items.get_or_insert_with(Vec::new);
                layer.error = Some(e);
                self.message = Some(error_message(e));
            }
        }
    }

    pub fn set_player(&mut self, player: Option<Player>, now_ms: u64) {
        self.player = player;
        self.player_at_ms = now_ms;
        self.now_ms = self.now_ms.max(now_ms);
    }

    /// Resultado de uma ação mandada (`Effect::Send`).
    pub fn sent(&mut self, result: Result<(), ApiError>) -> Vec<Effect> {
        match result {
            Ok(()) => {
                if let Some(t) = self.pending_toast.take() {
                    self.toast = Some(t);
                }
                Vec::new()
            }
            Err(e) => {
                self.pending_toast = None;
                self.message = Some(error_message(e));
                match e {
                    ApiError::Unauthorized => {
                        self.auth = Auth::NeedsLogin;
                        Vec::new()
                    }
                    ApiError::NoActiveDevice if !matches!(self.top().screen, Screen::List { source: Source::Devices, .. }) => {
                        self.menu = None;
                        self.push_list(Source::Devices, Entry::Devices.title().into(), None, None, false, None)
                    }
                    _ => Vec::new(),
                }
            }
        }
    }

    pub fn set_liked(&mut self, uri: String, liked: bool) {
        self.liked = Some((uri, liked));
    }

    /// Playlists do usuário para "Adicionar a playlist…": só as que dá para editar.
    pub fn set_playlists(&mut self, list: Vec<Item>) {
        if let Some(Menu { sub: Some(SubMenu::Playlists(slot)), .. }) = &mut self.menu {
            *slot = Some(list.into_iter().filter(|p| p.editable).collect());
        }
    }
}

/// Mensagem de um erro da API para o painel.
pub fn error_message(e: ApiError) -> String {
    match e {
        ApiError::Network | ApiError::Http(_) => "Sem conexão com o Spotify".into(),
        ApiError::NoActiveDevice => "Nenhum aparelho ativo. Abra o Spotify em algum aparelho".into(),
        ApiError::PremiumRequired => "Esta conta não é Premium".into(),
        ApiError::RateLimited(s) => format!("Muitas consultas; tentando de novo em {s} s"),
        ApiError::Unauthorized => "Entre de novo no Spotify".into(),
    }
}
