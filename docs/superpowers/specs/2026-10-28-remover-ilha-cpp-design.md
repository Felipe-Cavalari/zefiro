# Remover a ilha C++ e o motor externo

- **Data:** 2026-10-28
- **Status:** aprovada (2026-10-28)

## Por quê

O repositório ainda carrega o fork em C++ do Dynamic Island for Windows (`island/`, cerca de 25 mil linhas, quase tudo num `dynamic-island-for-windows.wh.cpp`), contra cerca de 40 mil linhas de Rust. Desde a ADR-0009 a ilha padrão é a embutida em Rust, e o fork só roda com `[island] engine = "externa"`. Mesmo assim, pelo volume de C++ e pela primeira frase do AGENTS.md ("a ilha de mídia é um fork em C++"), quem analisa o repositório, gente ou IA, lê o Zefiro como um fork da ilha. A configuração do usuário não tem a chave `engine`, então ele já usa só a ilha em Rust.

## O que sai

**Código:**

- `island/` inteira (o fork, o `wh_shim`, os scripts de build e a `island/LICENSE`).
- O crate `crates/modules/island-host` (`island.ini`, `island-state.ini`, política de reinício e processo), dos membros e das dependências do workspace.
- `crates/app/src/island.rs` (ciclo de vida do `zefiro-island.exe`) e as chamadas a ele no `event_loop` (`start`, `stop`, `sync`, `restart`).
- `crates/platform-windows/src/island_zorder.rs` e as chamadas a `raise_island()` (barra, robustez, bandeja, temporizador `TIMER_RAISE`).
- `window::island_reservation`, a mensagem registrada da geometria da ilha e o `WinEvent::IslandGeometryChanged`.
- O destaque lido do `island-state.ini` (`refresh_island_accent`, `USER_ISLAND_ACCENT` e o observador do `island-state.ini` no `main.rs`). A ilha embutida continua publicando o destaque da capa direto pelo `island_loop`.
- `USER_ISLAND`, "Reiniciar a ilha" no painel de Configurações (`SettingsAction::RestartIsland`) e no menu do ícone da bandeja. Com a ilha embutida, esses botões não fazem nada.
- As regras que só existiam por causa da externa: centro travado no modo de edição (`island_locked`/`center_locked`), a reserva no centro do principal no `bar_view` e o "Ocultar automaticamente · indisponível com a ilha externa".

**Config (`crates/core`):**

- Saem `IslandEngine`, `IslandConfig::external()` e o campo `exe`. `[island] enabled` continua.
- Uma config antiga com `engine = "externa"` ou `exe = "…"` continua abrindo: as chaves são ignoradas e o log avisa uma vez ("a ilha externa foi removida; usando a ilha embutida"). O Zefiro não grava nada no arquivo do usuário por causa disso.
- O modelo do `config.toml` perde as linhas `engine` e `exe`.

**Empacotamento e scripts:**

- `installer/zefiro.iss`: sai a cópia do `zefiro-island.exe` e a espera de 3 s pela ilha ao fechar. Entra um `[InstallDelete]` do `{app}\zefiro-island.exe`, para quem atualizar não ficar com o exe antigo.
- `scripts/package.ps1`: sai o `-BuildIsland` e a cópia do exe.
- `scripts/stability.ps1` e o teste dele: saem os eventos do `zefiro-island.exe`.
- `.github/workflows/ci.yml`: sai o `-p zefiro-island-host`.

**Não muda:**

- O crate `zefiro-island` (a ilha em Rust), os chips `island`, `player`, `weather` e `hardware`, as abas, os avisos e as letras.
- Os arquivos do usuário: `island.ini` e `island-state.ini` em `%APPDATA%\zefiro` ficam onde estão (regra 5 do AGENTS.md). Só deixam de ser lidos.
- As ADRs, specs e planos antigos continuam como histórico.

## Recursos que só a ilha C++ tinha

Conferido no código em 2026-10-28, a partir da lista da ADR-0009:

| Recurso | Na ilha em Rust? |
|---|---|
| Aviso de brilho | sim (`Activity::Brightness`, pelo DDC/CI) |
| Pesquisa rápida (Ctrl+Alt+Espaço; já falhava nesta máquina, erro 1409) | não |
| Anel de progresso externo (`ProgressPercent`) | não |
| Bandeja de arquivos | não (estava desligada no `island.ini`) |
| Sobreposição de jogo | não (estava desligada) |
| Bateria fraca | não (o PC é desktop) |
| Fixar expandido e o atalho Ctrl+Alt+D | não |
| Letras na pílula recolhida | não (as letras ficam na aba) |
| Duas pílulas lado a lado (`Split`, um aviso ao lado da mídia) | não: o aviso toma a pílula por alguns segundos |
| Visualizadores LED, plasma e orbe | não: só o de barras. O analisador de espectro (`analyzer.rs`) foi portado, mas não está ligado a nenhum estilo |
| As 8 paletas próprias da ilha | não: a ilha em Rust usa os temas do Zefiro |

Esses recursos deixam de existir. Nenhum deles é usado hoje.

## Créditos

- **README:** a linha do Dynamic Island for Windows sai do topo da tabela de créditos e vira uma linha curta de inspiração ("a ilha de mídia do Zefiro foi inspirada no Dynamic Island for Windows…"), sem citar `island/` nem "base da ilha".
- **`THIRD_PARTY_NOTICES.md`:** a seção "Código portado" continua, porque a ilha em Rust portou parte da lógica do projeto original (molas, visualizador, cor da capa e regras visuais). A licença MIT exige manter o aviso de copyright nessas partes. O texto da `island/LICENSE` passa para esse arquivo, já que a pasta sai.
- **AGENTS.md:** a primeira frase passa a dizer que o Zefiro é todo em Rust. Saem a linha `island/` da tabela de estrutura, os comandos de build da ilha e as notas da ilha externa ("Fork da ilha", "Ilha externa e temas novos", "Barra ↔ ilha externa").
- **ADR nova (0026):** registra a remoção e marca a ADR-0006 como substituída. A ADR-0009 ganha uma nota de que a externa saiu.

## Testes

- `crates/core`: uma config com `engine = "externa"` e `exe = "x"` abre sem erro e com a ilha embutida (o teste falha antes, porque hoje vira `external()`).
- `crates/modules/bar`: o painel de Configurações não tem mais "Reiniciar a ilha", e "Ocultar automaticamente" não depende mais da ilha.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` e `scripts/check-windows-imports.ps1`.
- Na tela: a barra abre com a ilha no chip, o modo de edição deixa mexer no centro e Sair fecha sem esperar a ilha.

## Riscos

- Alguém que usava `engine = "externa"` perde os recursos da tabela acima sem aviso na tela, só no log. Hoje o único usuário com config conhecida usa a embutida.
- O histórico do git continua com o fork. Quem olhar commits antigos ainda vê a ilha C++, o que é normal e correto.
