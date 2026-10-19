# ADR-0011: Widgets do YASB que faltavam (fase 1e)

- **Status:** aceito
- **Data:** 2026-10-19
- **Branch:** `main` (commits `3156ba4` … `e673856`); spec em [`2026-10-19-fase-1e-widgets-do-yasb-design.md`](../superpowers/specs/2026-10-19-fase-1e-widgets-do-yasb-design.md)

## Contexto

O roteiro previa para a 1e os itens declarativos. O usuário redirecionou a fase: a base é a barra do YASB que o Zefiro substitui, e a fase deve trazer só os widgets que ainda faltam, sem mudar a barra atual e sem duplicar o que a ilha embutida já mostra (player, CPU/RAM).

Comparando o `~/.config/yasb/config.yaml` com o catálogo:

- **faltavam:** `claude_usage`, `codex_usage`, `vscode`, `taskbar`, `notifications`, `dnd` e `power_menu`;
- **ficaram fora:** `komorebi_*` e `whkd` (não estão rodando; o komorebi fica fora até a 1.0) e `pomodoro` (o timer de foco da ilha cobre).

## Opções consideradas

1. **Itens declarativos primeiro, widgets depois, como itens.** Era o roteiro, mas atrasaria os widgets que o usuário usa todo dia.
2. **Widgets nativos agora, com a lógica em crates portáveis.** Os declarativos ficam para a 1f e podem reaproveitar os mesmos parsers. Escolhida.

## Números medidos

Ambiente: Windows 11 Pro 26200, dois monitores 1920×1080 a 100 %, perfil do usuário (`compacta`/`faixa`, Obsidian, ilha embutida), build release, YASB fechado, 2026-10-19. `scripts/perf.ps1 -Process zefiro -WarmupSeconds 20 -SampleSeconds 60`.

| Métrica | Sem os chips novos (zefiro + relógio) | Com os 9 chips (1d + 1e) | Depois de abrir Claude, Codex, Energia e Brilho |
|---|---|---|---|
| Memória privada | **25,97 MB** | **29,89 MB** (+3,9) | 30,69 MB |
| Working set | 70,21 MB | 74,31 MB | 78,05 MB |
| CPU em repouso | 0,0 % | 0,0 % | 0,0 % |

A meta da spec (≤ +4 MB e CPU 0,0 %) foi cumprida. Pelo ADR-0010, os três chips da 1d respondem por cerca de 3 MB desses 3,9; os seis da 1e, por menos de 1 MB.

| Medida | Resultado |
|---|---|
| Ler os projetos do VS Code (27 entradas) e achar o `Code.exe` | ~10 ms |
| `GET /api/oauth/usage` (Claude) | 200, campos `five_hour`/`seven_day` com `utilization` 0–100 e `resets_at` ISO com fuso |

## Decisão

Seis chips nativos no catálogo: `claude`, `codex`, `vscode`, `notifications`, `dnd` e `power`. Os de dados têm painel que nasce do chip; Notificações e Não perturbe agem direto no clique. O botão do meio é novo e só tem efeito no sino (limpar).

### Arquitetura

| Camada | O que tem |
|---|---|
| `crates/integrations/usage` (portável) | Parser do Claude (`parse_claude`, `token_state` com `Debug` que esconde o token, `chip_percent`), leitor do Codex (`read_codex`, fim dos `.jsonl`, volta para sessões anteriores) com a thread `CodexService`, nomes das janelas e datas em pt-BR (`format_relative`, `format_absolute`, `parse_iso_utc`). |
| `crates/integrations/vscode` (portável) | `workspaceStorage\*\workspace.json` → nome, local, tipo (local, WSL, remoto, workspace) e argumentos do `Code.exe`. |
| `crates/modules/bar` | `usage.rs` (chip e painel de uso), `vscode.rs` (painel com rolagem), `power.rs` (painel e `ConfirmState`), `bell_chip` e `dnd_chip` em `chips.rs`, catálogo e `PanelKind`. |
| `crates/platform` | `power::PowerAction`, compartilhado pela barra e pela plataforma. |
| `crates/platform-windows` | `claude.rs` (thread `zefiro-claude`), `http::get_with` (cabeçalhos e tempo limite com cancelamento), `power.rs` (ações, tempo ligado, nome e foto da conta), contagem/limpeza/central no serviço de notificações, `dnd::open_settings`, `system::spawn_detached` e `find_code_exe`. |
| `crates/app` | `usage_loop.rs`, `vscode_loop.rs`, `power_loop.rs` e `system_loop.rs`, com `refresh_services()`, que liga e desliga os serviços conforme os chips estão na barra (`BarConfig::uses`). |

### Decisões de comportamento

- **Serviços só com o chip na barra** (correção da revisão). O token do Claude não é lido nem enviado e a pasta do Codex não é varrida sem o chip. Tudo é reavaliado ao salvar o arquivo e ao concluir o modo de edição.
- **Claude:**
  - O arquivo de credenciais é relido a cada consulta. O Zefiro nunca renova nem grava o token, porque isso derrubaria o login do Claude Code. Sem login ou com token vencido, o painel explica em duas linhas.
  - O serviço avisa a cada consulta, para o "Atualizado há" não envelhecer; a barra só é refeita quando o número muda. Numa falha de rede, o chip mantém o último valor bom.
- **Codex:**
  - Lê o fim do `.jsonl` mais novo de `~/.codex/sessions` e não abre credenciais.
  - Janela já reiniciada conta 0 % e aparece como "reiniciada". O rodapé mostra o último uso no fuso local.
- **Chips de uso com largura fixa.** Reservam "100%"; sem dado, mostram "—" apagado na mesma largura (ADR-0010).
- **VS Code pelo `workspaceStorage`.** A chave `history.recentlyOpenedPathsList` do `state.vscdb` (PLANNING) não existe na versão instalada; arquivos soltos ficam fora. O `Code.exe` é chamado direto, sem o `cmd.exe`, que interpretaria os `%` das URIs.
- **Notificações:**
  - O serviço saiu da ilha para a barra: liga se a ilha embutida está ativa ou se o sino está na barra.
  - O clique manda Win+N.
  - Para limpar, o `ClearNotifications` falha sem identidade de pacote (0x80070490); por isso as notificações são removidas uma a uma.
- **Não perturbe:** a leitura (WNF) roda sempre, independente da ilha. Alternar não foi validado (o usuário preferiu não mexer no estado dele), então o clique abre `ms-settings:notifications`.
- **Energia:**
  - Bloquear e Suspender agem no primeiro clique. Sair, Reiniciar e Desligar pedem um segundo clique entre 0,4 s e 3 s, para que um clique duplo não confirme.
  - Nas ações destrutivas, a barra sai como no "Sair" (libera as AppBars) e só então chama `ExitWindowsEx`, sem forçar os apps.
- **Chip Apps removido.** Ele foi feito (fixados do Windows + janelas por `SetWinEventHook`, ícones por janela, menu nativo) e retirado a pedido do usuário depois de ver na tela: misturar atalhos com apps abertos não ficou bom, e a barra de tarefas do Windows e o Raycast cobrem.
- **Na barra do usuário.** Os chips foram colocados no `config.toml` dele na ordem do YASB, com cópia em `config.toml.antes-1e`.

## Validação na tela

Perfil isolado (`ZEFIRO_CONFIG_DIR`) e, no fim, o perfil do usuário. Cliques por `PostMessage` na janela de entrada; quando foi preciso entrada real, `SendInput`.

1. **Codex:** chip e painel batem com a sonda (5 h reiniciada, 7 dias 19 %, último uso 15/10 23:56).
2. **Claude:**
   - Chip 2 % e painel com reinícios relativos e absolutos.
   - Sem credenciais aparece "Faça login no Claude Code"; com um arquivo inventado e vencido, a mensagem de login expirado. O arquivo real nunca foi tocado.
   - O rodapé se renova com o painel aberto por 2 min.
3. **VS Code:** a lista bate com a sonda, a roda rola, e o clique abre projeto local e do WSL.
4. **Energia** (modo `ZEFIRO_POWER_DRY_RUN=1`): o botão arma, desarma em 3 s, e o segundo clique registra a ação simulada; a barra sai.
5. **Notificações:**
   - O contador sobe com uma notificação de teste e a central abre.
   - Com `[island] enabled = false`, o contador continua funcionando.
   - Limpar zera o contador.
6. **Não perturbe:** a lua fica acesa com o modo ligado, e o clique abre as configurações.
7. **Serviços:** sem o chip, o Claude não liga; ao adicionar, liga; ao remover, desliga.

## Consequências

- **Roteiro:** a 1f passa a ser itens declarativos v1 + Modo noturno.
- **Pendências de validação:**
  - Bloquear de verdade (o usuário pediu para pular).
  - Mudança externa do Não perturbe em ≤ 1 s (não alternado a pedido).
  - O caminho do modo de edição que adiciona o sino com a ilha desligada (coberto por código e teste da regra, não na tela).
  - DPI 125/150 % e PresentMon elevado.
- **Minor adiados** (lista completa no ledger):
  - Bloquear e Suspender rodam na thread da UI.
  - Sair pode esperar até 15 s pelo HTTP do Claude.
  - Pedido de limpar notificações durante o aquecimento dispara depois.
  - Win+N pode ser descartado com janela elevada em primeiro plano.
  - Chips de ação não respondem no painel de excedentes.
  - O Codex percorre a árvore de sessões a cada 120 s.
  - `utilization` nulo invalida a resposta do Claude.
  - Duplicatas não vizinhas no VS Code.
  - Horário de verão nas datas.
  - `exists()` em unidade de rede offline.
- **Se o Windows mudar a API de Não perturbe ou de notificações,** o chip continua lendo e cai para abrir as configurações.
