# Passagem da Fase 1a para o Codex

Este documento passa a execução da Fase 1a (barra Zefiro em Rust com o design da ilha) do Claude para o Codex. Ele é atualizado ao fim de cada tarefa. A seção **Estado atual** é a fonte da verdade sobre onde o trabalho parou.

## Onde está tudo

- **Worktree:** `C:\Users\felip\Projects\zefiro\.claude\worktrees\spike-s6-island-fork-6f5cda`. Trabalhe só aqui.
- **Branch:** `fase-1a/barra`. Nunca faça `git push`.
- **Spec (a autoridade):** `docs/superpowers/specs/2026-10-15-fase-1a-barra-design.md`.
- **Plano (25 tarefas, com o código de cada passo):** `docs/superpowers/plans/2026-10-15-fase-1a-barra.md`.
- **Ledger:** `.superpowers/sdd/2026-10-15-fase-1a-barra/progress.md`. A pasta é ignorada pelo git, então só existe nesta máquina.
  - Uma tarefa com a linha `Task N: complete` está feita e não deve ser refeita.
  - As linhas `Ruling:` registram cada desvio do plano e o motivo.
- **Briefs por tarefa:** `.superpowers/sdd/2026-10-15-fase-1a-barra/task-N-brief.md`. Contêm o trecho do plano de cada tarefa.

## Regras do projeto

- Docs e mensagens em pt-BR; identificadores em inglês.
- **Regra de ouro:** a crate `windows` só pode aparecer em três lugares:
  - `zefiro-platform-windows`;
  - `zefiro-integration-clock`, apenas em `windows.rs`;
  - `zefiro-app`.

  O script `scripts/check-windows-imports.ps1` verifica isso.
- As crates portáveis produzem dados: config, tokens de tema, retângulos de layout, display lists e regras de painel. A `platform-windows` desenha e anima.
- Cada desvio do plano vira uma linha no ledger, no formato `Task N: Ruling: <o que decidiu> — <por quê> — <custo se errado>`.
- O YASB deve estar fechado antes de medir e precisa ser reaberto no fim.
- Commits em inglês no estilo `feat(app): ...`, um commit por tarefa, como o plano manda.
- Não apague nada em `%APPDATA%\zefiro\codex-backup\`, que guarda os arquivos da barra antiga do Codex. Ignore aquele trabalho antigo, que não faz parte desta fase.
- O e-mail do usuário nunca deve ir para serviço nenhum.

## Como verificar

| O quê | Comando |
| --- | --- |
| Testes de tudo | `cargo test --workspace` |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` |
| Rodar a barra | `cargo build -p zefiro-app` e depois `.\target\debug\zefiro.exe` |
| Parar a barra | `Get-Process zefiro \| Stop-Process` |

Ao rodar a barra:

- O config fica em `%APPDATA%\zefiro\config.toml`.
- Os logs ficam em `%LOCALAPPDATA%\zefiro\logs`.
- Trocar `[bar] height` para `completa`/`compacta` e `format` para `faixa`/`pilulas` testa as 4 combinações.

Antes de recompilar a ilha (`island/`), pare o `zefiro-island.exe`; senão o link falha porque o exe está travado.

## Estado atual (2026-10-15)

### Tasks 1–23: concluídas e commitadas

O último commit da Task 23 é `cbd533c`.

Verificado na tela:

- **Barra:** as 4 combinações de altura/formato.
- **Calendário:** abre a partir do relógio; setas e roda trocam o mês com a altura animando; Esc fecha e devolve o foco.
- **Menu:** abre por clique e por botão direito numa pílula; as setas movem o foco; altura, formato e tema aplicam na hora, o menu continua aberto e acompanha a barra.
- **Troca menu → calendário:** a forma é redirecionada sem esperar.
- **Relógio:** vira no minuto.
- **Sair:** encerra o app.

### Decisões da Task 23 que mudam o que vem depois

Os detalhes estão nas linhas `Task 23: Ruling` do ledger.

**Duas janelas por barra e por painel** (`crates/platform-windows/src/window.rs`):

- A janela de desenho é `WS_EX_LAYERED | WS_EX_TRANSPARENT`, então todo clique a atravessa.
- Uma janela de entrada invisível (classe `ZefiroInput`), dona-dependente da de desenho, recebe o mouse. Ela é recortada por `SetWindowRgn` nos retângulos de `set_interactive`.
- Os eventos da janela de entrada chegam com o `hwnd` da janela de desenho.
- `window::input_window(hwnd)` devolve a janela de entrada.
- O motivo: `HTTRANSPARENT` não repassa clique para outros processos, e a margem da sombra engolia cliques.
- Na Task 24, quem recriar barras ou painéis (troca de monitor ou DPI) precisa destruir as duas janelas.

**A janela do painel cobre o monitor inteiro**, e a forma vive sempre no mesmo espaço. Trocar de painel só redireciona a mola.

**Molas:** `SpringParams::compositor()` usa `period = 1/ω`. Foi medido na tela; com 2π/ω as molas ficavam 4–5× lentas.

**Assinaturas que mudaram em relação ao plano:**

- `PanelView::close(ctx, restore_focus)`;
- `PanelView::resize(ctx, chip, band_bottom, list)`;
- `PanelView::has_focus()`;
- `Comp::fade_after`.

**Pendente para a Task 24:** o `cargo clippy -D warnings` acusa código morto que a Task 24 passa a usar:

- as constantes `USER_CONFIG_CHANGED`, `USER_THEMES_CHANGED`, `TIMER_DISPLAY` e `TIMER_RAISE`;
- `reposition_appbar`;
- `AppCtx.gfx`;
- `PanelView::size`.

Ao fim da Task 24 o clippy tem de passar.

**Ferramentas de teste da UI:** estão fora do repo, no scratchpad do Claude, e servem só de referência:

- `tools/ui.ps1`: clicar, teclar, rolar e capturar com tempo;
- `probe.ps1`: diz qual janela recebe o clique num ponto.

O Codex pode recriar algo equivalente.

**Bug conhecido (minor):** fechar, reabrir e fechar de novo em menos de ~0,3 s faz o primeiro `USER_PANEL_SETTLED` esconder o painel no meio da segunda animação. É raro; um contador de geração resolveria.

### Task 24 (ilha, recarga ao vivo, robustez): concluída

O commit é `1b0052d`.

Verificado na tela:

- **Ilha:** fica no centro e segue altura, formato e tema; muda junto com o tema escolhido no menu ou no `config.toml`.
- **Política de reinício:** ao matar a ilha 4 vezes, ela reinicia 3 vezes e depois desiste.
- **Reiniciar a ilha:** a opção do menu traz a ilha de volta.
- **Exe ausente:** sem o `zefiro-island.exe`, a barra abre normalmente e o log avisa.
- **Tema do usuário:** com erro, a barra mantém a versão anterior e o log mostra a linha; apagar o arquivo volta ao tema embutido.
- **Trocas rápidas de painel e formato:** nada fica preso aberto nem invisível.
- **Clippy:** passa no workspace inteiro.

**Não executado de verdade (fica para a validação do usuário na Task 25):**

- reiniciar o Explorer (havia 3 janelas do Explorador abertas);
- Win+P;
- desligar "Efeitos de animação".

Os caminhos de código foram exercitados assim: `TaskbarCreated` e 3× `WM_DISPLAYCHANGE` postados na barra produziram uma única recriação, sem vazar janelas.

**Achado para a revisão final (ilha/upstream):**

- A janela da ilha só fica click-through quando está Idle (`SetClickThrough`, `island/dynamic-island-for-windows.wh.cpp:23121`).
- Mostrando mídia, a margem dela, por volta de y 44–100 no centro, engole cliques nas janelas de baixo.

### Task 25: não começada

Siga o brief. A Task 25 inclui:

- as medições: `perf.ps1`, primeiro quadro, e `frames.ps1` com `ZEFIRO_CYCLES`;
- o ADR-0007, que deve registrar estes ajustes de API:
  - `USER_DEFAULT`;
  - `CreateBitmap`;
  - a sombra concêntrica no lugar de `DropShadow`;
  - `Send` em `on_settled`;
  - `WM_MOUSELEAVE` local;
  - a feature `Win32_Security`;
  - as janelas de entrada recortadas por região;
  - `period = 1/ω`;
  - o backup dos arquivos do Codex;
- as atualizações de PLANNING e README;
- a validação visual com o usuário: as 4 combinações e os três testes reais da Task 24.

### Revisão final

Depois da Task 25, faça uma revisão da branch inteira (`git merge-base main HEAD`..HEAD) contra a spec. Confira com atenção cada item da seção **Review Focus** do plano.

Classifique os achados pelo efeito para quem usa a barra:

- **Critical e Important:** corrija numa única passada. Cada correção precisa de um teste que falhe antes e passe depois, e a suíte inteira precisa ficar verde.
- **Minor:** vai para o ledger como `Final: minor (deferred): ...`.

No fim, entregue ao usuário duas listas: "Rulings I made", com todas as linhas `Ruling:` do ledger, e "Deferred minors".

## Scripts do fluxo

Os scripts usados no fluxo (superpowers) são bash e rodam no Git Bash.

| Script | Caminho |
| --- | --- |
| `task-start` | `C:/Users/felip/.claude/plugins/cache/claude-plugins-official/superpowers/6.4.1/skills/executing-plans/scripts/task-start PLANO N` |
| `task-done` | `.../executing-plans/scripts/task-done PLANO N BASE -- <comando de teste>` |
| `review-package` | `.../subagent-driven-development/scripts/review-package PLANO MERGE_BASE HEAD` |

`PLANO` é `docs/superpowers/plans/2026-10-15-fase-1a-barra.md`.

`task-start` imprime o caminho do brief e o BASE da tarefa. `task-done` só grava a linha `complete` no ledger se o comando de teste passar. Se preferir não usar os scripts, acrescente as linhas no ledger à mão, no mesmo formato.

A próxima é a Task 25, com BASE `1b0052d`. Rode `task-start` para obter o brief.
