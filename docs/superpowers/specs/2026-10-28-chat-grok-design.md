# Grok no chat e chip de uso do Grok — design

Data: 2026-10-28. Pedido do usuário: o Grok (xAI) como terceiro agente do chip `chat`, ao lado do
Claude e do Codex, e um chip de limite `grok`, como os chips `claude` e `codex`. Mesma preocupação
da ADR-0025: usar o caminho oficial da xAI, nunca o token da assinatura por conta própria, e deixar
claro quem paga e o que os termos permitem. Aprovada pelo usuário em 2026-10-28 ("pode implementar
direto", sem plano separado); implementada na mesma data. Decisão final: ADR-0027.

## Intenção

O usuário assina o SuperGrok e já tem o **Grok Build** (a CLI oficial da xAI, `grok`) instalado no
WSL. Ele quer conversar com o Grok pelo mesmo painel do chat e ver na barra quanto da cota semanal
já gastou, do mesmo jeito que faz com o Claude e o Codex.

## O que a pesquisa achou (2026-10-28)

**Existe CLI oficial com login da assinatura.** O Grok Build (`grok`, 1.0.70 no Ubuntu-24.04 desta
máquina) é da própria xAI (hoje SpaceXAI), com código aberto sob Apache 2.0 desde julho de 2026. Ele
roda no Windows (`irm https://x.ai/cli/install.ps1 | iex`, que instala em
`%USERPROFILE%\.grok\bin\grok.exe` e põe a pasta no PATH), no Linux e no WSL. Login:

- **Assinatura:** `grok login` abre o navegador (OAuth da SpaceXAI em `auth.x.ai`), ou
  `grok login --device-auth` mostra um código para digitar em outro aparelho. Vale para SuperGrok e
  X Premium+ (e, desde 12/10/2026, para a nova X Subscription). As credenciais ficam em
  `~/.grok/auth.json`, só do dono, e a própria CLI renova o token.
- **Chave de API:** `XAI_API_KEY` (criada em console.x.ai) é o plano B. O login da assinatura
  ganha da chave quando os dois existem.

**Modos que servem para o Zefiro** (conferidos no `--help` e na documentação que vem com a CLI, em
`~/.grok/docs/user-guide/14-headless-mode.md` e `15-agent-mode.md`):

| Modo | Como é | Serve? |
|---|---|---|
| `grok -p "<pergunta>" --output-format streaming-json` | Um processo por pergunta, `--resume <id>` para continuar. Não lê o stdin. Parar = matar o processo. | Funciona, mas é o "um processo por turno" que a ADR-0020 rejeitou para o Claude (partida a cada pergunta). |
| **`grok agent stdio`** | Processo vivo que fala **ACP** (Agent Client Protocol, JSON-RPC por linhas). `session/new`, `session/prompt` com blocos de texto e imagem, `session/update` com `agent_message_chunk`, `session/cancel`, `session/load` para retomar, `session/set_config_option` para trocar o modelo. A documentação diz que é para "IDEs, SDKs, eval harnesses, and custom apps". | **Sim.** É o equivalente do `codex app-server`. |

**Sonda feita nesta pesquisa (sem nenhuma pergunta ao modelo):** um `initialize` no `grok agent
stdio` do WSL respondeu em menos de 3 s com `loadSession: true`, `promptCapabilities.image: true`,
os métodos de login (`cached_token`, `grok.com`), o plano (`subscription_tier_display:
"SuperGrok"`) e a lista de modelos (Grok 4.7, 4.7 Fast, 4.6, 4.5; o padrão chegou num
`_x.ai/models/update` logo depois). Em seguida, o método de extensão **`_x.ai/billing`** devolveu:

```json
{"config":{"creditUsagePercent":25.0,
  "currentPeriod":{"type":"USAGE_PERIOD_TYPE_WEEKLY","start":"2026-10-23T23:04:17.875+00:00","end":"2026-10-30T23:04:17.875+00:00"},
  "prepaidBalance":{"val":0},"isUnifiedBillingUser":true, …},
 "subscription_tier":"SuperGrok"}
```

É o mesmo dado da aba "Usage limit" do `/usage` da CLI: a cota semanal única que, desde junho de
2026, o SuperGrok divide entre o chat do grok.com, o Imagine e o Build. O `x.ai/billing` sem o
sublinhado responde "Method not found"; o nome com `_` segue a regra do ACP para extensões.

**Termos.** A página dos termos de consumidor da xAI devolveu 403 para leitura automática; o que
deu para apurar:

- A Política de Uso Aceitável (agosto de 2026) proíbe acesso aos serviços por meios automatizados
  **não autorizados** (bots, scripts). Grok Build com `-p` e ACP são meios autorizados: a própria
  documentação oficial os oferece para scripts, CI e apps de terceiros.
- A xAI divulga o uso da assinatura em clientes de terceiros (OpenCode, Zed como "external agent",
  Hermes, Kilo Code), tanto pelo Grok Build quanto por um OAuth próprio desses clientes.
- Ninguém da xAI disse, por escrito, que **um app qualquer** pode dirigir o Grok Build como chat.
  Uma issue de outro projeto (de outubro de 2026) lê a X Subscription nova como cota só para "uso
  primário elegível" e não cita clientes de terceiros. O Grok Build é primário; o Zefiro em volta
  dele é zona cinzenta, como o `claude -p` no README.

## Objetivos

1. **Agente `grok` no chat:** `[chat] agent = "grok"`, terceiro segmento "Grok" no seletor do
   painel, com origem (Windows ou WSL), modelo, parar, imagens coladas, conversas antigas e
   retomada, como o Codex.
2. **Chip de limite `grok`:** ícone + % da cota semanal, painel com a semana, quando zera, o plano e
   o saldo de créditos extras (quando houver), com o mesmo botão de atualizar dos outros chips.
3. **Só o caminho oficial:** quem autentica, guarda e renova o login, e fala com a xAI é o `grok`
   instalado pelo usuário. O Zefiro **nunca lê** o `~/.grok/auth.json`, nunca guarda chave e nunca
   faz pedido HTTP à xAI.
4. Windows e WSL com as mesmas regras do Claude e do Codex (ADR-0019, ADR-0020): o automático segue
   `[chat.grok] prefer`, e nenhuma distro parada é ligada.

## Não objetivos

- Chave de API digitada no Zefiro. Quem quer pagar por token configura o `XAI_API_KEY` (ou um
  modelo com `env_key` no `~/.grok/config.toml`) **na CLI**; o Zefiro só repassa o que a CLI faz.
  Fica para depois, se o usuário pedir, com DPAPI como o token do Spotify (ADR-0024).
- Login pelo Zefiro. Sem login, o painel manda rodar `grok login` (ou `grok login --device-auth`
  no WSL); o Zefiro nunca aciona o método `grok.com` do ACP, que abre o navegador.
- OAuth próprio do Zefiro com a xAI (o caminho do Hermes e do OpenClaw). É exatamente o "token da
  assinatura fora do produto" que a ADR-0025 tirou do Claude.
- Seletor de esforço de raciocínio, `/imagine`, busca no X, ferramentas, MCP, workflows. O chat
  continua sendo só conversa.
- O app de desktop Grok Bot (instalado em `%LOCALAPPDATA%\Programs\Grok Bot`): não tem interface
  para outro programa.

## Anti-objetivos (mesmo funcionando, seria falha)

- Uma ferramenta do Grok rodando a partir do chat (ler arquivo, terminal, busca na web, subagente).
- O token, a chave, o e-mail da conta ou o texto de uma conversa no log do Zefiro.
- O chip ou o chat abrindo o navegador para login sem o usuário pedir.
- Uma consulta de uso que chame o modelo (gaste cota).

## Como o chat fala com o Grok

### Processo

Por conversa, um `grok agent stdio` vivo, como o `codex app-server`: sobe ao abrir o painel, sai
com 10 min sem uso, e o stdin fechado o encerra (com `kill` depois de 1,5 s, como hoje). Ambiente
do processo:

| Variável | Por quê |
|---|---|
| `GROK_DISABLE_AUTOUPDATER=1` | Sem atualização rodando em segundo plano (equivale ao `DISABLE_AUTOUPDATER` do Claude). |
| `GROK_MEMORY=0` | Sem memória entre sessões: o chat não grava notas nem injeta o índice no prompt. |
| `GROK_WORKFLOWS=0`, `GROK_SUBAGENTS=0` | Sem workflows nem subagentes. |
| `GROK_SANDBOX=strict` | Rede de segurança: se uma ferramenta escapar, ela só lê a pasta do chat, `~/.grok` e o sistema. Confirmar na sonda se o perfil vale no Windows. |

A pasta de trabalho é a mesma do chat: `%LOCALAPPDATA%\zefiro\chat` no Windows e
`~/.cache/zefiro-chat` no WSL (`command::WSL_DIR`). Ela não é um repositório, não tem `AGENTS.md` e
não é uma pasta confiável para o Grok, então ele não lê `.grok/` de projeto. No WSL as variáveis
entram no script do `bash -lc` (`wsl_args` ganha um prefixo de ambiente); no Windows, no `Command`.

### Sem ferramentas

O Grok Build não tem um `--setting-sources ""`: os hooks, plugins e servidores MCP do
`~/.grok/config.toml` do usuário continuam carregando. Três camadas, nesta ordem:

1. **Ferramentas fora:** a primeira tarefa do plano descobre na sonda se `--disallowed-tools` (ou
   `--tools` com uma lista vazia) antes de `agent stdio` vale no ACP. Se valer, sai tudo.
2. **Nenhuma permissão concedida:** a sessão nasce **sem** `yoloMode`. Todo
   `session/request_permission` que chegar é respondido com a opção do tipo `reject_once` e o
   painel não mostra nada.
3. **Prompt:** o `_meta.rules` do `session/new` leva o `SYSTEM_PROMPT` do chat (pt-BR, curto, texto
   simples) mais "não use ferramentas".

No modo padrão do Grok, ferramentas só de leitura (`read_file`, `list_dir`, `grep`) rodam sem pedir
permissão. Se a camada 1 não sair na sonda, a camada 2 não as pega, e só o `strict` limita o que
elas leem. Nesse caso a tarefa registra um ruling e volta ao usuário antes de seguir.

### Sessão

| ACP | O que o Zefiro faz |
|---|---|
| `initialize` | `protocolVersion: 1`, sem capacidades de arquivo ou terminal (o cliente não oferece `fs` nem `terminal`). Guarda a lista de modelos de `_meta.modelState` e a atualiza com cada `_x.ai/models/update`. |
| `authenticate` | Só com `methodId: "cached_token"`. Se falhar, o painel diz "Faça login no Grok: rode `grok login` no Windows" (ou "`grok login --device-auth` no Ubuntu-24.04") e o processo é encerrado. |
| `session/new` | `cwd` = pasta do chat, `mcpServers: []`, `_meta.rules`. O `sessionId` vai para `Conversation.session`. |
| `session/load` | Retoma a conversa gravada (a sessão fica em `~/.grok/sessions` da origem). Sessão sumida = começa outra e reenvia a pergunta uma vez, como no Claude. |
| `session/prompt` | Texto e até 4 imagens `{"type":"image","mimeType":"image/png","data":"<base64>"}`. A resposta (`stopReason`) fecha o turno. |
| `session/update` | `agent_message_chunk` vira texto; `agent_thought_chunk` é ignorado; `tool_call` só vai para o log em nível debug (o nome da ferramenta, nunca os argumentos). |
| `session/cancel` | O botão de parar. O processo continua vivo. |
| `session/set_config_option` (`configId: "model"`) | Troca de modelo sem reiniciar o processo nem trocar de conversa. |

O parser fica em `crates/integrations/chat/src/grok.rs`, portável e testado com linhas gravadas da
sonda, como o `codex.rs`. O ACP é JSON-RPC como o do Codex, mas com outros nomes: o desenho é um
`GrokState` ao lado do `CodexState` no `Backend` de `platform-windows/src/chat.rs`.

### Modelos

A lista vem do próprio `grok` (`availableModels`), como a do Codex. "Padrão" é o `currentModelId`
que ele anunciar. Quando o modelo gravado some da lista, `effective_model` cai no padrão, como hoje.

## Onde fica cada coisa no config

```toml
[chat]
agent = "grok"        # claude | codex | grok

[chat.grok]
origin = "auto"       # auto | windows | wsl | <distro>
prefer = "windows"    # o que o automático tenta primeiro
model = ""            # vazio = o padrão que o grok anunciar
```

- `Agent` ganha `Grok` (`parse`, `as_str`, `label`, `binary` = `grok`, `ALL` com 3).
- `ChatConfig` ganha `grok: ChatAgentConfig`. `of(codex: bool)` vira `of(agent: &str)`, para a
  crate `core` continuar sem depender da crate do chat; os usos em `chat_loop.rs` (hoje
  `of(agent == Agent::Codex)`) passam `agent.as_str()`.
- `windows_candidates(Grok)`: `%USERPROFILE%\.grok\bin\grok.exe` e depois o PATH.
- Mudar só `[chat.grok]` não fecha o painel (`Config::same_except_panels` já cobre `[chat]`).

## O painel do chat

- O seletor de agente vira "Claude | Codex | Grok" (`Agent::ALL`, que já mede os segmentos). Trocar
  de agente começa outra conversa, como hoje.
- Origens, "No automático, primeiro: Windows | WSL", modelos, parar, copiar e imagens: iguais.
- Erros novos, em pt-BR: "Não achei o `grok` no Windows nem numa distro do WSL rodando. Instale o
  Grok Build ou escolha outra origem." (`pick_error_text` já monta), "Faça login no Grok…" e "Sua
  cota semanal do Grok acabou. Ela volta em 2 d 5 h." (`usage_limit_reached` /
  `usage_pool_exhausted`).
- As conversas antigas mostram o Grok com o nome dele (o `Agent` já vai no JSON da conversa).

## Chip de limite `grok`

- **Catálogo:** `{ id: "grok", name: "Grok", description: "Uso do Grok (cota semanal)" }`, com
  `PanelKind::Grok`. Ícone do Grok no mesmo esquema dos ícones do Claude e do Codex.
- **Chip:** `usage_chip` com a % da semana (`creditUsagePercent`); "—" apagado sem dado. A partir de
  80 % o texto fica na cor de destaque, como os outros.
- **Painel:** um `UsageBlock` por conta: título "Grok · SuperGrok" (`subscription_tier`), subtítulo
  com a origem (e o e-mail, se o ACP der um, só no painel), a linha "Semana" com a % e "reinicia em
  2 d 5 h" (`currentPeriod.end`), e "Créditos extras: US$ 4,00" quando `prepaidBalance.val` > 0.
- **Fonte:** um `grok agent stdio` de curta duração por origem: `initialize`, `_x.ai/billing`,
  stdin fechado. Nenhum `session/new`, nenhuma pergunta: não gasta cota. Timeout de 30 s.
- **Origens e contas:** as do chat (Windows, depois as distros rodando com `grok`, ADR-0019). A
  sonda confere se o `_x.ai/auth/info` dá um id de conta; se der, contas iguais em origens
  diferentes viram um bloco só ("2 origens"); se não, um bloco por origem.
- **Intervalos:** os do Claude (5 min, 2 min ao abrir o painel, 60 s no botão). O
  `subscription_watch_interval_secs: 60` que o `grok` anuncia é o que a própria TUI usa, então 5 min
  fica bem abaixo.
- **Erros:** sem `grok` → "Grok Build não encontrado no …"; sem login → "Faça login no Grok"; sem
  `creditUsagePercent` → "Formato de resposta desconhecido" (o chip mantém o último valor); com
  chave de API e sem assinatura, a sonda diz o que o `_x.ai/billing` devolve e o painel mostra o
  saldo em vez da %.
- **Quando roda:** só com o chip em algum layout (`BarConfig::uses("grok")`), numa thread
  `zefiro-grok-usage` em `platform-windows/src/grok_usage.rs`. O parser e as datas ficam em
  `crates/integrations/usage/src/grok.rs`.

## Quem paga e quais são os riscos

| Login na CLI | Quem paga | O que o Zefiro gasta |
|---|---|---|
| SuperGrok / X Premium+ / X Subscription | A assinatura do usuário. O chat do Zefiro tira da **mesma cota semanal** do grok.com, do Imagine e do Build. Acabou, para tudo até zerar (ou compra créditos extras, a partir de US$ 5). | Cada pergunta é um turno do Grok Build. O chip não gasta nada (`_x.ai/billing` não chama o modelo). |
| `XAI_API_KEY` | O usuário, por token, dos créditos pré-pagos do console.x.ai. Com o limite de fatura padrão (US$ 0), a xAI recusa os pedidos quando o crédito acaba; não há cobrança surpresa. | O mesmo turno, cobrado por token. |

Riscos:

- **Termos (médio):** dirigir o Grok Build pelo ACP é um uso documentado, mas nenhum texto da xAI
  autoriza explicitamente um app de terceiros a usá-lo como chat com a assinatura. As regras de
  plano mudaram duas vezes em 2026 (cota única em junho, X Subscription em outubro). No pior caso,
  a xAI pode limitar ou suspender a conta. O README ganha um "Aviso sobre a assinatura do Grok" no
  mesmo tom do aviso do Claude: o que roda, quem paga, que é zona cinzenta e como desligar (tirar
  os chips `grok` e `chat`, ou deixar o chat no Claude/Codex).
- **Custo (baixo, mas real):** o Grok 4.7 Fast custa o dobro; cada modelo tem o preço no nome na
  lista do `grok` ("Fast variant. 2x the price."), e o seletor mostra esse texto. A cota semanal não
  tem números publicados; o chip é a única forma de ver.
- **API não documentada (médio):** o `_x.ai/billing` é uma extensão fora da documentação ("treat it
  as non-exhaustive"). Se mudar, o chip mostra "Formato de resposta desconhecido", nunca um número
  errado. A sonda `grok_usage_probe` confere de ponta a ponta.
- **Dados (baixo):** em julho de 2026 o Grok Build enviou repositórios inteiros para a xAI sem
  pedir; foi corrigido no servidor. No chat, a pasta de trabalho é vazia e o `strict` limita a
  leitura. O que o usuário digita vai para a xAI de qualquer jeito, sob a configuração de
  "Coding data" da conta dele (`/privacy` na CLI).
- **Configuração do usuário (baixo):** hooks, plugins e MCP do `~/.grok` carregam no processo do
  chat (são do próprio usuário). Se um hook custar tokens a cada partida (como o plugin do Claude na
  ADR-0020), a sonda mostra e a tarefa registra.

## Logs e privacidade

- O log diz "chat: Grok iniciado (Ubuntu-24.04, grok 1.0.70)" e "uso do Grok: 1 conta (WSL),
  25 %". Nunca o texto das perguntas e respostas, o e-mail, o `sessionId` completo, os argumentos de
  ferramentas, o token ou a chave.
- O Zefiro não abre o `~/.grok/auth.json`; para dizer "sem login" ele usa a resposta do
  `authenticate`.
- O diagnóstico exportado (`diagnostics::sanitize`) já limpa o log; nada novo vai para o zip.

## Onde mora cada parte

| Parte | Lugar |
|---|---|
| `Agent::Grok`, argumentos, candidatos no Windows, prefixo de ambiente do WSL | `crates/integrations/chat/src/{agent,command}.rs` |
| Linhas do ACP (pedidos e respostas), modelos, erros | `crates/integrations/chat/src/grok.rs` (novo) |
| Processo, `GrokState`, respostas de permissão | `crates/platform-windows/src/chat.rs` |
| Seletor de 3 agentes, textos de erro | `crates/modules/bar/src/chat.rs`, `crates/app/src/chat_loop.rs` |
| `[chat.grok]`, `of(agent)` | `crates/core/src/config.rs` |
| Parser do `_x.ai/billing`, datas | `crates/integrations/usage/src/grok.rs` (novo) |
| Serviço do chip | `crates/platform-windows/src/grok_usage.rs` (novo), `crates/app/src/usage_loop.rs` |
| Chip, painel, catálogo | `crates/modules/bar/src/{catalog,controller,usage}.rs`, `crates/app/src/bar_view.rs` |
| Decisão | ADR-0027 (novo), adendo no fim da ADR-0020 e linha no `AGENTS.md` |
| Aviso | `README.md` |

A crate `windows` continua só em `platform-windows` e `app`.

## Verificação

- **Testes portáveis:** `Agent` com 3 valores e o `[chat] agent = "grok"`; `of("grok")`; os
  argumentos e o ambiente do `grok agent stdio` no Windows e no WSL; o parser do ACP com linhas da
  sonda (chunk, fim de turno, cancelado, pedido de permissão → rejeição, erro de cota, models/update);
  o parser do `_x.ai/billing` (com e sem saldo, formato desconhecido) e o "reinicia em".
- **Sonda `chat_probe`** com `ZEFIRO_CHAT_AGENT=grok` (e `ZEFIRO_CHAT_ORIGIN=Ubuntu-24.04`):
  responde, para no meio, lembra o contexto num processo novo (`session/load`), lê uma imagem, e
  **pedir para ler um arquivo não roda ferramenta nenhuma** (falha antes das três camadas, passa
  depois). Mede o primeiro texto frio e vivo, como a tabela da ADR-0020.
- **Sonda `grok_usage_probe`:** a % e o fim da semana batem com o `/usage` da TUI; tempo por
  origem; nenhum `session/new`.
- Depois das sondas, nenhum `grok` do Zefiro fica rodando no Windows nem na distro.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` e
  `scripts/check-windows-imports.ps1`.
- As sondas que mandam perguntas gastam um pouco da cota semanal do usuário: elas pedem para rodar
  antes.

## Perguntas para o usuário

1. Concorda em **não** ter campo de chave de API no Zefiro nesta versão (a chave, se houver, fica na
   CLI)?
2. O Grok no Windows: hoje só há `grok` no WSL. Quer instalar o Grok Build no Windows para testar as
   duas origens, ou o Windows fica só com os testes portáveis?
3. Se a sonda mostrar que não dá para tirar as ferramentas de leitura no ACP, prefere (a) aceitar
   com o `strict` e a pasta vazia, ou (b) cair para um `grok -p --tools …` por pergunta, mais lento
   (~partida a cada turno) mas sem ferramenta nenhuma?

## Decisões na implementação (2026-10-28)

As três perguntas foram resolvidas assim, e a sonda mudou duas coisas da spec:

1. **Sem campo de chave de API** no Zefiro: a chave, se houver, fica na CLI.
2. **O Windows não tem o `grok`**: a origem Windows só tem os testes portáveis; as sondas rodaram no
   Ubuntu-24.04.
3. **Ferramentas:** nem (a) nem (b). O `--deny '*'` e o `--disallowed-tools` antes de `agent` não
   valem no ACP (o `read_file` rodou), e o `always-approve` do usuário faz a sessão nascer com
   `yolo: true`, então recusar os pedidos de permissão não pega nada. Um **perfil de agente**
   (`--agent-profile`, `tools: todo_write`, `mcpInheritance: none`) tira as ferramentas: o modelo
   responde "sem ferramentas" ao pedido de ler um arquivo, e a entrada cai de ~19 mil para ~11 mil
   tokens.
4. **`GROK_SANDBOX=strict` saiu:** sem o bubblewrap, o `grok` se recusa a subir no Linux.
5. **Contas do chip** juntadas pelo `principalId` do `_x.ai/auth/info`.
6. **"reinicia"** segue a regra dos outros chips: data e hora para janelas de mais de um dia.
7. **Créditos extras** vão no subtítulo do bloco, só com saldo maior que zero (lidos como centavos;
   não verificado, o saldo do usuário é 0).
8. **Ícone do chip:** o glifo de robô do Segoe, sem o logo da marca.
