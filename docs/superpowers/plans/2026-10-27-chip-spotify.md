# Chip Spotify — plano

> **Para agentes:** SUB-SKILL OBRIGATÓRIA: use superpowers:subagent-driven-development (recomendado) ou
> superpowers:executing-plans para executar este plano tarefa por tarefa. Os passos usam caixas
> (`- [ ]`) para acompanhar.

**Objetivo:** um chip `spotify` cujo painel, no estilo Raycast, busca no Spotify, navega pela
biblioteca, mostra a fila e os aparelhos e age sobre a música atual pela Web API, sem trazer o
Spotify para a frente.

**Arquitetura:**
- A crate portável nova `zefiro-integration-spotify` tem:
  - PKCE e login;
  - os pedidos da API (`Request`);
  - a leitura das respostas;
  - a sessão com renovação de token e 429, testável com um transporte falso;
  - o cache de capas.
- O `zefiro-bar` guarda o estado do painel (pilha de camadas, menu de ações, teclado) e o desenha.
  Ele devolve `Effect`s que o app executa.
- A `platform-windows` traz:
  - o HTTP com método e corpo;
  - o DPAPI e os bytes aleatórios;
  - o listener do callback;
  - a thread `zefiro-spotify`.
- O `zefiro-app` junta tudo num `spotify_loop.rs`, no molde do `vscode_loop.rs`.

**Stack:** Rust 2024, `serde_json`, `toml_edit`, WinRT `HttpClient`, Win32 `CryptProtectData` e
`BCryptGenRandom`, `std::net::TcpListener`.

**Spec:** `docs/superpowers/specs/2026-10-27-chip-spotify-design.md` (leia junto).

Ledger: `.superpowers/sdd/2026-10-27-chip-spotify/progress.md`.
- **Por tarefa:** `cargo test -p <crate>`.
- **No fim:**
  - `cargo test --workspace`;
  - `cargo clippy --workspace --all-targets -- -D warnings`;
  - `pwsh -NoProfile -File scripts/check-windows-imports.ps1`.

## Restrições globais

- **Crate `windows`:** só em `crates/platform-windows`, `crates/integrations/clock/src/windows.rs` e
  `crates/app`. A crate nova e o `zefiro-bar` não usam.
- **Lint:** `clippy::all = deny`. `unsafe` só em blocos pequenos com `// SAFETY:`.
- **Idioma:** interface, comentários e logs em pt-BR. Identificadores e commits em inglês:
  `feat(spotify): …`, `feat(bar): …`, `feat(platform-windows): …`, `feat(app): …`, `docs(adr): …`.
- **Segredos:**
  - o token (de acesso ou de renovação) e o texto da busca nunca vão para log, teste, fixture ou
    sonda;
  - o token de acesso só existe na memória da thread `zefiro-spotify`;
  - o de renovação só existe cifrado (DPAPI) em `%LOCALAPPDATA%\zefiro\spotify\token.bin`.
- **Thread de UI:** nada de rede, DPAPI ou decodificação de imagem nela. A gravação do `config.toml`
  é a exceção, como no `terminal_loop`.
- **Painel fechado:** nenhuma consulta de rede.
- **Constantes:**
  - Redirect URI `http://127.0.0.1:47823/callback`, porta `47823`;
  - base da API `https://api.spotify.com/v1`;
  - autorização em `https://accounts.spotify.com/authorize`;
  - token em `https://accounts.spotify.com/api/token`;
  - busca dispara 300 ms depois da última tecla;
  - relê `/me/player` a cada 5 s com o painel aberto;
  - listener desiste em 5 min;
  - limpeza com 10 min de painel fechado;
  - cache de 150 capas;
  - `limit=5` na busca "Tudo" e `limit=10` num tipo só.
- **Catálogo:**
  - id `spotify`;
  - nome "Spotify";
  - descrição "Busque e toque no Spotify sem abrir o app (Premium)";
  - glifo `\u{EC4F}` (MusicNote);
  - sem logotipo do Spotify.
- **Textos exatos:**
  - campo de busca: "Buscar no Spotify…" e "Filtrar <nome>…";
  - entradas da raiz: "Sua biblioteca", com o subtítulo "Curtidas, playlists, álbuns, artistas,
    podcasts"; "Fila", com "A seguir: <x>"; "Aparelhos", com "Tocando no <x>";
  - rótulo "Tocadas há pouco";
  - "Nada tocando";
  - "Tocar a playlist", "Tocar o álbum" e "Tocar o artista";
  - nota "O Spotify não mostra as músicas desta playlist";
  - mensagens: "Carregando…", "Nenhum resultado", "Sem conexão com o Spotify", "Nenhum aparelho
    ativo. Abra o Spotify em algum aparelho", "Esta conta não é Premium", "Muitas consultas;
    tentando de novo em N s" e "A porta 47823 está em uso";
  - avisos no rodapé: "Copiado", "Adicionada a <playlist>" e "Tirada de <playlist>";
  - botões: "Entrar com o Spotify", "Salvar" e "Copiar";
  - ação "Sair da conta do Spotify";
  - página do callback: "Pronto, pode voltar ao Zefiro".
- **Escopos**, nesta ordem:
  `user-read-playback-state user-modify-playback-state user-read-currently-playing user-read-recently-played user-library-read user-library-modify playlist-read-private playlist-read-collaborative playlist-modify-private playlist-modify-public user-follow-read`.

## Foco da revisão

Cinco casos que o spec implica e que nenhum teste das tarefas pegaria sozinho. Cada um ganhou um
teste na tarefa dona:

1. **Token de acesso vence no meio do uso (1 h):** o próximo pedido renova uma vez e repete, sem o
   usuário ver nada. Teste na Tarefa 4.
2. **Busca com acento e símbolo** ("ação & café #1"): vai como UTF-8 com escape de URL, e não corta a
   query no `&`. Teste na Tarefa 2.
3. **Biblioteca vazia ou nada tocando** (player 204, `items: []`): mostra "Nada tocando" ou "Nenhum
   resultado", sem pânico nem "Carregando…" eterno. Testes nas Tarefas 3 e 7.
4. **Tocar sem aparelho ativo:** o painel mostra o aviso e abre Aparelhos sozinho. Teste na Tarefa 6.
5. **Spotify devolve um token de renovação novo:** o novo é gravado e o antigo deixa de ser usado.
   Senão, o login cai no dia seguinte. Teste na Tarefa 4.

---

### Tarefa 1: crate `zefiro-integration-spotify` com PKCE e login

**Arquivos:**
- Criar: `crates/integrations/spotify/Cargo.toml`, `src/lib.rs`, `src/auth.rs`.
- Mudar:
  - `Cargo.toml` (workspace): membro e dependência `zefiro-integration-spotify`;
  - `crates/integrations/update/src/lib.rs`: extrair `pub fn sha256(data: &[u8]) -> [u8; 32]` e
    fazer o `sha256_hex` usar ela.
- Dependências: `serde_json`, `zefiro-integration-update` (pelo `sha256`) e
  `zefiro-integration-chat` (pelo `image::base64`).

**Interfaces:**
- Produz, em `zefiro_integration_spotify::auth`:
  - `pub const PORT: u16 = 47823;`
  - `pub const REDIRECT_URI: &str;`
  - `pub const SCOPES: &str;`
  - `pub struct Pkce { pub verifier: String, pub challenge: String, pub state: String }`
  - `pub fn pkce(random: &[u8; 96]) -> Pkce`: os 64 primeiros bytes viram o verificador em base64url
    sem `=`; os 32 últimos, o `state`.
  - `pub fn challenge(verifier: &str) -> String`
  - `pub fn authorize_url(client_id: &str, p: &Pkce) -> String`
  - `pub enum Callback { Code(String), Denied(String), WrongState, NotCallback }`
  - `pub fn parse_callback(request_head: &str, state: &str) -> Callback`: `request_head` é a 1ª linha
    do HTTP, `GET /callback?… HTTP/1.1`.
  - `pub fn code_body(client_id: &str, code: &str, verifier: &str) -> String`
  - `pub fn refresh_body(client_id: &str, refresh: &str) -> String`: os dois em
    `application/x-www-form-urlencoded`.
  - `pub struct Tokens { pub access: String, pub expires_in: u64, pub refresh: Option<String> }`
  - `pub fn parse_tokens(body: &str) -> Option<Tokens>`
  - `pub struct Stored { pub client_id: String, pub refresh: String }`, com
    `fn to_bytes(&self) -> Vec<u8>` e `fn from_bytes(b: &[u8]) -> Option<Stored>`, em JSON.
  - `pub const DONE_PAGE: &str`: resposta HTTP 200 completa, com o HTML "Pronto, pode voltar ao
    Zefiro".
  - `pub fn url_encode(s: &str) -> String`: escape de URL do UTF-8, sem mexer em `A-Za-z0-9-._~`.

- [ ] **Passo 1: testes que falham**, em `crates/integrations/spotify/tests/auth.rs`.
  - `challenge_matches_rfc7636`:
    `challenge("dBjftJeZ4CVP-mJ92K9CQ1q0vTE7m-u3U0Rl5pV2Ugs") == "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"`.
  - `pkce_verifier_is_base64url_within_limits`:
    - o verificador tem de 43 a 128 caracteres, todos em `[A-Za-z0-9_-]`;
    - `challenge(&p.verifier) == p.challenge`;
    - o `state` não vazio e diferente do verificador.
  - `authorize_url_has_every_parameter`: a URL contém `response_type=code`, `client_id=abc`,
    `code_challenge_method=S256`, `code_challenge=…`, `state=…`, a
    `redirect_uri=http%3A%2F%2F127.0.0.1%3A47823%2Fcallback` codificada e os escopos separados por
    `%20`.
  - `callback_reads_code_state_and_denial`:
    - o `state` certo dá `Code("xyz")`;
    - o `state` errado dá `WrongState`;
    - `?error=access_denied&state=s` dá `Denied("access_denied")`;
    - `GET /favicon.ico` dá `NotCallback`.
  - `bodies_are_form_encoded`: `code_body("abc","c d","v")` contém `grant_type=authorization_code`,
    `code=c%20d`, `code_verifier=v`, `client_id=abc` e a `redirect_uri` codificada;
    `refresh_body` contém `grant_type=refresh_token`.
  - `tokens_keep_the_new_refresh_when_present`: com e sem `refresh_token` no JSON.
  - `stored_round_trips`.
  - `url_encode_handles_accents_and_reserved`: `url_encode("ação & café #1") == "a%C3%A7%C3%A3o%20%26%20caf%C3%A9%20%231"`.
- [ ] **Passo 2:** `cargo test -p zefiro-integration-spotify` falha porque as funções não existem.
- [ ] **Passo 3:** implementar. O base64url é `image::base64(..)` com `+→-`, `/→_` e sem `=`.
- [ ] **Passo 4:** `cargo test -p zefiro-integration-spotify -p zefiro-integration-update` passa.
- [ ] **Passo 5:** commit `feat(spotify): PKCE and login helpers`.

### Tarefa 2: pedidos da API

**Arquivos:**
- Criar: `crates/integrations/spotify/src/api.rs` e `tests/api.rs`.

**Interfaces:**
- Consome: `auth::url_encode`.
- Produz, em `zefiro_integration_spotify::api`:
  - `pub const BASE: &str = "https://api.spotify.com/v1";`
  - `pub enum Method { Get, Put, Post, Delete }`, com `fn as_str(&self) -> &'static str`.
  - `pub struct Request { pub method: Method, pub path: String, pub body: Option<String> }`: `path`
    sem a base, com a query, e o corpo em JSON.
  - `pub enum Kind { Track, Episode, Album, Artist, Playlist, Show }`, com
    `fn api_name(&self) -> &'static str`: `track`, `episode`, `album`, `artist`, `playlist`, `show`.
  - `pub enum Repeat { Off, Context, Track }`, com `fn next(self) -> Repeat` e
    `fn api_name(&self) -> &'static str`: `off`, `context`, `track`.
  - `pub enum Source { Liked, Playlists, Albums, Artists, Shows, Album(String), Artist(String), Show(String), Playlist(String), Queue, Devices }`.
    Os `String` são ids.
  - `pub enum PlayTarget { Uris(Vec<String>), Context { uri: String, offset_uri: Option<String> }, Resume }`
  - **Construtores**, todos `-> Request`:
    - `search(q: &str, kinds: &[Kind], limit: u32, offset: u32)`
    - `list(source: &Source, offset: u32, after: Option<&str>)`: `after` é o cursor de Artistas.
    - `me()`, `player()`, `recently_played()`
    - `album(id)`, `artist(id)`, `playlist(id)`: para nome e capa de "Tocadas há pouco".
    - `play(target: &PlayTarget, device: Option<&str>)`, `pause()`, `next()`, `previous()`
    - `seek(ms: u64)`, `shuffle(on: bool)`, `repeat(r: Repeat)`
    - `volume(percent: u8, device: Option<&str>)`, `queue(uri: &str)`
    - `transfer(device: &str)`, com `{"device_ids":[…],"play":true}`
    - `library_save(uris: &[String])`, `library_remove(uris: &[String])`, `library_contains(uris: &[String])`
    - `playlist_add(id: &str, uri: &str)`, `playlist_remove(id: &str, uri: &str)`
  - `pub fn link(uri: &str) -> Option<String>`: `spotify:track:ID` → `https://open.spotify.com/track/ID`.
  - `pub fn embed(uri: &str) -> Option<String>`: `<iframe style="border-radius:12px" src="https://open.spotify.com/embed/<tipo>/<id>" width="100%" height="352" frameBorder="0" allowfullscreen="" allow="autoplay; clipboard-write; encrypted-media; fullscreen; picture-in-picture" loading="lazy"></iframe>`.
  - `pub fn id_of(uri: &str) -> Option<&str>`.

- [ ] **Passo 1: testes que falham.**
  - `search_paths`: `search("ação & café #1", &[Kind::Track], 10, 20).path ==
    "/search?q=a%C3%A7%C3%A3o%20%26%20caf%C3%A9%20%231&type=track&limit=10&offset=20"`. A busca "Tudo"
    leva `type=track,artist,album,playlist,show,episode&limit=5`.
  - `list_paths`, um por `Source`:
    - `/me/tracks?limit=50&offset=0`;
    - `/me/playlists?…`, `/me/albums?…`, `/me/shows?…`;
    - `/me/following?type=artist&limit=50` (e `&after=X`);
    - `/albums/ID/tracks?…`, `/artists/ID/albums?include_groups=album,single&…`;
    - `/shows/ID/episodes?…`, `/playlists/ID/items?…`;
    - `/me/player/queue`, `/me/player/devices`.
  - `player_commands`:
    - `play(Context{uri:"spotify:playlist:P", offset_uri:Some("spotify:track:T")}, Some("D"))` dá
      `PUT /me/player/play?device_id=D` com o corpo
      `{"context_uri":"spotify:playlist:P","offset":{"uri":"spotify:track:T"}}`;
    - `Uris` dá `{"uris":[…]}`; `Resume` vai sem corpo;
    - `shuffle(true)` dá `PUT /me/player/shuffle?state=true`;
    - `repeat(Context)` dá `?state=context`;
    - `volume(50,None)` dá `?volume_percent=50`;
    - `queue(uri)` dá `POST /me/player/queue?uri=<codificado>`;
    - `seek` dá `?position_ms=`.
  - `library_and_playlists`:
    - `library_save` dá `PUT /me/library?uris=…`, com os URIs separados por vírgula e codificados;
    - `library_contains` dá `GET /me/library/contains?uris=…`;
    - `playlist_add` dá `POST /playlists/P/items` com `{"uris":["spotify:track:T"]}`;
    - `playlist_remove` dá `DELETE /playlists/P/items` com `{"items":[{"uri":"spotify:track:T"}]}`.
  - `repeat_cycles_off_context_track`.
  - `links_and_embed`: `link` do `spotify:episode:E` dá `https://open.spotify.com/episode/E`;
    `link("lixo") == None`; `embed` contém `open.spotify.com/embed/track/T`.
- [ ] **Passo 2:** os testes falham.
- [ ] **Passo 3:** implementar.
- [ ] **Passo 4:** os testes passam.
- [ ] **Passo 5:** commit `feat(spotify): Web API requests`.

### Tarefa 3: leitura das respostas e dos erros

**Arquivos:**
- Criar:
  - `crates/integrations/spotify/src/model.rs`;
  - `tests/model.rs`;
  - as fixtures `tests/fixtures/{search_all,player_playing,player_episode,queue,devices,liked,playlists,playlist_items,playlist_metadata_only,album_tracks,artist_albums,following,shows,show_episodes,recently_played,me,error_no_device,error_premium}.json`.
- **Fixtures:** escritas a partir dos exemplos da referência da Web API, cortadas ao mínimo, sem
  dados pessoais. A Tarefa 10 troca as principais por respostas reais.

**Interfaces:**
- Consome: `api::{Kind, Repeat}`.
- Produz, em `zefiro_integration_spotify::model`:
  - `pub struct Link { pub uri: String, pub name: String }`
  - `pub struct Item`, com os campos:
    - `kind: Kind`, `uri: String`, `name: String`, `subtitle: String`;
    - `duration_ms: Option<u64>`, `image: Option<String>`;
    - `album: Option<Link>`, `artists: Vec<Link>`, `show: Option<Link>`;
    - `editable: bool` (playlist do usuário ou colaborativa), `owner: Option<String>`;
    - `total: Option<u32>`.

    `subtitle` já vem pronto: "Artista · Álbum" (faixa), "Playlist · sua" ou
    "Playlist · <dono>", "Álbum · <ano>", "Artista", "Podcast · <editora>" e
    "<podcast> · <data>" (episódio).
  - `pub struct Page { pub items: Vec<Item>, pub next_offset: Option<u32>, pub after: Option<String> }`
  - `pub struct Device { pub id: String, pub name: String, pub kind: String, pub active: bool, pub volume: Option<u8> }`
  - `pub struct Player`, com os campos `item: Option<Item>`, `playing: bool`, `progress_ms: u64`,
    `device: Option<Device>`, `shuffle: bool`, `repeat: Repeat` e `context_uri: Option<String>`.
  - `pub enum ApiError { Network, Unauthorized, NoActiveDevice, PremiumRequired, RateLimited(u64), Http(u16) }`
  - **Funções:**
    - `pub fn api_error(status: u16, body: &str, retry_after: Option<u64>) -> ApiError`: um 404 com
      `reason` igual a `NO_ACTIVE_DEVICE` dá `NoActiveDevice`; um 403 com `PREMIUM_REQUIRED` dá
      `PremiumRequired`; 401 e 429 também viram as variantes deles.
    - `pub fn parse_me(body) -> Option<String>`: o id do usuário.
    - `pub fn parse_player(status: u16, body: &str) -> Option<Player>`: 204 ou corpo vazio dá `None`.
    - `pub fn parse_search(body: &str) -> Vec<(Kind, Page)>`: na ordem Músicas, Artistas, Álbuns,
      Playlists, Podcasts, Episódios, e só os grupos não vazios.
    - `pub fn parse_page(source: &Source, body: &str, me: &str) -> Option<Page>`: devolve `None` só em
      `Playlist(_)` sem `items`.
    - `pub fn parse_queue(body) -> (Option<Item>, Vec<Item>)`
    - `pub fn parse_devices(body) -> Vec<Device>`
    - `pub fn recent_contexts(body) -> Vec<String>`: os URIs de contexto distintos, até 5, na ordem.
    - `pub fn parse_one(kind: Kind, body: &str, me: &str) -> Option<Item>`: para os contextos.

- [ ] **Passo 1: testes que falham.**
  - `search_groups_in_order_without_empty_groups`.
  - `player_playing_paused_and_empty`: `parse_player(204,"") == None`; os campos `shuffle`, `repeat`,
    `device.name` e `progress_ms`; um episódio dá `kind == Episode` e `show` preenchido.
  - `playlist_items_use_2026_field_names`: `items[].item`.
  - `playlist_without_items_is_none`.
  - `liked_albums_shows_unwrap_saved_wrappers`.
  - `album_tracks_inherit_album_image_and_link`: `Item.album` e `image` vêm do álbum de cima.
  - `following_reads_cursor_after`.
  - `playlist_editable_when_owner_is_me_or_collaborative`.
  - `empty_items_give_empty_page`: `{"items":[],"next":null}` dá `Page` vazia, com `next_offset`
    igual a `None`.
  - `errors_map_reason_and_retry`: as duas fixtures de erro, 401, `429` com `Some(7)` dando
    `RateLimited(7)` e 500 dando `Http(500)`.
  - `recent_contexts_are_distinct_and_capped_at_five`.
  - `next_offset_comes_from_next_url`: `next` com `offset=50` dá `Some(50)`.
- [ ] **Passo 2:** os testes falham.
- [ ] **Passo 3:** implementar com `serde_json::Value`, como o resto das integrações.
- [ ] **Passo 4:** os testes passam.
- [ ] **Passo 5:** commit `feat(spotify): parse Web API responses`.

### Tarefa 4: sessão (token, renovação, 429) e cache de capas

**Arquivos:**
- Criar: `crates/integrations/spotify/src/session.rs`, `src/cache.rs` e `tests/session.rs`.

**Interfaces:**
- Consome: `auth::*`, `api::{Request, Method, BASE}` e `model::{ApiError, api_error}`.
- Produz:
  - `pub trait Transport { fn send(&self, method: &str, url: &str, headers: &[(&str, &str)], body: Option<(&str, &str)>) -> Result<(u16, String, Option<u64>), ()>; }`
    - `body` é o par (content-type, texto);
    - `Err(())` é falha de rede.
  - `pub struct Session`, que guarda o `client_id`, o de renovação, o de acesso e a hora em que ele
    vence, e o `me`:
    - `pub fn new(client_id: String, refresh: String) -> Session`
    - `pub fn execute(&mut self, t: &dyn Transport, req: &Request, now_s: u64) -> Result<(u16, String), ApiError>`
      - sem token de acesso, ou com menos de 60 s até vencer, renova antes;
      - num 401, renova **uma vez** e repete;
      - se a renovação for recusada (400 `invalid_grant`), devolve `Unauthorized`;
      - 2xx dá `Ok`; o resto passa por `api_error`.
    - `pub fn take_new_refresh(&mut self) -> Option<String>`: o token de renovação novo que chegou,
      para o serviço gravar.
    - `pub fn stored(&self) -> Stored`
  - `pub fn exchange_code(t: &dyn Transport, client_id: &str, code: &str, verifier: &str) -> Result<Tokens, ApiError>`
  - `pub struct CoverCache<T> { … }` (LRU), com:
    - `pub fn new(cap: usize) -> Self`
    - `pub fn get(&mut self, url: &str) -> Option<&T>`: a capa lida passa a ser a mais recente.
    - `pub fn insert(&mut self, url: String, value: T)`
    - `pub fn clear(&mut self)`
    - `pub fn len(&self) -> usize`
  - `pub const COVERS: usize = 150;`

- [ ] **Passo 1: testes que falham**, com um `FakeTransport` que guarda os pedidos e devolve as
  respostas em fila.
  - `first_call_refreshes_then_sends_bearer`: o 1º pedido vai para `accounts.spotify.com/api/token`;
    o 2º leva `Authorization: Bearer A1`.
  - `expired_access_token_refreshes_once_and_retries`, do Foco 1: depois de `now_s` passar do
    `expires_in`, há 1 renovação e 1 pedido, e a resposta é `Ok`.
  - `unauthorized_retries_once_then_gives_up`: 401, renova, 401 de novo, e dá `Err(Unauthorized)`.
    São 3 pedidos, sem laço.
  - `invalid_grant_is_unauthorized`.
  - `new_refresh_token_is_reported_once`, do Foco 5: `take_new_refresh()` dá `Some("R2")` e depois
    `None`, e a próxima renovação usa `R2`.
  - `rate_limit_passes_retry_after`: dá `Err(RateLimited(7))`.
  - `network_failure_is_network`.
  - `cover_cache_evicts_least_recent`: com capacidade 2, inserir a e b, ler a, inserir c; b sai e a
    fica.
- [ ] **Passo 2:** os testes falham.
- [ ] **Passo 3:** implementar.
- [ ] **Passo 4:** os testes passam.
- [ ] **Passo 5:** commit `feat(spotify): session with refresh and cover cache`.

### Tarefa 5: `[spotify]` no config

**Arquivos:**
- Mudar: `crates/core/src/config.rs` e os testes dele.

**Interfaces:**
- Produz:
  - `pub struct SpotifyConfig { pub client_id: String }`, com `Default` vazio e `serde(default)`;
  - o campo `Config::spotify`;
  - `pub fn set_spotify(path: &Path, spotify: &SpotifyConfig) -> std::io::Result<()>`, no molde do
    `set_terminal`, gravando a chave com o comentário do spec na primeira vez;
  - `same_except_panels` passa a ignorar também `spotify`.

- [ ] **Passo 1: testes que falham.**
  - `spotify_section_parses_and_defaults_empty`.
  - `set_spotify_keeps_comments_and_other_sections`.
  - `only_spotify_changed_is_same_except_panels`.
- [ ] **Passo 2:** `cargo test -p zefiro-core` falha.
- [ ] **Passo 3:** implementar.
- [ ] **Passo 4:** os testes passam.
- [ ] **Passo 5:** commit `feat(core): spotify config section`.

### Tarefa 6: estado do painel (portável)

**Arquivos:**
- Criar: `crates/modules/bar/src/spotify.rs`, só o estado; o desenho fica na Tarefa 7.
- Mudar:
  - `crates/modules/bar/src/lib.rs`;
  - `crates/modules/bar/Cargo.toml`: depender de `zefiro-integration-spotify`;
  - `catalog.rs`: o item `spotify`;
  - `controller.rs`: `PanelKind::Spotify` e `"spotify" => Some(PanelKind::Spotify)`.

**Interfaces:**
- Consome: `api::*` e `model::*`. O `fold` do `vscode.rs` passa a ser `pub(crate)` e é reaproveitado.
- Produz, em `zefiro_bar::spotify`:
  - `pub enum Auth { NeedsClientId, NeedsLogin, Waiting, PortBusy, Ready }`
  - `pub enum Filter { All, Tracks, Artists, Albums, Playlists, Podcasts }`, com
    `fn kinds(&self) -> Vec<Kind>`; Podcasts inclui episódios.
  - `pub enum Screen`:
    - `Root`;
    - `Search`;
    - `Library`;
    - `List { source: Source, title: String, context: Option<String>, playlist_id: Option<String>, editable: bool, header: Option<Header> }`,
      onde `Header { Play(String), Note }` é a 1ª linha "Tocar …" ou a nota da playlist de outro dono.
  - `pub struct Layer { pub screen: Screen, pub items: Option<Vec<Item>>, pub next_offset: Option<u32>, pub after: Option<String>, pub selected: usize, pub scroll: usize, pub filter: String, pub error: Option<ApiError> }`
  - `pub enum Row`: `Entry(usize)` (as 3 entradas da raiz e as 5 da Biblioteca), `Item(usize)` e
    `Header`.
  - `pub enum Action`:
    - `Play`, `Queue`, `ToggleLike`, `AddToPlaylist`, `RemoveFromThisPlaylist`;
    - `GoAlbum`, `GoArtist`, `GoShow`;
    - `CopyLink`, `CopyArtistTitle`, `CopyEmbed`;
    - `Open`, `PlayShuffled`, `PlayHere`, `Volume`;
    - `PlayPause`, `Next`, `Previous`, `Restart`, `Shuffle`, `Repeat`;
    - `Back15`, `Forward15`, `Logout`.
  - `pub struct Menu { pub target: MenuTarget, pub actions: Vec<Action>, pub selected: usize, pub sub: Option<SubMenu> }`
    - `MenuTarget { Row(usize), Card }`;
    - `SubMenu { Volume, Playlists(Option<Vec<Item>>) }`;
    - o volume tem os passos 0, 25, 50, 75, 100, −10 e +10.
  - `pub enum Key { Up, Down, Enter, CtrlEnter, ShiftEnter, Tab, CtrlTab, CtrlShiftTab, Esc, CtrlL, CtrlP, CtrlC }`
  - `pub enum Effect`:
    - `Refresh`: o app pede ao serviço `Player`, `Queue` e `Recent`.
    - `Load { depth: usize, source: Source, offset: u32, after: Option<String> }`
    - `Search { seq: u64, query: String, filter: Filter, offset: u32 }`: o app dispara com 300 ms de
      atraso e só o `seq` mais novo.
    - `Send(Request)`
    - `ToggleLike(String)`, `CheckLiked(String)`
    - `LoadPlaylists`
    - `Copy(String)`
    - `Close`
    - `Login`, `Logout`, `SaveClientId(String)`, `CopyRedirect`
  - `pub struct SpotifyPanel`, com os campos:
    - `pub auth: Auth`;
    - `pub client_field: String`;
    - `pub stack: Vec<Layer>`;
    - `pub query: String`;
    - `pub filter: Filter`;
    - `pub menu: Option<Menu>`;
    - `pub player: Option<Player>`, `pub player_at_ms: u64`;
    - `pub up_next: Option<String>`;
    - `pub recent: Vec<Item>`;
    - `pub message: Option<String>`;
    - `pub toast: Option<String>`;
    - `pub liked: Option<(String, bool)>`;
    - `pub seq: u64`.

    Métodos:
    - `pub fn opened(&mut self) -> Vec<Effect>`: volta à raiz com o campo vazio e mantém as camadas
      carregadas em cache por fonte.
    - `pub fn key(&mut self, k: Key) -> Vec<Effect>`
    - `pub fn char(&mut self, code: u32, paste: Option<&str>) -> Vec<Effect>`: Espaço com o campo
      vazio toca ou pausa; senão, edita o texto pelo `vscode::edit_query`.
    - `pub fn click(&mut self, hit: Hit, right: bool) -> Vec<Effect>`: o `Hit` vem da Tarefa 7.
    - `pub fn wheel(&mut self, delta: i32) -> Vec<Effect>`: perto do fim de uma lista paginada, dá
      `Load` ou `Search` com o próximo `offset`.
    - Chegadas: `pub fn loaded(&mut self, depth: usize, offset: u32, page: Result<Option<Page>, ApiError>)`,
      `pub fn searched(&mut self, seq: u64, offset: u32, result: Result<Vec<(Kind, Page)>, ApiError>)`,
      `pub fn set_player(&mut self, p: Option<Player>, now_ms: u64)`,
      `pub fn sent(&mut self, r: Result<(), ApiError>) -> Vec<Effect>`,
      `pub fn set_liked(&mut self, uri: String, liked: bool)` e
      `pub fn set_playlists(&mut self, list: Vec<Item>)`.
    - `pub fn actions_for(&self, target: MenuTarget) -> Vec<Action>`
    - `pub fn rows(&self) -> Vec<Row>`: as linhas da camada do topo, já filtradas.

- [ ] **Passo 1: testes que falham.**
  - `opens_at_root_with_empty_field_and_asks_player`: depois de uma busca e de entrar numa lista,
    `opened()` deixa `stack.len() == 1` (raiz), `query == ""` e devolve `vec![Effect::Refresh]`.
  - `typing_at_root_searches_and_old_results_are_dropped`: `searched` com um `seq` antigo não troca
    os itens.
  - `typing_inside_a_list_filters_locally_without_accents`: na lista com "Ação" e "Rock", digitar
    "acao" deixa 1 linha e não gera `Effect`.
  - `esc_pops_one_layer_then_closes`: Biblioteca → Playlists → Esc → Biblioteca → Esc → raiz → Esc
    dá `Close`. Esc com o menu aberto fecha só o menu.
  - `going_back_keeps_selection_and_scroll`.
  - `enter_on_track_inside_playlist_plays_in_context`: dá `Send(play(Context{uri:playlist, offset_uri:Some(track)}, None))`.
  - `ctrl_enter_queues_and_shift_enter_shuffles`: em álbum, Shift+Enter dá `Send(shuffle(true))`
    seguido de `Send(play(Context))`.
  - `foreign_playlist_shows_only_play_and_note`: `loaded(.., Ok(None))` dá um `Header::Play` e uma
    `Header::Note`, sem itens.
  - `actions_menu_per_kind`:
    - música dentro de playlist editável tem `RemoveFromThisPlaylist`, e fora dela não tem;
    - episódio tem `GoShow`;
    - aparelho tem `PlayHere` e `Volume`;
    - o cartão tem `Shuffle`, `Repeat` e `Logout`;
    - `Back15` só aparece no cartão com episódio.
  - `no_active_device_opens_devices`, do Foco 4: `sent(Err(NoActiveDevice))` dá a mensagem exata e
    empilha `List{source: Devices}` com um `Load` dela.
  - `premium_and_rate_limit_messages`: `RateLimited(7)` dá "Muitas consultas; tentando de novo em 7 s".
  - `add_to_playlist_lists_only_editable_and_toasts`: escolher "Mix" dá
    `Send(playlist_add("P","spotify:track:T"))`; depois de `sent(Ok)`, o aviso é "Adicionada a Mix".
  - `space_with_empty_field_toggles_play`: tocando dá `pause()`; pausado dá `play(Resume)`.
  - `ctrl_tab_cycles_filters_and_researches`.
  - `scroll_near_end_loads_next_page_once`: dois `wheel` seguidos geram só um `Load`.
  - `catalog_and_controller_know_spotify`: no arquivo de testes do controlador e do catálogo.
- [ ] **Passo 2:** `cargo test -p zefiro-bar` falha.
- [ ] **Passo 3:** implementar.
- [ ] **Passo 4:** os testes passam.
- [ ] **Passo 5:** commit `feat(bar): spotify panel state`.

### Tarefa 7: desenho do painel

**Arquivos:**
- Mudar: `crates/modules/bar/src/spotify.rs`.

**Interfaces:**
- Consome: `SpotifyPanel` e o `Search` do VS Code como padrão do campo.
- Produz:
  - `pub enum Hit { Row(usize), Filter(Filter), Back, MenuItem(usize), Card(CardButton), ClientField, CopyRedirect, SaveClientId, Login }`
  - `pub enum CardButton { Previous, PlayPause, Next, Like }`
  - `pub fn spotify_panel(state: &SpotifyPanel, covers: &dyn Fn(&str) -> Option<Image>, now_ms: u64, measure: &dyn TextMeasurer, scale: f32, tokens: &Tokens) -> (DisplayList, Vec<(Rect, Hit)>)`
  - Largura `400.0` DIP, linhas de `44.0`, capa de `36.0` e no máximo 10 linhas visíveis.
  - O progresso do cartão é `progress_ms + (now_ms - player_at_ms)` quando está tocando.

- [ ] **Passo 1: testes que falham.**
  - `root_shows_card_entries_and_recent`: com os textos exatos "Sua biblioteca", "Fila",
    "Aparelhos", "Tocadas há pouco" e "A seguir: X".
  - `nothing_playing_says_so`: `player: None` dá "Nada tocando", do Foco 3.
  - `field_hint_changes_with_screen`: "Buscar no Spotify…" na raiz e "Filtrar Foco no código…" dentro
    da lista.
  - `search_groups_have_labels_and_durations`: "3:36" a partir de 216000 ms.
  - `cover_placeholder_until_loaded`: sem capa, a linha tem um `RoundRect` no lugar; com capa, um
    `DrawOp::Image`.
  - `menu_overlays_and_hits_its_items`.
  - `setup_screens`:
    - `NeedsClientId` mostra o Redirect URI, "Copiar", o campo e "Salvar";
    - `NeedsLogin` mostra "Entrar com o Spotify";
    - `PortBusy` mostra "A porta 47823 está em uso".
  - `empty_list_says_no_results`: `Some(vec![])` dá "Nenhum resultado"; `None` dá "Carregando…", do
    Foco 3.
  - `progress_advances_with_time_while_playing`.
  - `footer_shows_shortcuts_or_toast`.
- [ ] **Passo 2:** os testes falham.
- [ ] **Passo 3:** implementar.
- [ ] **Passo 4:** os testes passam.
- [ ] **Passo 5:** commit `feat(bar): draw spotify panel`.

### Tarefa 8: plataforma (HTTP com corpo, DPAPI, aleatório, listener)

**Arquivos:**
- Mudar:
  - `Cargo.toml` (workspace): a feature `Win32_Security_Cryptography` da `windows`;
  - `crates/platform-windows/src/http.rs`;
  - `crates/platform-windows/src/lib.rs`.
- Criar:
  - `crates/platform-windows/src/secret.rs`;
  - `crates/platform-windows/src/spotify_auth.rs`, com o listener;
  - os testes em `crates/platform-windows/tests/spotify_platform.rs`.

**Interfaces:**
- Produz:
  - `http::send(method: &str, url: &str, headers: &[(&str, &str)], body: Option<(&str, &str)>, timeout: Duration) -> Result<(u16, String, Option<u64>)>`.
    O `get_with_retry_after` passa a chamar ela com `"GET"`.
  - `secret::protect(data: &[u8]) -> windows::core::Result<Vec<u8>>` e `secret::unprotect`, com
    `CryptProtectData` no escopo do usuário e sem interface.
  - `secret::random<const N: usize>() -> [u8; N]`, com `BCryptGenRandom` e
    `BCRYPT_USE_SYSTEM_PREFERRED_RNG`.
  - `spotify_auth::listen(state: String, timeout: Duration, done: impl FnOnce(ListenResult) + Send + 'static) -> std::io::Result<()>`
    - `ListenResult { Code(String), Denied, TimedOut }`;
    - erro de `bind` volta direto, como `AddrInUse`;
    - responde `auth::DONE_PAGE` e fecha;
    - ignora pedidos `NotCallback`, como o favicon;
    - um `WrongState` também é ignorado.
  - `pub struct HttpTransport;`, que implementa `zefiro_integration_spotify::session::Transport` sobre
    `http::send` com 15 s de tempo limite.

- [ ] **Passo 1: testes que falham.**
  - `send_puts_method_headers_and_body`: um `TcpListener` em `127.0.0.1:0` responde
    `HTTP/1.1 204` e guarda a requisição; `send("PUT", …, Some(("application/json","{}")))` faz o
    servidor receber `PUT`, `Content-Type: application/json` e o corpo, e devolve 204.
  - `send_reads_retry_after`: 429 com `Retry-After: 7` dá `Some(7)`.
  - `dpapi_round_trips_and_is_not_plain`.
  - `random_is_not_constant`.
  - `listener_returns_code_and_serves_done_page`: `listen` numa porta livre e, nos testes, a porta
    passada por parâmetro. Assinatura de teste:
    `listen_on(port: u16, state, timeout, done)`; o `listen` usa `auth::PORT`.
  - `listener_reports_port_busy`.
  - `listener_times_out`, com um tempo limite de 200 ms.
- [ ] **Passo 2:** `cargo test -p zefiro-platform-windows --test spotify_platform` falha.
- [ ] **Passo 3:** implementar.
- [ ] **Passo 4:** os testes passam.
- [ ] **Passo 5:** commit `feat(platform-windows): http send, dpapi, spotify callback listener`.

### Tarefa 9: thread `zefiro-spotify`

**Arquivos:**
- Criar: `crates/platform-windows/src/spotify.rs`.

**Interfaces:**
- Consome: `Session`, `exchange_code`, `CoverCache`, `HttpTransport`, `secret`, `spotify_auth::listen`,
  `imaging::decode_bytes`, `http::get_bytes_with` e `system::open_uri`.
- Produz:
  - `pub enum Job`:
    - `Login`, `Logout`, `SetClientId(String)`;
    - `Player`, `Recent`, `Queue`;
    - `Load { depth: usize, source: Source, offset: u32, after: Option<String> }`;
    - `Search { seq: u64, query: String, kinds: Vec<Kind>, offset: u32 }`;
    - `Send(Request)`, `ToggleLike(String)`, `CheckLiked(String)`;
    - `Playlists`;
    - `Cover { url: String, side: u32 }`.
  - `pub enum Reply`:
    - `Auth(AuthState)`, com `AuthState { NeedsLogin, Waiting, PortBusy, Ready }`;
    - `Player(Result<Option<Player>, ApiError>)`;
    - `Recent(Vec<Item>)`, `UpNext(Option<String>)`;
    - `Loaded { depth: usize, offset: u32, result: Result<Option<Page>, ApiError> }`;
    - `Searched { seq: u64, offset: u32, result: Result<Vec<(Kind, Page)>, ApiError> }`;
    - `Sent(Result<(), ApiError>)`, `Liked { uri: String, liked: bool }`;
    - `Playlists(Vec<Item>)`, `Cover`, `Idle`.
  - `pub struct SpotifyService`, com:
    - `pub fn start(dir: PathBuf, client_id: String, notify: Arc<dyn Fn() + Send + Sync>) -> std::io::Result<SpotifyService>`:
      `dir` é `%LOCALAPPDATA%\zefiro\spotify`;
    - `pub fn send(&self, job: Job)`;
    - `pub fn replies(&self) -> Vec<Reply>`;
    - `pub fn set_panel_open(&self, open: bool)`;
    - `pub fn cover(&self, url: &str) -> Option<Image>`: lê o cache compartilhado;
    - o `Drop` para a thread.
- **Regras da thread:**
  - **Ao acordar:** junta todos os jobs da fila e roda `Send`, `ToggleLike` e `Login`/`Logout`
    primeiro, depois o resto, e as capas por último. Uma `Search` só roda se o `seq` dela for o mais
    novo.
  - **Painel aberto:** `recv_timeout(5 s)` e, a cada vez que vencer, um `Job::Player`. Fechado, fica em
    `recv()`, sem tempo limite.
  - **10 min fechado:** limpa as capas e manda `Reply::Idle`.
  - **429:** manda o `RateLimited(n)` e espera n s (no máximo 60) antes do próximo pedido.
  - **`Unauthorized` depois da renovação:** apaga o `token.bin` e manda
    `Reply::Auth(NeedsLogin)`.
  - **`take_new_refresh()`:** depois de cada pedido; se trouxer token, grava o `token.bin` cifrado.
  - **`SetClientId`:** quando o `client_id` muda, apaga o `token.bin` e manda `NeedsLogin`.
  - **`Login`:** são 96 bytes aleatórios, `pkce`, `listen` e `open_uri(authorize_url)`.
    - O `ListenResult::Code` volta para a fila da thread como um job interno.
    - O fluxo segue com `exchange_code`, a gravação, o `GET /me` e `Ready`.
    - A porta ocupada dá `PortBusy`.
  - **`Recent`:** `recent_contexts` e um `GET` para cada contexto (`parse_one`).
  - **`Queue`:** manda `UpNext` com o nome da 1ª próxima.
  - **`Playlists`:** lê `/me/playlists`, todas as páginas, e devolve só as `editable`.
  - **`ToggleLike`:** `library_contains`, depois `library_save` ou `library_remove`, e manda
    `Liked`. O `CheckLiked` só faz o `contains`.
  - **Log:** só o que aconteceu ("spotify: busca, 10 resultados em 230 ms", "spotify: tocar (faixa)",
    "spotify: 429, esperando 7 s" e "spotify: renovação recusada, saindo"). Nunca a query nem o
    token.

- [ ] **Passo 1:** a lógica da fila fica testável numa função pura `fn order(jobs: Vec<Job>, newest_seq: u64) -> Vec<Job>`,
  com o teste `jobs_run_player_commands_first_and_skip_stale_searches` em `#[cfg(test)]` no próprio
  arquivo. Escrever o teste.
- [ ] **Passo 2:** `cargo test -p zefiro-platform-windows spotify` falha.
- [ ] **Passo 3:** implementar o serviço.
- [ ] **Passo 4:** os testes passam e `cargo clippy -p zefiro-platform-windows --all-targets -- -D warnings` fica limpo.
- [ ] **Passo 5:** commit `feat(platform-windows): spotify service thread`.

### Tarefa 10: sonda `spotify_probe` e fixtures reais

**Arquivos:**
- Criar: `crates/platform-windows/examples/spotify_probe.rs`.
- Mudar: `crates/integrations/spotify/tests/fixtures/*.json`.

**O que a sonda faz:**
- `cargo run -p zefiro-platform-windows --example spotify_probe -- [--login] [--dump <pasta>] [busca]`
- Lê o `client_id` do `config.toml` (`ZEFIRO_CONFIG_DIR` vale) e o `token.bin`.
- `--login` roda o fluxo de login e grava o token.
- Imprime o estado do player, os aparelhos e 3 resultados da busca, nunca o token.
- `--dump` grava as respostas cruas de `search`, `player`, `queue`, `devices`, `playlists`,
  `playlist_items` (uma sua), `playlist_metadata_only` (um Daily Mix) e `recently_played`.

- [ ] **Passo 1:** com o usuário presente, e só depois de pedir um momento, criar o app no
  developer.spotify.com com o Redirect URI exato, gravar o `client_id`, rodar `--login` e depois
  `--dump`.
- [ ] **Passo 2:** limpar os dumps antes de copiar: tirar e-mail, nome de exibição, ids de
  usuário (trocar por `me`/`outro`) e ids de aparelho.
- [ ] **Passo 3:** substituir as fixtures equivalentes e rodar `cargo test -p zefiro-integration-spotify`.
  Se algum teste quebrar, o leitor está errado: corrigir o leitor, não a fixture.
- [ ] **Passo 4:** commit `test(spotify): real API fixtures and probe`.

### Tarefa 11: ligação no app

**Arquivos:**
- Criar: `crates/app/src/spotify_loop.rs`.
- Mudar:
  - `crates/app/src/main.rs`: `mod spotify_loop`;
  - `event_loop.rs`, com estes pontos:
    - `USER_SPOTIFY = 28`, `TIMER_SPOTIFY_TICK` (1 s) e `TIMER_SPOTIFY_SEARCH` (300 ms);
    - o campo `spotify: SpotifyRuntime`;
    - `PanelKind::Spotify` no `panel_list`;
    - `spotify_prepare` e `spotify_panel_closed` nos efeitos;
    - a rota em `Key` (antes do `vscode_key`), `Char` (antes do `vscode_char`), `LeftUp`, `RightUp`,
      `Move` e `Wheel`;
  - `bar_view.rs`: glifo `"spotify" => "\u{EC4F}"`;
  - `system_loop.rs`: `self.spotify_services()` no `refresh_services`.

**Interfaces:**
- Consome tudo das Tarefas 5 a 9.
- Produz `impl Loop`:
  - `spotify_services`: sobe ou derruba o `SpotifyService` conforme `BarConfig::uses("spotify")`;
    `SetClientId` quando o config muda;
  - `spotify_prepare(kind)` e `spotify_panel_closed()`;
  - `spotify_panel_list(scale) -> DisplayList`;
  - `spotify_key(vk) -> Result<bool>`, que lê `ctrl_down` e `shift_down` para montar `Key`;
  - `spotify_char(code) -> Result<bool>`;
  - `spotify_click(p, right: bool) -> Result<bool>`;
  - `spotify_move(p)`;
  - `spotify_wheel(delta) -> Result<bool>`;
  - `spotify_replies()`, chamado no `USER_SPOTIFY`;
  - `spotify_tick()`.
- **Regras:**
  - **`Effect` → serviço:** cada `Effect` vira um `Job`. `Copy` usa
    `system_events::set_clipboard_text` e o aviso "Copiado". `SaveClientId` usa
    `zefiro_core::config::set_spotify`. `Login` e `Logout` vão para o serviço.
  - **Busca:** fica num `pending_search` e é disparada pelo `TIMER_SPOTIFY_SEARCH` de 300 ms,
    rearmado a cada tecla.
  - **Tique de 1 s:** como o `island_tick`, só com o painel aberto, a raiz à vista e música tocando.
    Ele só redesenha o progresso: é conteúdo novo uma vez por segundo, não animação (ver *Ruling* no
    ledger).
  - **Capas:** ao desenhar, cada URL sem capa no cache gera `Job::Cover`, uma vez por URL. O retorno
    `Reply::Cover` redesenha.
  - **Avisos do rodapé:** somem com o próximo redesenho depois de 2 s, reaproveitando o tique.
  - **Depois de cada `Sent(Ok)`:** pedir `Player` e `Queue`.
  - **Diagnóstico:** o "Exportar diagnóstico" já leva só logs, crashes, config e `info.txt`, então o
    `token.bin` fica de fora sem mudança. O `client_id` no config não é segredo. Conferir no zip na
    verificação da tela.

- [ ] **Passo 1:** `cargo build -p zefiro-app` e `cargo clippy --workspace --all-targets -- -D warnings`
  limpos.
- [ ] **Passo 2:** `pwsh -NoProfile -File scripts/check-windows-imports.ps1` passa.
- [ ] **Passo 3:** na tela, com o usuário avisado antes, já que reinicia a barra: o roteiro de
  verificação do spec, os passos de 1 a 10. Anotar no ledger o que passou.
- [ ] **Passo 4:** commit `feat(app): spotify chip panel`.

### Tarefa 12: medição, ADR e docs

**Arquivos:**
- Criar: `docs/adr/0024-chip-spotify.md`, a partir do `0000-modelo.md`.
- Mudar:
  - `AGENTS.md`: a linha `crates/integrations/spotify` na tabela de estrutura; a lista de ids do
    `[bar.sections]` ganha `spotify`; um item em "Coisas não óbvias" com PKCE, a porta 47823, o DPAPI
    e a API de 2026;
  - `docs/PLANNING.md`, se o roteiro listar chips.

- [ ] **Passo 1:** fechar o YASB e pôr `[island] enabled = false` num perfil isolado
  (`ZEFIRO_CONFIG_DIR`), com o chip na barra.
  - Rodar `./scripts/perf.ps1 -Process zefiro` sem o chip, com o chip e o painel fechado, e com o
    painel aberto e rolado por 1 min.
  - Orçamento: no máximo +1 MB fechado e +6 MB aberto, e 0 % de CPU com o painel fechado.
  - Reabrir o YASB no fim.
- [ ] **Passo 2:** ADR com:
  - o contexto (API de 2026, Premium, 5 usuários);
  - as decisões (PKCE com Client ID próprio, DPAPI, porta fixa, nenhuma consulta com o painel
    fechado, painel estilo Raycast, sem ✓ de playlist e sem atalho global);
  - os números medidos.
- [ ] **Passo 3:** commit `docs(adr): spotify chip`.
- [ ] **Passo 4:** revisão da branch inteira contra o spec, como manda o fluxo do `AGENTS.md`.
  Corrigir os Critical e os Important, cada um com um teste que falhe antes. Anotar os Minor no
  ledger.
