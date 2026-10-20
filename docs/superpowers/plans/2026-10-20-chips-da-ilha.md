# Chips da ilha — plano

Spec: `docs/superpowers/specs/2026-10-20-chips-da-ilha-design.md`. Ledger em
`.superpowers/sdd/2026-10-20-chips-da-ilha/progress.md`.

Verificação de cada tarefa: `cargo test -p <crate>`; no fim, `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings` e `scripts/check-windows-imports.ps1`.

## Task 1 — Estado e pílula do player (`zefiro-island`)

- `IslandState::player(has_media, hover, now) -> Collapsed`: Idle / Media / Peek, ignorando avisos,
  timer e o hover da ilha. `IslandModel::player(hover, now)`.
- `view::player_idle_pill(pill_h, tokens)`: pílula pequena com a nota (`E8D6`) apagada.
- Testes: avisos e timer não tomam o player; hover do chip espia; troca de faixa espia; sem mídia é
  parado; largura parada < mídia < espiando.

## Task 2 — Conjuntos de abas (`zefiro-island::tabs`)

- `TabSet { Island, Media, Weather, Hardware }` e `tabs::for_set(set, has_media, has_weather, has_lyrics)`.
- Testes: Island = `available`; Media vazio sem mídia; Weather vazio sem clima; Hardware sempre.

## Task 3 — Catálogo, painéis e chip de clima (`zefiro-bar`)

- Catálogo: `player`, `weather`, `hardware`.
- `PanelKind::{Player, Weather, Hardware}`, `panel_kind_for`, `PanelKind::tabbed()`.
- `chips::weather_chip(reading, …)`: emoji, temperatura com a largura de `-00°`, cidade apagada e
  cortada em 160 DIP; sem leitura, `--°`.
- Testes: mapeamento, largura estável entre temperaturas, corte da cidade, sem leitura.

## Task 4 — Teto de largura do chip (`zefiro-ui`)

- `ChipSpec { wide }`: chips largos (o player) não sofrem o teto de 160 DIP.
- Teste: um chip largo de 300 px mantém a largura; um comum continua com o teto.

## Task 5 — Barra (`zefiro-app::bar_view`)

- Conteúdo dos chips `weather` e `hardware`.
- Player: `IslandView` própria por barra no retângulo do chip, estilo faixa, hover por barra, sem
  fundo de hover/ativo; `fade_chip` esmaece a pílula; visualizador; largura muda → refaz o layout.

## Task 6 — Laço (`zefiro-app::island_loop` / `event_loop`)

- Serviços de mídia e clima seguem `embedded || uses(...)`; `refresh_services` chama `island_services`.
- Abas pelo tipo do painel aberto; painel do player fecha sem mídia; clique em player sem mídia ou
  clima sem leitura não abre nada; hover do player; captura com ilha ou player visível; clima novo
  redesenha as barras.

## Task 7 — Docs

- ADR-0012, AGENTS.md (catálogo) e PLANNING.md.
