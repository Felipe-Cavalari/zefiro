# Zefiro — Fase 1e: Widgets do YASB que faltam (spec)

> **Data:** 2026-10-19
> **Status:** proposta, aguardando aprovação do usuário.
> **Base:** `~/.config/yasb/config.yaml` do usuário (a barra que o Zefiro substitui), [PLANNING](../../PLANNING.md) (§2 widgets da Fase 1, §3.4 threads, §12 roteiro), [ADR-0009](../../adr/0009-ilha-embutida-na-barra.md) (ilha embutida) e [ADR-0010](../../adr/0010-audio-e-brilho-fase-1d.md) (áudio e brilho, modelo de chip + painel).

## 1. Objetivo

Trazer para o Zefiro os widgets da barra do YASB que **ainda não existem** nele, para o Zefiro substituir o YASB no uso diário. Cada widget vira um chip do catálogo, com painel próprio que nasce do chip, como som, microfone e brilho na 1d.

Pedido do usuário (2026-10-19):

- **Não mudar nada da barra atual.** Chips, layout, ilha, menus e comportamento existentes ficam como estão.
- **Só os widgets que faltam.** O que a ilha embutida já mostra não vira widget separado: o player de mídia e o CPU/RAM (aba de hardware da ilha).
- Exemplos citados: os repositórios do VS Code e os atalhos dos apps mais usados.

## 2. Inventário: YASB × Zefiro

Barra principal do YASB: `left: komorebi_control, komorebi_workspaces, komorebi_active_layout, komorebi_stack, pomodoro, taskbar` · `center: island` · `right: claude_usage, codex_usage, vscode, whkd, sysmon, brightness, volume, microphone, notifications, dnd, power_menu`. Barra dos outros monitores: `komorebi_workspaces, komorebi_active_layout, taskbar` · `island_lite` · `brightness, volume, microphone`.

| Widget do YASB | No Zefiro hoje | Na 1e |
|---|---|---|
| `island` (visualizador, mídia, data, hora), `island_lite`, `media` | Ilha embutida e relógio | — já existe |
| `brightness`, `volume`, `microphone` | Chips da 1d | — já existe |
| `sysmon` (CPU/GPU/RAM) | Aba de hardware da ilha | — já existe (pedido do usuário) |
| `pomodoro` | Timer de foco do menu da ilha (25/50 min, pausa de 5 min) | — coberto pela ilha (**decisão minha**, ver §9) |
| `komorebi_*`, `whkd` | — | fora: o komorebi e o whkd não estão rodando, e a integração com o komorebi fica fora até a 1.0 (decisão da 1a) |
| `claude_usage` | — | **Claude** (§4.1) |
| `codex_usage` | — | **Codex** (§4.2) |
| `vscode` | — | **VS Code** (§4.3) |
| `taskbar` | — | **Apps** (§4.4) |
| `notifications` | Avisos de notificação na pílula | **Notificações** (§4.5): contador e central |
| `dnd` | Indicador na pílula | **Não perturbe** (§4.6): alternar |
| `power_menu` | "Sair" do menu do Zefiro só fecha a barra | **Energia** (§4.7) |

## 3. Regras comuns

- **Catálogo:** sete itens novos em `crates/modules/bar/src/catalog.rs`, com ids `claude`, `codex`, `vscode`, `apps`, `notifications`, `dnd`, `power`. Aparecem na galeria do modo de edição (1b) como os outros.
- **Configuração do usuário intocada:** a fase não grava nada no `config.toml` dele. Os itens novos só aparecem quando ele os adiciona pela galeria ou pelo arquivo. No fim da fase, a entrega oferece (e só faz com um "sim") colocar os itens na ordem do YASB: `start = ["zefiro", "apps"]`, `end = ["claude", "codex", "vscode", "brightness", "audio", "mic", "notifications", "dnd", "power", "clock"]`.
- **Cliques**, iguais aos chips atuais:
  - esquerdo abre ou fecha o painel do chip;
  - direito continua abrindo o menu do Zefiro em qualquer ponto da barra, **exceto** num ícone do chip Apps, que abre o menu daquele app (§4.4);
  - botão do meio é novo e só tem efeito no Apps (nova janela) e em Notificações (limpar). Nos outros chips não faz nada.

  O YASB usava o botão direito para alternar o rótulo (`toggle_label`); aqui o painel mostra os dois valores, então o chip não alterna rótulo.
- **Visual:** chip = ícone (Segoe Fluent Icons) + texto curto, no `level_chip` da 1d (largura reservada quando o texto é numérico). Painéis no estilo dos painéis de som/brilho: título, linhas, separadores, tokens do tema. Nenhum token novo no formato de tema.
- **Dados fora da thread da UI:** cada fonte tem sua thread, avisa a UI por `post_user` e só trabalha quando precisa (§6). A UI nunca faz I/O.
- **Falha isolada:** fonte indisponível deixa o chip apagado com "—" e o painel explica o motivo em uma linha. Nada derruba a barra. Erros vão para o log; **tokens e conteúdo de credenciais nunca vão para o log**.
- **Textos:** português do Brasil. Datas e horas no formato do Windows (pt-BR), como o relógio.

## 4. Widgets

### 4.1 Claude (`claude`)

O que o YASB mostrava: `{five_hour}%`, com 7 dias no rótulo alternativo; reinício de 5 h relativo e de 7 dias absoluto; atualização a cada 120 s.

- **Chip:** ícone + uso da janela de 5 h (`42%`). Cor do texto: normal abaixo de 80 %, `accent` a partir de 80 % (**decisão minha**: o tema não tem cor de alerta e a fase não cria tokens).
- **Painel "Claude":** duas linhas com barra de progresso:
  - **5 horas:** % e "reinicia em 2 h 14 min";
  - **7 dias:** % e "reinicia qui 23 out, 14:00".

  Se a resposta trouxer limites semanais por modelo, eles entram como linhas extras com o nome que a API der. No rodapé, "Atualizado há 1 min".
- **Fonte:** `GET https://api.anthropic.com/api/oauth/usage` com `Authorization: Bearer <claudeAiOauth.accessToken>` de `~/.claude/.credentials.json` e o cabeçalho `anthropic-beta: oauth-2025-04-20`. Campos esperados: `five_hour.utilization` e `five_hour.resets_at`; `seven_day.utilization` e `seven_day.resets_at`. O parser é tolerante: campo ausente é "sem dado", formato desconhecido é "formato desconhecido", e nada entra em pânico.
- **Credenciais:** o arquivo é relido a cada consulta, porque o Claude Code renova o token sozinho. O Zefiro **nunca renova nem grava** o token: um refresh dele trocaria o refresh token e derrubaria o login do Claude Code. Com `expiresAt` vencido ou resposta 401, o painel mostra "Login do Claude Code expirado; abra o Claude Code para renovar" e o chip fica apagado.
- **Ritmo:** consulta ao iniciar, a cada 120 s e ao abrir o painel se o dado tiver mais de 60 s. Com erro de rede, repete em 120 s, sem rajadas. HTTP pelo `HttpClient` do Windows (`platform-windows/src/http.rs`).
- **Primeira tarefa:** uma sonda (`examples/claude_usage_probe.rs`) confirma o endpoint e o cabeçalho `anthropic-beta`. Ela imprime só o status HTTP e os **nomes** dos campos da resposta, nunca o token. A resposta salva como fixture de teste vai anonimizada.

### 4.2 Codex (`codex`)

O que o YASB mostrava: `{primary_used}%`, com `{secondary_window} {secondary_used}%` no alternativo, sem varrer o histórico (`show_token_usage: false`).

- **Chip:** ícone + uso da janela primária (`80%`), com a mesma regra de cor do Claude.
- **Painel "Codex":** uma linha por janela (primária e secundária). O nome vem de `window_minutes`: 300 → "5 horas", 10080 → "7 dias", outros valores em horas ou dias. Cada linha tem % e quando reinicia; o plano (`plan_type`) aparece no título se existir.
- **Fonte:** o `.jsonl` mais recente em `~/.codex/sessions/**`, pela data de modificação. Lê só o fim do arquivo (até 256 KB) de trás para frente, até achar um evento com `payload.rate_limits`. Formato confirmado nesta máquina:

  ```json
  "rate_limits": {"primary": {"used_percent": 80.0, "window_minutes": 300, "resets_at": 1792130364},
                  "secondary": {"used_percent": 19.0, "window_minutes": 10080, "resets_at": 1792678081},
                  "plan_type": "plus"}
  ```

  Não abre `auth.json` nem nenhuma credencial do Codex.
- **Dado velho:** o arquivo só muda quando o Codex roda. Se `resets_at` já passou, aquela janela mostra 0 % e "reiniciada"; o rodapé diz "Último uso do Codex: 16/10 23:56".
- **Ritmo:** a cada 120 s, só um `stat` do arquivo mais recente; relê apenas se ele mudou. Também relê ao abrir o painel.

### 4.3 VS Code (`vscode`)

O que o YASB mostrava: ícone; menu "VS Code · recentes" com até 20 pastas e 10 arquivos, data `dd/mm HH:MM`, nome encurtado para a pasta raiz, abrindo com `code`.

- **Chip:** só o ícone.
- **Painel "VS Code · recentes":** até 20 projetos, do mais recente ao mais antigo. Cada linha tem:
  - o nome da pasta (último segmento do caminho);
  - à direita, em texto terciário, a data `dd/MM HH:mm`;
  - embaixo, o caminho curto, e um selo "WSL", "Remoto" ou "Workspace" quando não é uma pasta local.

  Roda do mouse rola a lista quando ela passa da altura máxima (12 linhas visíveis). Clique abre o projeto e fecha o painel.
- **Fonte:** `%APPDATA%\Code\User\workspaceStorage\*\workspace.json`, com `"folder": <uri>` ou `"workspace": <uri>`, ordenados pela data de modificação da pasta de cada entrada. Medido aqui: 27 entradas, incluindo pastas `vscode-remote://wsl%2Bubuntu-24.04/...`. **Mudança em relação ao PLANNING:** a chave `history.recentlyOpenedPathsList` do `state.vscdb` não existe na versão do VS Code instalada, então não há SQLite. Arquivos recentes soltos não aparecem nessa fonte e ficam fora (§9).
- **Abrir:** `code --folder-uri <uri>` para pastas (locais e remotas) e `code --file-uri <uri>` para workspaces, sem janela de console. Se o `code` não estiver no PATH, usar o `Code.exe` instalado (`%LOCALAPPDATA%\Programs\Microsoft VS Code\Code.exe`). Pastas locais que não existem mais ficam fora da lista.
- **Ritmo:** lê a pasta ao abrir o painel, numa thread. Sem consulta periódica.

### 4.4 Apps (`apps`)

O que o YASB mostrava (`taskbar`): ícones de 18 px dos apps abertos, agrupados por app, sem título. Clique alterna a janela e o botão direito abre o menu de contexto. O usuário descreveu este widget como "os atalhos dos apps mais usados".

- **Chip:** uma fileira de ícones (18 px a 100 %, escalando com o DPI), na ordem:
  1. **apps fixados:** por padrão, os atalhos fixados na barra de tarefas do Windows (`%APPDATA%\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar\*.lnk`; hoje Brave, Explorador de Arquivos e VS Code);
  2. **apps abertos** que não estão fixados.

  App aberto tem um ponto de 3 px embaixo do ícone; o app em primeiro plano tem o ponto em `accent`. As janelas são agrupadas por app (AUMID quando houver, senão o executável). Passou de 12 ícones, os excedentes vão para o painel de excedentes que já existe.
- **Cliques num ícone:**
  - **esquerdo:** app fechado abre; app com uma janela ativa minimiza; com uma janela em segundo plano traz para frente; com várias janelas alterna entre elas a cada clique;
  - **meio:** abre uma nova instância;
  - **direito:** menu nativo do app com "Nova janela", "Fixar na barra" ou "Desafixar da barra" e, se estiver aberto, "Fechar janela" (ou "Fechar todas as janelas", com várias).
- **Fixados próprios:** fixar ou desafixar grava `[apps] pinned = [...]` (caminhos de `.lnk`/exe ou AUMIDs) no `config.toml`, com `toml_edit` preservando comentários. Enquanto a chave não existe, valem os fixados do Windows. O Zefiro nunca altera a barra de tarefas do Windows.
- **Quais janelas contam:** visíveis, sem dono, que não sejam ferramenta e não estejam "cloaked", com o mesmo critério da barra de tarefas. Ficam fora as janelas do próprio Zefiro e as janelas sem título. Todas as barras mostram as janelas de todos os monitores, como o `monitor_exclusive: false` do YASB.
- **Fonte:** ganchos `SetWinEventHook` (criar, destruir, mostrar, esconder, primeiro plano, mudança de nome) numa thread própria, sem consulta periódica. Ícones pelo `app_icon.rs` da ilha, com cache por app.
- **Painel:** o chip Apps não tem painel. O clique esquerdo age direto no ícone.
- **Clique por ícone:** hoje um chip é uma área de clique só. O conteúdo do Apps devolve também o retângulo de cada ícone, e a barra resolve qual ícone foi clicado a partir disso. O realce do mouse fica no ícone, não no chip inteiro. Só o Apps usa isso.

### 4.5 Notificações (`notifications`)

O que o YASB mostrava: ícone + contador; clique abre a central, meio limpa.

- **Chip:** sino; com notificações, sino com ponto + o número (`3`). Sem notificações, só o sino, apagado.
- **Cliques:** esquerdo abre a central de notificações do Windows (o mesmo que Win+N); meio limpa todas (`UserNotificationListener.ClearNotifications`). O chip não tem painel.
- **Fonte:** o serviço de notificações da ilha (`platform-windows/src/notifications.rs`), que já tem acesso concedido, passa também a publicar a contagem. Funciona com a ilha embutida ligada ou não: a contagem é do serviço, não da ilha.

### 4.6 Não perturbe (`dnd`)

O que o YASB mostrava: ícone; clique liga e desliga em "Prioridade".

- **Chip:** ícone de lua; aceso (`accent`) quando o Não perturbe está ativo.
- **Clique:** alterna entre desligado e "somente prioridade". O chip não tem painel.
- **Fonte de leitura:** o `dnd.rs` atual (WNF). Hoje ele guarda um único aviso estático (`NOTIFY: OnceLock`), usado pela ilha. A leitura continua sendo uma só, mas o aviso passa a acordar a ilha e o chip juntos (um aviso só para a UI, que atualiza os dois), funcionando também com a ilha desligada.
- **Escrita:** ainda não foi provada (o S5 da Fase 0 não terminou). A primeira tarefa desta parte é uma sonda que tenta alternar o perfil (WNF ou o serviço Quiet Hours, como o `dnd_api` do YASB) e confere a leitura. **Se não for confiável em ≤ 1 s**, o clique abre `ms-settings:notifications` e o risco fica registrado no ADR. A decisão é tomada pela sonda, não pelo plano.

### 4.7 Energia (`power`)

O que o YASB mostrava: ícone ⏻; popup com foto e nome do usuário, tempo ligado e os botões Bloquear, Suspender, Sair, Reiniciar, Desligar e Cancelar.

- **Chip:** ícone de energia.
- **Painel "Energia":**
  - **cabeçalho:** foto da conta do Windows (sem foto, as iniciais num círculo), o nome de exibição e "Ligado há 3 d 4 h". Foto e nome vêm de `Windows.System.User` (`GetPictureAsync`, `DisplayName`); se a API falhar, usa o nome de usuário (`GetUserNameExW`) e as iniciais. Tudo lido numa thread ao abrir o painel e guardado em cache;
  - **botões:** Bloquear, Suspender, Sair, Reiniciar e Desligar, numa grade de ícone + rótulo.
- **Confirmação:** Bloquear e Suspender agem no primeiro clique. **Sair, Reiniciar e Desligar pedem um segundo clique**: o botão vira "Clique de novo para reiniciar" por 3 s e volta sozinho. Fechar o painel cancela. (O YASB usava um botão Cancelar; o segundo clique é **decisão minha**.)
- **Ações:** `LockWorkStation`; suspender com `SetSuspendState` sem hibernar; sair com `ExitWindowsEx(EWX_LOGOFF)`; reiniciar e desligar com `ExitWindowsEx` e o privilégio de desligamento ajustado, sem forçar apps a fechar.
- **Antes de sair, reiniciar ou desligar,** a barra libera a AppBar e fecha normalmente, como no "Sair" do menu.

## 5. Arquitetura

A regra de ouro continua: a crate `windows` só aparece em `platform-windows`, `integrations/clock/src/windows.rs` e `app`.

| Camada | O que entra |
|---|---|
| `crates/integrations/usage` (nova, portável) | Parser da resposta do Claude, leitura do fim do `.jsonl` do Codex, nomes das janelas, formatação de "reinicia em" (relativa) e da data absoluta a partir de um relógio injetado, e a regra de cor por faixa. Arquivos com `std::fs`. A thread que lê o Codex mora aqui. |
| `crates/integrations/vscode` (nova, portável) | Leitura do `workspaceStorage`, decodificação de URI (`file:///c%3A/...`, `vscode-remote://wsl%2B...`), nome curto, selo e ordenação. |
| `crates/modules/bar` | Chips e painéis dos sete itens (`DisplayList`), áreas de clique, rolagem da lista do VS Code, catálogo e `PanelKind` novos (`Claude`, `Codex`, `VsCode`, `Power`). |
| `crates/platform-windows` | HTTP do Claude (reaproveita `http.rs`), janelas e ganchos do Apps (`taskbar.rs`), ícones (reaproveita `app_icon.rs`), abrir processos sem console, contagem e limpeza de notificações, escrita do Não perturbe, ações de energia, foto e nome da conta, tempo ligado. |
| `crates/app` | Um `*_loop.rs` por grupo (`usage_loop.rs`, `apps_loop.rs`, `system_loop.rs`, no padrão do `devices_loop.rs`), roteamento dos cliques, inclusive o botão do meio novo. |
| CI | As crates portáveis novas entram no `cargo check` do Linux. |

## 6. Desempenho e robustez

- **Repouso:** CPU **0,0 %** com os sete chips na barra e nenhum painel aberto (`perf.ps1`, 60 s, YASB fechado).
- **Memória:** com os sete chips, no máximo **+4 MB privados** em relação à barra sem eles, medido do mesmo jeito que no ADR-0010. A memória que fica depois de abrir os painéis também é medida e registrada, sem meta própria.
- **Sem consulta por quadro.**
  - Periódicas: Claude e Codex a cada 120 s; no Codex é só um `stat`.
  - Por evento: Apps, notificações e Não perturbe.
  - Só ao abrir o painel: VS Code e o tempo ligado.
- **Apps:** o gancho de eventos não pode travar a UI. A thread filtra e agrupa os eventos e só avisa quando o conjunto de apps ou o app em primeiro plano muda, no máximo uma vez a cada 50 ms.
- **Fonte travada:** HTTP com tempo limite de 15 s, como o `timeout` do YASB. Leitura de arquivo nunca na thread da UI.

## 7. Validação e critérios de aceite

Seguindo o AGENTS.md: TDD em toda lógica portável e sonda que falha antes e passa depois quando o comportamento é de tela.

| Parte | Testes automatizados | Na tela / sonda |
|---|---|---|
| Claude | Parser com resposta real anonimizada, campos ausentes, formato desconhecido, token vencido; formatação de reinício. | Chip e painel com dados reais; com o token vencido simulado no perfil de teste (cópia do arquivo de credenciais **sem** token válido), mostra o aviso. |
| Codex | Fim de `.jsonl` com e sem `rate_limits`, linha cortada no começo do bloco, janela já reiniciada, nomes de janela. | Chip e painel batendo com o último evento do arquivo. |
| VS Code | Decodificação de URIs locais, WSL e workspace; ordenação; pasta local apagada some. | Abrir um projeto local e um do WSL pelo painel. |
| Apps | Agrupamento por app, ordem fixados → abertos, gravação de `[apps] pinned` preservando comentários. | Abrir, alternar, minimizar, nova janela, fechar e fixar com o Bloco de notas como app de teste (nunca fechar apps do usuário). |
| Notificações | Formatação do contador. | Contagem muda com uma notificação de teste; o clique abre a central. Limpar só com o usuário de acordo. |
| Não perturbe | — | A sonda do §4.6 decide entre alternar e abrir as configurações. |
| Energia | Formatação do tempo ligado; máquina de estados da confirmação (primeiro clique arma, 3 s desarma, fechar cancela). | Bloquear validado com o usuário presente. **Suspender, Sair, Reiniciar e Desligar não são executados nos testes**: valida-se até o estado "confirmar", e a chamada final é coberta por uma sonda que roda com a ação trocada por um registro. |

Além disso:

- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `scripts/check-windows-imports.ps1` e o `cargo check` das crates portáveis no alvo Linux passam.
- A barra do usuário, sem os itens novos, fica visualmente igual: captura da faixa antes e depois.
- Medições do §6 registradas no ADR da fase.

## 8. Roteiro

Esta spec muda o roteiro do PLANNING (**decisão minha**, a confirmar na aprovação):

| Etapa | Antes | Depois |
|---|---|---|
| 1e | Itens declarativos v1 + linguagem de expressões; Claude e Codex como itens | **Widgets do YASB que faltam**: Claude, Codex, VS Code, Apps, Notificações, Não perturbe e Energia, nativos |
| 1f | VS Code, Pomodoro, Modo foco, Modo noturno, Notificações | **Itens declarativos v1** + linguagem de expressões + Modo noturno (o único da antiga 1f que não entra aqui) |
| 1g | Tema, HUD, diagnóstico, 2 semanas de uso | igual |

Claude e Codex nascem nativos. Quando os itens declarativos chegarem, eles podem ser refeitos como itens sobre os mesmos parsers, que já ficam em crate portável.

Ordem sugerida de entrega dentro da 1e, da mais simples para a mais arriscada: Codex → Claude → VS Code → Energia → Notificações → Não perturbe (com a sonda) → Apps.

## 9. Fora do escopo e decisões minhas

**Fora:**

- `komorebi_*` e `whkd`;
- Pomodoro com ciclos automáticos (o timer da ilha cobre 25/50/5 min);
- arquivos recentes soltos do VS Code e busca na lista de projetos;
- dicas flutuantes (tooltips);
- alternar o rótulo pelo botão direito;
- pré-visualização de janelas no Apps;
- itens declarativos.

**Decisões minhas, para conferir na leitura:**

1. Pomodoro coberto pelo timer da ilha.
2. Botão direito continua sendo o menu do Zefiro, exceto nos ícones do Apps.
3. Cor por faixa só com o `accent` (≥ 80 %), sem tokens novos.
4. VS Code pelo `workspaceStorage`, sem SQLite e sem arquivos soltos.
5. Apps começa com os fixados do Windows e grava os próprios em `[apps] pinned`.
6. Confirmação por segundo clique em Sair, Reiniciar e Desligar.
7. Itens novos fora do `config.toml` do usuário até ele aceitar a ordem do YASB no fim da fase.
8. Roteiro: declarativos passam para a 1f.
