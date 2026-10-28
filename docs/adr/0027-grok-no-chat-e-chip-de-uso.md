# ADR-0027: Grok no chat e chip de uso do Grok pelo Grok Build

- **Status:** aceito
- **Data:** 2026-10-28
- **Spike/branch:** main (spec `docs/superpowers/specs/2026-10-28-chat-grok-design.md`)

## Contexto

O usuário assina o SuperGrok e queria o Grok (xAI) como terceiro agente do chip `chat`, ao lado do
Claude e do Codex (ADR-0020), e um chip `grok` com o limite de uso, como os chips `claude` e
`codex`. A preocupação é a mesma da ADR-0025: usar só o caminho oficial, nunca o token da
assinatura por conta própria, e deixar claro quem paga.

O caminho oficial existe: o **Grok Build** (`grok`, a CLI da xAI, código aberto sob Apache 2.0).
O `grok login` entra com a assinatura (SuperGrok, X Premium+, X Subscription) e guarda o login em
`~/.grok/auth.json`; o `XAI_API_KEY` é o plano B, pago por token. O `grok agent stdio` é um
processo vivo que fala ACP (Agent Client Protocol, JSON-RPC por linhas), oferecido pela própria
documentação para "custom apps".

## Opções consideradas

1. **`grok -p` por pergunta** (`--output-format streaming-json`, `--resume`): aceita `--tools` e
   `--disallowed-tools`, mas sobe um processo a cada pergunta, o que a ADR-0020 rejeitou para o
   Claude.
2. **`grok agent stdio` (ACP)**, um processo por conversa, como o `codex app-server`. Escolhida.
3. **OAuth próprio do Zefiro com a xAI** (o caminho do Hermes e do OpenClaw): é o "token da
   assinatura fora do produto" que a ADR-0025 tirou do Claude. Descartada.

Para tirar as ferramentas do chat no ACP:

1. `--deny '*'` e `--disallowed-tools` antes de `agent`: **não valem** no `agent stdio`. Na sonda
   o `read_file` rodou e leu `/etc/hostname`.
2. Recusar os `session/request_permission`: não basta. O `~/.grok/config.toml` do usuário tem
   `permission_mode = "always-approve"` (a sessão nasce com `yolo: true`), e no modo padrão as
   ferramentas de leitura nem pedem permissão.
3. `GROK_SANDBOX=strict`: o `grok` **se recusa a subir** sem o bubblewrap no Linux ("Refusing to
   start with denied paths unprotected").
4. **Perfil de agente próprio** (`grok agent --agent-profile <arquivo> stdio`) com
   `tools: todo_write` e `mcpInheritance: none`. Escolhida: o modelo responde "Não tenho acesso a
   read_file ou cat", e a entrada cai de cerca de 19 mil para 11 mil tokens por pergunta.

## Números medidos

Grok Build 1.0.70 no Ubuntu-24.04 (WSL2), SuperGrok, Grok 4.7 com esforço `high` (o padrão dele),
2026-10-28. O Windows não tem o `grok` instalado: a origem Windows só tem os testes portáveis.

| Métrica | Valor |
|---|---|
| `initialize` | < 3 s |
| Primeiro texto, processo frio (`chat_probe`) | 6,3 s |
| Primeiro texto, processo novo retomando a sessão (`session/load`) | 4,8 s |
| Primeiro texto de uma resposta longa ("400 palavras") | 53,7 s (raciocínio `high`) |
| Tokens de entrada por pergunta curta, sem perfil / com perfil | ~19 mil / ~11 mil |
| Consulta do chip (`grok_usage_probe`, WSL) | 2,8 s |
| Pedir para ler um arquivo, com o perfil | nenhuma ferramenta; respondeu "sem ferramentas" |

## Decisão

**Chat:** um `grok agent --agent-profile grok-perfil.md stdio` por conversa, na pasta do chat
(`%LOCALAPPDATA%\zefiro\chat` ou `~/.cache/zefiro-chat`), com `GROK_DISABLE_AUTOUPDATER=1`,
`GROK_MEMORY=0`, `GROK_WORKFLOWS=0` e `GROK_SUBAGENTS=0`. O perfil é gravado pelo Zefiro antes de
cada partida: no Windows com `std::fs::write`, no WSL com uma linha `echo` por item no script do
`bash -lc` (sem aspas duplas nem barras invertidas, que a linha de comando do `wsl.exe` estraga).

| ACP | O Zefiro |
|---|---|
| `initialize` | Sem `fs` nem `terminal`. Guarda o `currentWorkingDirectory` (o `cwd` das sessões, no Windows e no WSL) e os modelos. |
| `authenticate` | Só `cached_token`. O método `grok.com` abre o navegador e nunca é pedido. Uma falha só vai para o `session/new`, que diz o motivo. |
| `session/new` | `mcpServers: []`, `_meta.rules` com o `SYSTEM_PROMPT` do chat. Sem `yoloMode`. |
| `session/load` | Retoma a conversa gravada. O Grok reenvia o histórico como `session/update` antes da resposta: até ela chegar, os pedaços não vão para a tela. Sessão que não existe responde `FS_NOT_FOUND` e vira "session not found" (o chat começa outra e reenvia a pergunta, como no Claude). |
| `session/prompt` | Imagens PNG em base64 (`{"type":"image","mimeType":"image/png","data":…}`) antes do texto. O `stopReason` da resposta fecha o turno. |
| `session/update` | `agent_message_chunk` vira texto; `agent_thought_chunk`, "pensando"; `tool_call` só vai para o log em debug, com o nome. |
| `session/cancel` | O botão de parar (notificação). O `session/prompt` responde `cancelled`. |
| `session/set_config_option` | Troca de modelo antes da pergunta, com o valor em texto puro (com objeto, "Invalid params"). A pergunta espera a resposta. |
| `session/request_permission` | Sempre a opção `reject_once` (ou "cancelado", se não houver). Outros pedidos do agente: erro `-32601`. |

Os modelos vêm do próprio `grok` (`availableModels`); o padrão é o primeiro `currentModelId` que o
processo anunciar. O Grok 4.7 Fast aparece como "Grok 4.7 Fast (2× o preço)".

**Chip `grok`:** a cada 5 min (2 min ao abrir o painel, 60 s no botão), um `grok agent stdio`
curto por origem (o Windows e cada distro rodando), sem perfil: `initialize`, `_x.ai/auth/info` e
`_x.ai/billing`, com o sublinhado das extensões do ACP (`x.ai/billing` responde "Method not
found"). Nenhuma sessão é aberta e o modelo não é chamado: o chip não gasta cota. O
`creditUsagePercent` é a % do chip; o `currentPeriod` diz quando a semana vira; o
`subscription_tier`, o plano. O `principalId` do `_x.ai/auth/info` junta a mesma conta em origens
diferentes e nunca vai para o log; o e-mail só aparece no painel. Origem sem `grok` (código 127 do
bash ou executável ausente) fica de fora.

O Zefiro nunca lê o `~/.grok/auth.json`, nunca guarda chave e nunca faz pedido HTTP à xAI.

## Consequências

- **Quem paga:** com a assinatura, cada pergunta do chat sai da mesma cota semanal do grok.com, do
  Imagine e do Build; com `XAI_API_KEY`, o usuário paga por token. O README tem o "Aviso sobre a
  assinatura do Grok".
- **Termos (médio):** rodar o Grok Build pelo ACP é uso documentado, mas nenhum texto da xAI
  autoriza explicitamente um app de terceiros a usá-lo como chat com a assinatura. Zona cinzenta,
  como o `claude -p`.
- **Extensão não documentada:** o `_x.ai/billing` está fora da documentação. Se mudar, o chip diz
  "Formato de resposta desconhecido", nunca um número errado.
- **Configuração do usuário:** hooks e plugins do `~/.grok` continuam carregando no processo do chat
  (não há equivalente do `--setting-sources ""`). Só o perfil limita as ferramentas.
- **Não verificado:** a origem Windows (o `grok` não está instalado), o texto do erro de cota
  esgotada e de login ausente (classificados por palavras: "quota", "unauthenticated"…), e a
  unidade do `prepaidBalance.val` (lida como centavos de dólar; o saldo do usuário é 0).
- **Esforço de raciocínio:** o Grok 4.7 usa `high` por padrão, e respostas longas demoram para
  começar (53,7 s medidos). Um esforço menor no chat (`--reasoning-effort` ou o `configId`
  `reasoning_effort`) fica para depois.
- **Sondas:** `chat_probe` com `ZEFIRO_CHAT_AGENT=grok` (inclui o pedido para ler um arquivo) e
  `grok_usage_probe`.
