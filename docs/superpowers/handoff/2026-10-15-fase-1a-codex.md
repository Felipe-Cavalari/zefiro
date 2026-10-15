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

### Tasks 1–22: concluídas e commitadas

O último commit é `cfc5994`.

Verificado na tela:

- **Faixa/completa:** barra Obsidian, "Z Zefiro" à esquerda, relógio à direita, janelas começando abaixo da barra.
- **Pílulas/compacta:** duas pílulas flutuantes de 32 DIP com sombra.
- **Primeiro quadro:** 59–75 ms em 2 monitores.

### Task 23 (painéis, menu, calendário e relógio): código escrito, não commitado

Arquivos:

- **Novos:** `crates/app/src/panel_view.rs`, `crates/app/src/event_loop.rs`, `crates/app/src/island.rs` (stub) e `crates/app/src/robustness.rs` (stub).
- **Alterado:** `crates/app/src/main.rs`, que agora chama `event_loop::run`.

Compila, mas ainda não foi testado na tela.

Ainda falta:

1. Rodar e conferir os itens do "Step 3" do brief da Task 23:
   - o calendário abre a partir do relógio;
   - o menu abre a partir de "Z Zefiro";
   - Compacta/Pílulas/tema animam a barra e o menu continua aberto;
   - a troca menu → calendário acontece sem esperar;
   - o relógio vira no minuto.
2. Registrar no ledger as duas decisões já tomadas na Task 23:
   - **Recorte:** o conteúdo do painel fica dentro de uma `ContainerVisual` (`layer`) do tamanho da janela, e o recorte (`clip_child`) e a opacidade (`bind_alpha`) são aplicados nela, não no sprite. O clipe de um visual fica no espaço do próprio visual. Com o sprite deslocado até o painel, o recorte sairia deslocado.
   - **Unused:** os imports não usados (`HashMap`, `Monitor`) foram removidos do código do plano.
3. Fazer o commit com a mensagem do brief e depois rodar `task-done` (ver **Scripts** abaixo).

### Pontos em aberto da Task 23 (investigar e decidir)

**A. Cliques na margem transparente.**

- A janela da barra tem 24 DIP de margem abaixo da faixa, que existe para a sombra.
- O hit-test devolve `HTTRANSPARENT` fora dos retângulos interativos.
- A documentação do Windows diz que `HTTRANSPARENT` só repassa o clique para janelas da mesma thread.
- Teste:
  1. Com a barra aberta, clique logo abaixo da faixa, em cima de outra janela.
  2. Se o clique se perder, troque a estratégia de passagem de clique. Por exemplo, ligue e desligue `WS_EX_TRANSPARENT | WS_EX_LAYERED` conforme o mouse entra ou sai dos retângulos interativos, ou reduza a janela à faixa e desenhe a sombra em outra janela.
  3. A mesma questão vale para a janela do painel.

**B. Redirecionar a forma entre painéis.**

- A spec pede que, ao trocar de painel no mesmo monitor (fecha um e abre outro), a forma seja redirecionada e a mola parta do valor atual.
- Hoje `PanelView::place` reposiciona a janela (`window_bounds` muda) e, se o painel estava fechado ou fechando, coloca a forma no chip novo com um salto.
- Uma saída: fixar a origem da janela do painel na origem do monitor, para que as coordenadas da forma não mudem. Só vale a pena depois de resolver o ponto A.
- Revise também a condição `if !was_open || old_window != self.window && !opening` em `place`. No `resize` com a janela mudando de lugar, ela recoloca a forma no chip, e o correto é partir do painel antigo convertido para o novo espaço.

### Tasks 24 e 25: não começadas

Siga os briefs. A Task 25 inclui:

- as medições: `perf.ps1`, primeiro quadro, e `frames.ps1` com `ZEFIRO_CYCLES`;
- o ADR-0007, que deve registrar estes ajustes de API:
  - `USER_DEFAULT`;
  - `CreateBitmap`;
  - a sombra concêntrica no lugar de `DropShadow`;
  - `Send` em `on_settled`;
  - `WM_MOUSELEAVE` local;
  - a feature `Win32_Security`;
  - o backup dos arquivos do Codex;
- as atualizações de PLANNING e README;
- a validação visual das 4 combinações com o usuário.

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

BASE da Task 23: `cfc5994`. Comando de teste dela: `cargo build -p zefiro-app`.
