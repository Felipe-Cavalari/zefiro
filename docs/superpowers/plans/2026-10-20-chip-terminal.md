# Chip Terminal — plano

> **Para agentes:** SUB-SKILL OBRIGATÓRIA: use superpowers:subagent-driven-development (recomendado) ou
> superpowers:executing-plans para executar este plano tarefa por tarefa. Os passos usam caixas
> (`- [ ]`) para acompanhar.

**Objetivo:** um chip `terminal` cujo painel lista os perfis do Windows Terminal, abre o escolhido e,
pelo lápis, deixa esconder e ordenar os perfis, gravando a escolha em `[terminal]` no `config.toml`.

**Arquitetura:** uma crate portável nova (`zefiro-integration-terminal`) lê o `settings.json` e os
fragmentos do Terminal e aplica `order`/`hidden`; o `zefiro-bar` guarda o estado de edição e desenha
os dois modos do painel; a `platform-windows` acha a pasta do pacote, o `wt.exe` e decodifica os
ícones; o `zefiro-app` junta tudo num `terminal_loop.rs`, no molde do `vscode_loop.rs`.

**Stack:** Rust 2024, `serde_json`, `toml_edit`, WinRT `BitmapDecoder`, Win32 Appx
(`GetPackagesByPackageFamily`, `GetPackagePathByFullName`).

**Spec:** `docs/superpowers/specs/2026-10-20-chip-terminal-design.md` (leia junto).

Ledger: `.superpowers/sdd/2026-10-20-chip-terminal/progress.md`. Verificação de cada tarefa:
`cargo test -p <crate>`; no fim, `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings` e
`pwsh -NoProfile -File scripts/check-windows-imports.ps1`.

## Restrições globais

- Crate `windows` só em `crates/platform-windows`, `crates/integrations/clock/src/windows.rs` e
  `crates/app`. A crate nova e o `zefiro-bar` não a usam.
- `clippy::all = deny`; `unsafe` só em blocos pequenos com `// SAFETY:`.
- Textos de interface, comentários e logs em pt-BR; identificadores e commits em inglês
  (`feat(terminal): …`, `feat(bar): …`, `feat(app): …`, `docs(adr): …`).
- Textos exatos: título "Terminal"; subtítulo "Perfis do Windows Terminal" (normal) e "Arraste para
  ordenar · o olho esconde" (edição); botão "Concluir"; rótulo "Ocultos"; selo "padrão"; mensagens
  "Carregando…", "Windows Terminal não encontrado", "Não consegui ler as configurações do Windows
  Terminal", "Nenhum perfil visível. Toque no lápis para escolher."
- Catálogo: id `terminal`, nome "Terminal", descrição "Perfis do Windows Terminal; o lápis escolhe e
  ordena". Glifo do chip `E756`; lápis `E70F`.
- Nada lê arquivo nem decodifica ícone na thread de UI (a gravação do `config.toml` é a exceção, como
  a do menu do Z). O Zefiro nunca grava nos arquivos do Windows Terminal.
- `guid`s sempre comparados e gravados em minúsculas, com chaves (`{…}`).

## Pontos de revisão

1. **`wt.exe` é um alias de execução (reparse point):** `Path::exists()` dá `false` nele. Conferir
   com `std::fs::symlink_metadata(..).is_ok()`; senão o painel diria "não encontrado" com o Terminal
   instalado. → Task 7 (função) e Task 10 (conferência na tela).
2. **JSONC com `//` dentro de string** (`"commandline": "ssh //host"`, URL em `"backgroundImage"`) e
   aspas escapadas: tirar comentário dentro de string quebraria o arquivo inteiro. → Task 1.
3. **`guid` em maiúsculas** no `settings.json` ou num fragmento contra minúsculas no `config.toml`:
   a escolha não pode "sumir". → Tasks 1 e 2.
4. **`defaultProfile` pelo nome** em vez do `guid` (o Terminal aceita os dois). → Task 1.
5. **Ícone que não carrega** (emoji, glifo, arquivo apagado, `ms-appx` sem a variante de escala): cai
   no próximo candidato e por fim no glifo; nunca linha vazia nem lista travada. → Tasks 2 e 8.

---

### Task 1: Ler perfis do `settings.json` e dos fragmentos

**Files:**
- Create: `crates/integrations/terminal/Cargo.toml` (como o do `vscode`, com `serde_json`; dev:
  `tempfile`), `crates/integrations/terminal/src/lib.rs`, `crates/integrations/terminal/src/jsonc.rs`
- Create: `crates/integrations/terminal/tests/fixtures/settings.json` e
  `.../fixtures/wsl-fragment.json` — cópias do `settings.json` e do fragmento do WSL do usuário
  (`%LOCALAPPDATA%\Packages\Microsoft.WindowsTerminal_8wekyb3d8bbwe\LocalState\settings.json` e
  `%LOCALAPPDATA%\Microsoft\Windows Terminal\Fragments\Microsoft.WSL\{05554e26-…}.json`), com
  `C:\Users\felip` trocado por `C:\Users\teste`. Acrescentar ao `settings.json` da fixture um
  comentário `// …`, um `/* … */` e uma vírgula final.
- Modify: `Cargo.toml` (membro e `zefiro-integration-terminal` em `[workspace.dependencies]`)

**Interfaces — produz:**
```rust
pub fn jsonc::strip(text: &str) -> String; // sem BOM, sem comentários, sem vírgulas finais; strings intactas
#[derive(Debug, Clone, PartialEq)]
pub enum IconSource { File(PathBuf), Package(String) /* caminho relativo, ex. "ProfileIcons/pwsh.png" */, None }
#[derive(Debug, Clone, PartialEq)]
pub struct Profile { pub guid: String /* minúsculas, com chaves */, pub name: String, pub icon: IconSource, pub default: bool }
pub fn parse_profiles(settings: &str, fragments: &[&str], env: &dyn Fn(&str) -> Option<String>) -> Result<Vec<Profile>, String>;
```

Regras (da spec): `profiles` lista ou `{ "list": […] }`; fragmento completa o perfil de mesmo `guid`
ou o de `updates`; o `settings.json` vence; perfil só de fragmento entra no fim, na ordem dos
fragmentos recebidos; entrada de fragmento sem `guid` nem `updates` é ignorada; `hidden: true` no
resultado final some; perfil sem `name` some. `icon`: `%VAR%` expandido por `env`;
`ms-appx:///x` → `Package("x")`; caminho absoluto terminado em `.png`/`.ico`/`.jpg`/`.jpeg`/`.bmp` →
`File`; resto → `None`. `default` = `defaultProfile` igual ao `guid` (sem caixa) ou ao `name`.

- [ ] **Passo 1: testes que falham** (em `lib.rs` e `jsonc.rs`)

```rust
#[test] fn strip_keeps_slashes_inside_strings() {
    let t = "{\"a\": \"ssh //host /* x */\", // c\n \"b\": \"\\\"q\\\"\", /* d */ \"c\": [1,2,],}";
    let v: serde_json::Value = serde_json::from_str(&jsonc::strip(t)).unwrap();
    assert_eq!(v["a"], "ssh //host /* x */");
    assert_eq!(v["b"], "\"q\"");
    assert_eq!(v["c"], serde_json::json!([1, 2]));
}
#[test] fn strip_drops_bom() { assert!(serde_json::from_str::<serde_json::Value>(&jsonc::strip("\u{feff}{}")).is_ok()); }

#[test] fn user_settings_give_the_taskbar_profiles() {
    let p = parse_profiles(SETTINGS, &[WSL], &no_env).unwrap();
    let names: Vec<_> = p.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["PowerShell", "Windows PowerShell", "Prompt de comando", "Azure Cloud Shell", "Ubuntu-24.04"]);
    assert!(p.iter().all(|p| p.guid != "{963ff2f7-6aed-5ce3-9d91-90d99571f53a}"), "a cópia antiga do Ubuntu está oculta pelo fragmento");
    assert_eq!(p.iter().filter(|p| p.default).map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Ubuntu-24.04"]);
    assert_eq!(p[4].icon, IconSource::File(r"C:\Users\teste\AppData\Local\wsl\{54e7dab8-3953-4a07-9e15-9c58dc528819}\shortcut.ico".into()));
}
#[test] fn object_form_and_uppercase_guids() {
    let s = r#"{"defaultProfile":"Cmd","profiles":{"defaults":{},"list":[{"guid":"{AB-CD}","name":"Cmd"}]}}"#;
    let p = parse_profiles(s, &[], &no_env).unwrap();
    assert_eq!(p[0].guid, "{ab-cd}");
    assert!(p[0].default, "defaultProfile pelo nome");
}
#[test] fn settings_win_over_fragment_and_fragment_only_goes_last() {
    let s = r#"{"profiles":[{"guid":"{a}","name":"Meu"}]}"#;
    let f = r#"{"profiles":[{"updates":"{A}","name":"Do fragmento","icon":"C:\\i.png"},{"guid":"{b}","name":"Novo"},{"name":"Sem guid"}]}"#;
    let p = parse_profiles(s, &[f], &no_env).unwrap();
    assert_eq!((p[0].name.as_str(), &p[0].icon), ("Meu", &IconSource::File(r"C:\i.png".into())));
    assert_eq!(p.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Meu", "Novo"]);
}
#[test] fn icon_sources() {
    let env = |k: &str| (k == "USERPROFILE").then(|| r"C:\Users\t".to_string());
    let s = r#"{"profiles":[{"guid":"{1}","name":"a","icon":"ms-appx:///ProfileIcons/pwsh.png"},
        {"guid":"{2}","name":"b","icon":"%USERPROFILE%\\x.ico"},{"guid":"{3}","name":"c","icon":"🐧"},{"guid":"{4}","name":"d"}]}"#;
    let p = parse_profiles(s, &[], &env).unwrap();
    assert_eq!(p[0].icon, IconSource::Package("ProfileIcons/pwsh.png".into()));
    assert_eq!(p[1].icon, IconSource::File(r"C:\Users\t\x.ico".into()));
    assert_eq!((&p[2].icon, &p[3].icon), (&IconSource::None, &IconSource::None));
}
#[test] fn broken_json_is_an_error() { assert!(parse_profiles("{", &[], &no_env).is_err()); }
```
(`SETTINGS`/`WSL` = `include_str!` das fixtures; `no_env = |_: &str| None`.)

- [ ] **Passo 2:** `cargo test -p zefiro-integration-terminal` → falha (não compila).
- [ ] **Passo 3:** implementar `jsonc::strip` (máquina de estados: dentro/fora de string, escape,
  `//`, `/* */`; vírgula seguida só de espaço e `]`/`}` é removida) e `parse_profiles`.
- [ ] **Passo 4:** `cargo test -p zefiro-integration-terminal` → passa.
- [ ] **Passo 5:** commit `feat(terminal): parse Windows Terminal profiles and fragments`.

### Task 2: Ordem, ocultos, ícones candidatos e leitura do disco

**Files:** Modify `crates/integrations/terminal/src/lib.rs`

**Interfaces — consome:** `Profile`, `IconSource`, `parse_profiles` (Task 1). **Produz:**
```rust
/// (visíveis na ordem escolhida, ocultos na ordem do Terminal), índices em `profiles`.
pub fn arrange(profiles: &[Profile], order: &[String], hidden: &[String]) -> (Vec<usize>, Vec<usize>);
/// (`order`, `hidden`) para gravar; mantém de `previous_hidden` os guids que `profiles` não conhece.
pub fn saved_lists(profiles: &[Profile], visible: &[usize], hidden: &[usize], previous_hidden: &[String]) -> (Vec<String>, Vec<String>);
/// Caminhos a tentar, em ordem (spec "Ícones" 1 e 2); vazio = glifo.
pub fn icon_candidates(profile: &Profile, package_dir: Option<&Path>) -> Vec<PathBuf>;
pub fn open_args(profile: &Profile) -> Vec<String>; // ["-p", guid]
#[derive(Debug, Clone, PartialEq)] pub enum LoadError { NotInstalled, Unreadable(String) }
/// Lê o primeiro `settings.json` (Stable, Preview, sem pacote) e os fragmentos (`*\*.json`, em ordem de caminho).
pub fn read_profiles(local_app_data: &Path, program_data: &Path) -> Result<Vec<Profile>, LoadError>;
```

- [ ] **Passo 1: testes que falham**

```rust
fn p(guid: &str) -> Profile { Profile { guid: guid.into(), name: guid.into(), icon: IconSource::None, default: false } }
#[test] fn new_profiles_go_last_and_hidden_follow_terminal_order() {
    let ps = [p("{a}"), p("{b}"), p("{c}"), p("{d}")];
    let (v, h) = arrange(&ps, &["{C}".into(), "{a}".into()], &["{d}".into(), "{b}".into()]);
    assert_eq!((v, h), (vec![2, 0], vec![1, 3]));
}
#[test] fn no_section_shows_everything() { assert_eq!(arrange(&[p("{a}"), p("{b}")], &[], &[]), (vec![0, 1], vec![])); }
#[test] fn unknown_hidden_guids_survive_saving() {
    let ps = [p("{a}"), p("{b}")];
    let (order, hidden) = saved_lists(&ps, &[1], &[0], &["{gone}".into(), "{a}".into()]);
    assert_eq!(order, ["{b}"]);
    assert_eq!(hidden, ["{a}", "{gone}"]);
}
#[test] fn icon_candidates_in_order() {
    let dir = Path::new(r"C:\WT");
    let mut q = p("{x}"); q.icon = IconSource::Package("ProfileIcons/pwsh.png".into());
    assert_eq!(icon_candidates(&q, Some(dir)), [dir.join("ProfileIcons/pwsh.scale-200.png"), dir.join("ProfileIcons/pwsh.scale-100.png"),
        dir.join("ProfileIcons/pwsh.png"), dir.join("ProfileIcons/{x}.scale-200.png"), dir.join("ProfileIcons/{x}.scale-100.png"), dir.join("ProfileIcons/{x}.png")]);
    assert!(icon_candidates(&p("{x}"), None).is_empty(), "emoji ou nada, sem pacote: glifo");
}
#[test] fn opens_by_guid() { assert_eq!(open_args(&p("{a}")), ["-p", "{a}"]); }
#[test] fn reads_stable_settings_and_fragments_from_disk() {
    // tempdir com Packages\Microsoft.WindowsTerminal_8wekyb3d8bbwe\LocalState\settings.json (fixture)
    // e Microsoft\Windows Terminal\Fragments\Microsoft.WSL\x.json (fixture): 5 perfis, Ubuntu com ícone.
}
#[test] fn missing_terminal_and_unreadable_settings() {
    // tempdir vazio → Err(NotInstalled); settings.json = "{" → Err(Unreadable(_)).
}
```

- [ ] **Passo 2:** `cargo test -p zefiro-integration-terminal` → os novos falham.
- [ ] **Passo 3:** implementar; `read_profiles` usa `std::env::var` para o `env` de `parse_profiles`
  e procura fragmentos em `<local>\Microsoft\Windows Terminal\Fragments` e
  `<program_data>\Microsoft\Windows Terminal\Fragments`.
- [ ] **Passo 4:** testes passam.
- [ ] **Passo 5:** commit `feat(terminal): profile order, hidden list, icon candidates and disk reading`.

### Task 3: Seção `[terminal]` no config

**Files:** Modify `crates/core/src/config.rs`

**Produz:**
```rust
#[derive(Debug, Clone, PartialEq, Deserialize, Default)] #[serde(default)]
pub struct TerminalConfig { pub order: Vec<String>, pub hidden: Vec<String> }
// Config ganha `pub terminal: TerminalConfig` (Default incluso).
impl Config { pub fn same_except_terminal(&self, other: &Config) -> bool; }
/// Grava `[terminal] order` e `hidden` de uma vez, com `toml_edit`, mantendo o resto do arquivo.
pub fn set_terminal(path: &Path, terminal: &TerminalConfig) -> std::io::Result<()>;
```

- [ ] **Passo 1: testes que falham**

```rust
#[test] fn terminal_section_is_optional_and_parsed() {
    assert_eq!(parse_config(DEFAULT_CONFIG).unwrap().terminal, TerminalConfig::default());
    let c = parse_config("[terminal]\norder = [\"{a}\"]\nhidden = [\"{b}\"]\n").unwrap();
    assert_eq!(c.terminal, TerminalConfig { order: vec!["{a}".into()], hidden: vec!["{b}".into()] });
}
#[test] fn set_terminal_keeps_comments_and_other_keys() {
    // tempdir com DEFAULT_CONFIG; set_terminal duas vezes (a segunda troca os valores);
    // o texto ainda contém "# completa | compacta", parse dá os valores da segunda e `theme` intacto.
}
#[test] fn only_terminal_changed() {
    let a = Config::default();
    let mut b = a.clone(); b.terminal.hidden.push("{x}".into());
    assert!(a.same_except_terminal(&b));
    b.theme = "cobalto".into();
    assert!(!a.same_except_terminal(&b));
}
```

- [ ] **Passo 2:** `cargo test -p zefiro-core` → falha.
- [ ] **Passo 3:** implementar (`set_terminal` no molde de `set_value`, gravando arrays de strings).
- [ ] **Passo 4:** `cargo test -p zefiro-core` → passa.
- [ ] **Passo 5:** commit `feat(core): terminal section with profile order and hidden list`.

### Task 4: Estado de edição do painel (`zefiro-bar`)

**Files:** Create `crates/modules/bar/src/terminal.rs`; Modify `crates/modules/bar/src/lib.rs`

**Produz:**
```rust
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Arrangement { pub visible: Vec<usize>, pub hidden: Vec<usize> } // índices de perfil; `hidden` sempre crescente (ordem do Terminal)
#[derive(Debug, Clone, Copy, PartialEq)] pub enum Slot { Visible(usize), Hidden }
impl Arrangement {
    pub fn toggle(&mut self, profile: usize);           // olho: visível → ocultos; oculto → fim dos visíveis
    pub fn move_to(&mut self, profile: usize, slot: Slot); // soltar do arraste; `Visible(i)` = posição entre os visíveis sem o arrastado
    pub fn nudge(&mut self, profile: usize, up: bool);  // Ctrl+↑/↓ entre os visíveis; oculto não move
    /// Linhas na ordem da tela (visíveis e depois ocultos), para as setas.
    pub fn rows(&self) -> Vec<usize>;
}
```
*(Ruling da spec: "o olho manda para o fim de Ocultos" vira "Ocultos sempre na ordem do Terminal",
como diz a seção do modo de edição; registrar no ledger.)*

- [ ] **Passo 1: testes que falham**

```rust
fn a() -> Arrangement { Arrangement { visible: vec![0, 1, 3], hidden: vec![2] } }
#[test] fn eye_hides_and_shows() {
    let mut x = a(); x.toggle(1);
    assert_eq!(x, Arrangement { visible: vec![0, 3], hidden: vec![1, 2] });
    x.toggle(2);
    assert_eq!(x, Arrangement { visible: vec![0, 3, 2], hidden: vec![1] });
}
#[test] fn drag_reorders_hides_and_shows() {
    let mut x = a(); x.move_to(3, Slot::Visible(0));
    assert_eq!(x.visible, [3, 0, 1]);
    x.move_to(0, Slot::Hidden);
    assert_eq!((x.visible.clone(), x.hidden.clone()), (vec![3, 1], vec![0, 2]));
    x.move_to(2, Slot::Visible(1));
    assert_eq!((x.visible, x.hidden), (vec![3, 2, 1], vec![0]));
}
#[test] fn nudge_moves_within_visible_only() {
    let mut x = a(); x.nudge(3, true);
    assert_eq!(x.visible, [0, 3, 1]);
    x.nudge(0, true); x.nudge(2, false);
    assert_eq!((x.visible.clone(), x.hidden.clone()), (vec![0, 3, 1], vec![2]));
}
#[test] fn hiding_everything_is_allowed() {
    let mut x = a(); for i in [0, 1, 3] { x.toggle(i); }
    assert_eq!((x.visible.len(), x.hidden), (0, vec![0, 1, 2, 3]));
}
```

- [ ] **Passo 2:** `cargo test -p zefiro-bar terminal` → falha.
- [ ] **Passo 3:** implementar.
- [ ] **Passo 4:** passa.
- [ ] **Passo 5:** commit `feat(bar): terminal profile arrangement editing`.

### Task 5: Desenho e áreas de clique do painel (`zefiro-bar`)

**Files:** Modify `crates/modules/bar/src/terminal.rs`

**Consome:** `Arrangement`, `Slot` (Task 4). **Produz:**
```rust
pub struct ProfileRow { pub name: String, pub icon: Option<Image>, pub default: bool }
pub struct DragView { pub profile: usize, pub point: Point, pub slot: Slot }
pub struct PanelView<'a> {
    pub rows: &'a [ProfileRow], pub arrangement: &'a Arrangement, pub editing: bool,
    pub message: Option<&'a str>, pub selected: Option<usize> /* perfil */, pub hover: Option<usize> /* perfil */,
    pub drag: Option<&'a DragView>, pub scroll: usize,
}
pub struct PanelLayout {
    pub list: DisplayList, pub pencil: Rect,
    pub rows: Vec<(Rect, usize)>, pub eyes: Vec<(Rect, usize)>,
    pub visible_zone: Rect, pub hidden_zone: Rect, // só na edição; `Rect::default()` no normal
}
pub const VISIBLE: usize = 12;
pub fn max_scroll(count: usize) -> usize;
pub fn terminal_panel(view: &PanelView, scale: f32, tokens: &Tokens) -> PanelLayout;
/// Onde o arrastado cairia com o mouse em `p` (sobre "Ocultos" → `Hidden`; senão a posição entre os visíveis).
pub fn drop_slot(layout: &PanelLayout, arrangement: &Arrangement, dragged: usize, p: Point) -> Slot;
```
Medidas *(chamada minha, a spec deixa com quem constrói)*: largura 300 DIP, linha 36, topo das linhas
62 (título 14, subtítulo 34), ícone 18 com raio 4, lápis 28×28 no canto superior direito, olho
`E7B3` (visível) / `ED1A` (oculto), alça `E76F` em `text_tertiary`, ocultos com opacidade 0,4,
rótulo "Ocultos" 10,5 SemiBold `text_tertiary`. Sem ícone: glifo `E756` em `text_secondary`. A
linha inteira arrasta. Rolagem (`scroll`, até `VISIBLE`) só no modo normal; na edição todas as
linhas aparecem *(ruling: a spec só fala de rolagem no modo normal)*. Arraste: linha original com
opacidade 0,35, cópia em `raised_strong` com contorno `accent` seguindo `point`, traço `accent` de
2,5 DIP no `slot`; zona de destino em `accent_soft`.

- [ ] **Passo 1: testes que falham**

```rust
#[test] fn normal_mode_lists_visible_rows_with_default_badge() {
    // 4 linhas, arrangement visible [3,0], hidden [1,2], default na 0.
    // textos contêm "Terminal", "Perfis do Windows Terminal", nomes 3 e 0, "padrão"; não contêm os ocultos;
    // rows == [(_,3),(_,0)]; eyes vazio; tem um Text com "\u{E70F}" (lápis); largura 300.
}
#[test] fn editing_shows_hidden_rows_eyes_and_done() {
    // mesmo caso com editing: textos "Concluir", "Arraste para ordenar · o olho esconde", "Ocultos";
    // rows cobre [3,0,1,2]; eyes.len() == 4; altura maior que a do modo normal; hidden_zone abaixo de visible_zone.
}
#[test] fn row_without_icon_gets_the_terminal_glyph() { /* icon None → Text "\u{E756}"; com Some → DrawOp::Image */ }
#[test] fn messages_replace_the_list() {
    // message Some("Carregando…") → texto presente, rows vazio. Nenhum visível e message None, modo normal →
    // "Nenhum perfil visível. Toque no lápis para escolher."
}
#[test] fn drop_slot_follows_the_mouse() {
    // edição com visible [0,1,2], hidden [3]: ponto no meio da 1ª linha arrastando 2 → Visible(0);
    // abaixo da última visível → Visible(2); dentro de hidden_zone → Hidden.
}
#[test] fn scrolls_after_twelve_in_normal_mode() { /* 20 visíveis, scroll 3 → rows.len()==12, rows[0].1==visible[3]; max_scroll(20)==8 */ }
```

- [ ] **Passo 2:** `cargo test -p zefiro-bar terminal` → falha.
- [ ] **Passo 3:** implementar.
- [ ] **Passo 4:** passa.
- [ ] **Passo 5:** commit `feat(bar): terminal panel drawing in normal and edit modes`.

### Task 6: Catálogo, tipo de painel e glifo do chip

**Files:** Modify `crates/modules/bar/src/catalog.rs`, `crates/modules/bar/src/controller.rs`,
`crates/app/src/bar_view.rs` (`icon_glyph` → `"terminal" => "\u{E756}"`; `"hardware" | "power" |
"terminal"` no `level_chip`)

**Produz:** `PanelKind::Terminal`; `panel_kind_for("terminal") == Some(PanelKind::Terminal)`.

- [ ] **Passo 1:** acrescentar `"terminal"` à lista de `new_items_are_in_the_catalog` e um teste no
  `controller` para `panel_kind_for("terminal")`.
- [ ] **Passo 2:** `cargo test -p zefiro-bar` → falha.
- [ ] **Passo 3:** item no catálogo (logo depois de `vscode`), variante e mapeamento; no `panel_list`
  do `event_loop.rs`, `PanelKind::Terminal => DisplayList::new(Size { w: 1.0, h: 1.0 })` provisório
  só para compilar (a Task 8 troca).
- [ ] **Passo 4:** `cargo test -p zefiro-bar` e `cargo build -p zefiro-app` passam.
- [ ] **Passo 5:** commit `feat(bar): terminal chip in the catalog`.

### Task 7: Pacote do Terminal, `wt.exe` e ícones de arquivo (`platform-windows`)

**Files:** Create `crates/platform-windows/src/windows_terminal.rs`; Modify
`crates/platform-windows/src/lib.rs`, `crates/platform-windows/src/imaging.rs`

**Produz:**
```rust
// windows_terminal.rs
/// Pasta de instalação do pacote (Stable, depois Preview) por `GetPackagesByPackageFamily` + `GetPackagePathByFullName`.
pub fn package_dir() -> Option<PathBuf>;
/// `%LOCALAPPDATA%\Microsoft\WindowsApps\wt.exe` se o alias existir (`symlink_metadata`, Ponto de revisão 1).
pub fn wt_exe() -> Option<PathBuf>;
// imaging.rs
/// Arquivo (`.png`, `.ico`, `.jpg`, `.bmp`) por `StorageFile::GetFileFromPathAsync` + `decode_stream`, em `side` px. Bloqueante.
pub fn decode_file(path: &Path, side: u32) -> Option<Image>;
```
Famílias: `Microsoft.WindowsTerminal_8wekyb3d8bbwe`, `Microsoft.WindowsTerminalPreview_8wekyb3d8bbwe`.

- [ ] **Passo 1:** teste `#[ignore]` em `windows_terminal.rs` que imprime `package_dir()`, `wt_exe()`
  e o tamanho de `decode_file(<pacote>\ProfileIcons\{0caa0dad-35be-5f56-a8ff-afceeeaa6101}.scale-200.png, 36)`;
  rodar com `cargo test -p zefiro-platform-windows windows_terminal -- --ignored --nocapture` → falha (não existe).
- [ ] **Passo 2:** implementar.
- [ ] **Passo 3:** o mesmo comando mostra a pasta `…WindowsTerminal_1.24…`, o `wt.exe` e `36x36`.
- [ ] **Passo 4:** `cargo clippy -p zefiro-platform-windows -- -D warnings` passa.
- [ ] **Passo 5:** commit `feat(platform-windows): Windows Terminal package folder, wt alias and file icons`.

### Task 8: Painel no modo normal (`zefiro-app`)

**Files:** Create `crates/app/src/terminal_loop.rs`; Modify `crates/app/src/main.rs`,
`crates/app/src/event_loop.rs`, `crates/app/Cargo.toml` (`zefiro-integration-terminal`)

**Consome:** `read_profiles`, `arrange`, `icon_candidates`, `open_args`, `LoadError` (Tasks 1–2);
`terminal_panel`, `PanelView`, `ProfileRow`, `Arrangement`, `max_scroll`, `VISIBLE` (Tasks 4–5);
`package_dir`, `wt_exe`, `decode_file` (Task 7).

**Produz:** `pub const USER_TERMINAL: u32 = 24;` e, em `impl Loop`:
`terminal_prepare(kind)`, `terminal_loaded()`, `terminal_panel_list(scale) -> DisplayList`,
`terminal_key(vk) -> Result<bool>`, `terminal_wheel(delta) -> Result<bool>`,
`terminal_up(p) -> Result<bool>`, `terminal_panel_closed()`. `TerminalRuntime` no `Loop`
(campo `terminal`).

Comportamento: ao abrir, thread `zefiro-terminal` faz `read_profiles`, `package_dir`, `wt_exe` e, por
perfil, o primeiro `decode_file(c, round(18 × escala))` que der certo entre os `icon_candidates`;
devolve tudo por `Arc<Mutex<Option<…>>>` + `post_user(USER_TERMINAL)`. Mensagens: lendo →
"Carregando…"; `NotInstalled` ou sem `wt.exe` → "Windows Terminal não encontrado"; `Unreadable` →
"Não consegui ler as configurações do Windows Terminal" + `tracing::warn!`. Arranjo =
`arrange(perfis, &config.terminal.order, &config.terminal.hidden)`. Clique numa linha ou Enter →
`spawn_detached(wt, open_args)` (falha → `warn!`), fecha o painel. ↑/↓ e roda como no VS Code.
Ligar em `panel_list`, `run_effects` (Open → `terminal_prepare`; Close → `terminal_panel_closed`),
`LeftUp` (antes do `vscode_click`), `Key` (antes do `vscode_key`), `Wheel` e `User`.

- [ ] **Passo 1:** `cargo build -p zefiro-app` e `cargo clippy -p zefiro-app -- -D warnings` passam.
- [ ] **Passo 2:** sonda na tela (pedir um momento ao usuário, Regra 6): com um perfil isolado
  (`ZEFIRO_CONFIG_DIR`) e `terminal` em `[bar.sections] end`, abrir o painel e conferir os 5 perfis
  com ícones e o selo "padrão" no Ubuntu (o Azure aparece: sem `[terminal]` tudo é visível); clicar
  em "Prompt de comando" abre o Windows Terminal nesse perfil e fecha o painel.
- [ ] **Passo 3:** commit `feat(app): terminal panel lists and opens Windows Terminal profiles`.

### Task 9: Modo de edição e gravação (`zefiro-app`)

**Files:** Modify `crates/app/src/terminal_loop.rs`, `crates/app/src/event_loop.rs`

**Consome:** `Arrangement::{toggle, move_to, nudge, rows}`, `drop_slot`, `DragView` (Tasks 4–5);
`saved_lists` (Task 2); `set_terminal`, `TerminalConfig`, `Config::same_except_terminal` (Task 3).

**Produz:** `terminal_down(panel, p) -> Result<bool>`, `terminal_move(p) -> Result<bool>`; o
`TerminalRuntime` ganha `editing: bool`, `before_edit: Arrangement`, `press: Option<(usize, Point)>`,
`drag: Option<DragView>`, `hover: Option<usize>`.

Comportamento (spec "modo de edição"):
- Lápis → `editing = true`, `before_edit = arrangement.clone()`. "Concluir" (o mesmo retângulo) ou
  Enter → grava e sai.
- `LeftDown` numa linha na edição: guarda `press` e `window::capture`. `MouseMove` com `press` e
  mais de 4 DIP de distância → `drag` (com `drop_slot`); sem `press`, atualiza `hover` (redesenha só
  se mudou). `LeftUp`: com `drag` → `move_to`; sem `drag`, no olho → `toggle`; na linha → nada.
  Sempre `release_capture`.
- Teclas na edição: ↑/↓ escolhem em `rows()`; Ctrl (`GetKeyState(0x11) < 0`, em `platform-windows`
  como `system_events::ctrl_down()`) + ↑/↓ → `nudge`; Espaço (`0x20`) → `toggle`; Esc → volta a
  `before_edit` e sai da edição **sem fechar o painel**.
- Gravar = `saved_lists` → se diferente do `config.terminal` atual, `set_terminal(&ctx.config_path, …)`
  (erro → `warn!`, a edição na tela continua valendo até fechar). *(Ruling: grava direto com
  `toml_edit` só na `[terminal]`, lendo o arquivo atual — o mesmo efeito da "relê e reaplica só
  `[terminal]`" da spec, sem passar pelo `config_worker`.)*
- `terminal_panel_closed` com `editing` → grava antes de limpar (fechar no meio grava).
- `USER_CONFIG_CHANGED`: se `new.same_except_terminal(&old)`, só troca `ctx.config` e, com o painel
  do Terminal aberto e fora da edição, refaz o arranjo e redesenha — **sem** `PanelInput::ConfigChanged`
  (senão "Concluir" fecharia o painel).

- [ ] **Passo 1:** a lógica já tem testes (Tasks 2–5); aqui é só ligação. `cargo build -p zefiro-app`
  e `cargo clippy --workspace --all-targets -- -D warnings` passam.
- [ ] **Passo 2:** sonda na tela (pedir um momento): lápis; arrastar o Ubuntu para o topo; esconder o
  Azure pelo olho; Concluir → painel continua aberto, normal, na ordem nova, sem o Azure; o
  `config.toml` do perfil isolado tem `[terminal] order` e `hidden` e os comentários originais;
  editar de novo e apertar Esc → nada muda; editar e clicar fora → grava; reiniciar a barra pelo
  "Sair" e abrir de novo → mesma ordem.
- [ ] **Passo 3:** commit `feat(app): edit, reorder and hide terminal profiles in the panel`.

### Task 10: Documentação e verificação final

**Files:** Create `docs/adr/0015-chip-terminal.md` (modelo `0000-modelo.md`: perfis lidos do
Terminal sem gravar nele, chave por `guid`, edição dentro do painel como primeiro caso de
configuração fora do "Editar barra", recarga do config que não fecha o painel, `wt.exe` como alias);
Modify `AGENTS.md` (crate nova na tabela de estrutura; `terminal` na lista de ids do
`[bar.sections]`; seção `[terminal]` em "Arquivos da barra em uso"), `docs/PLANNING.md` se o
roteiro listar os chips.

- [ ] **Passo 1:** `cargo test --workspace` → tudo passa.
- [ ] **Passo 2:** `cargo clippy --workspace --all-targets -- -D warnings` → sem avisos.
- [ ] **Passo 3:** `pwsh -NoProfile -File scripts/check-windows-imports.ps1` → ok (a crate nova não
  usa `windows`).
- [ ] **Passo 4:** conferir na tela, com o Terminal instalado pelo pacote, que o `wt.exe` é achado
  (Ponto de revisão 1). O glifo de reserva já está coberto pelos testes das Tasks 2 e 5.
- [ ] **Passo 5:** commit `docs(adr): terminal chip and in-panel profile editing`.
- [ ] **Passo 6:** revisão da branch inteira contra a spec (AGENTS.md, fluxo 5); Critical/Important
  corrigidos com teste; Minor no ledger; lista de rulings e minors para o usuário.
