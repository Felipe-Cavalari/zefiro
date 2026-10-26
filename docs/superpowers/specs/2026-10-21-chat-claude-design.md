# Chat com o Claude na barra — design

Data: 2026-10-21. Pedido: "lançar um chat pelo Zefiro", na leitura de um chat de verdade num painel
da barra: o usuário digita no painel e a resposta aparece ali, nativo, sem WebView. O usuário pediu
para implementar direto, sem a rodada de perguntas; as decisões abaixo são do agente e ficam
marcadas como tal.

## Intenção

Uma pergunta rápida ao Claude sem sair do que se está fazendo: clicar no chip, digitar, ler a
resposta e voltar. Não é um substituto do Claude Code nem do app do Claude: é o "pergunta de
corredor" que hoje exige abrir um terminal ou o navegador.

## Metas

- Chip `chat` ("Chat com o Claude") em qualquer seção; clique abre o painel do chat.
- Resposta em streaming, palavra por palavra, com "Pensando…" até chegar o primeiro texto.
- Conversa de vários turnos com contexto; continua viva ao fechar e reabrir o painel e ao reiniciar
  o Zefiro (a última conversa).
- Usa a assinatura do usuário pelo Claude Code instalado (`claude`), sem chave de API.
- Conta do Windows ou do WSL.
- Campo de texto de verdade: acentos e teclas mortas, IME, emoji (Win+.), colar várias linhas,
  setas, Home/End, Ctrl+setas, Ctrl+Backspace, Ctrl+A, Enter envia, Shift+Enter quebra linha.
- Rolagem pela roda; a conversa acompanha o fim enquanto a resposta chega, a menos que o usuário
  tenha subido.
- Parar uma resposta no meio; começar uma conversa nova; copiar uma resposta.

## Não metas

- Ferramentas: o Claude do chat não lê arquivos, não roda comandos, não busca na web.
- Lista de conversas antigas, anexos, imagens, Markdown rico (tabelas, negrito de verdade).
- Seleção de texto com o mouse nas mensagens e no campo.
- Chave de API ou chamada HTTP direta à API.

## Anti-metas (seria um fracasso mesmo funcionando)

- Derrubar ou mexer no login do Claude Code (o Zefiro nunca lê nem grava o token para o chat; quem
  autentica é o próprio `claude`).
- Ligar uma distro do WSL parada sem o usuário ter escolhido o WSL.
- Janela de console piscando a cada mensagem.
- Barra travando enquanto a resposta chega (todo E/S fora da thread da interface).
- Texto da conversa em log, diagnóstico ou HUD.

## Como funciona

### Backend: `claude -p` com entrada e saída em stream-json (decisão do agente)

Um processo `claude` por conversa, vivo enquanto ela é usada:

```
claude -p --input-format stream-json --output-format stream-json --verbose
       --include-partial-messages --tools "" --strict-mcp-config
       --disable-slash-commands --setting-sources ""
       --append-system-prompt "<prompt do chat>"
       (--session-id <uuid> | --resume <uuid>) [--model <modelo>]
```

- Medido nesta máquina (Claude Code 2.1.307): o primeiro turno leva ~6 s com o processo frio
  (quase tudo é a partida); com o processo vivo, ~1–3 s. Por isso o processo **sobe ao abrir o
  painel** (sem custo de tokens: nada é pedido até a primeira mensagem) e fica vivo; morre depois de
  10 min sem uso, ao começar uma conversa nova e ao sair do Zefiro.
- Cada mensagem é uma linha `{"type":"user","message":{"role":"user","content":"…"}}` no stdin.
- Da saída, interessam: `stream_event` com `content_block_delta`/`text_delta` (o texto chegando),
  `assistant` (o texto completo, caso não venham parciais) e `result` (fim do turno; `is_error`
  e o texto do erro). O resto é ignorado.
- Parar: `{"type":"control_request","request_id":"…","request":{"subtype":"interrupt"}}`. O turno
  termina com `result` de erro (`error_during_execution`) e o processo segue vivo para o próximo.
- `--setting-sources ""`, `--strict-mcp-config` e `--disable-slash-commands` deixam de fora hooks,
  plugins, MCP e skills do usuário: partida mais rápida e um chat que não dispara automações do
  Claude Code. `--tools ""` tira as ferramentas.
- A sessão é gravada pelo próprio `claude` (`--session-id` na primeira partida, `--resume` nas
  seguintes), numa pasta só do Zefiro: `%LOCALAPPDATA%\zefiro\chat` no Windows e
  `~/.cache/zefiro-chat` no WSL. Assim o contexto sobrevive ao processo morrer por inatividade, a
  um travamento e a reiniciar o Zefiro.
- Prompt do chat (acrescentado ao do Claude Code): responder em português do Brasil, curto e
  direto, em texto simples (sem tabelas nem cabeçalhos), porque o painel é pequeno e não desenha
  Markdown.

### Conta: Windows ou WSL (decisão do agente)

`[chat] origin` no `config.toml`:

- `auto` (padrão): o `claude.exe` do Windows, se existir; senão, a primeira distro **rodando**
  (fora `docker-desktop*`) que tenha `claude`. Nunca liga uma distro parada.
- `windows`: só o Windows.
- `wsl`: a distro padrão do WSL (pode ligá-la; o usuário escolheu).
- qualquer outro valor: o nome da distro (pode ligá-la).

No Windows, o executável é procurado em `%USERPROFILE%\.local\bin\claude.exe` (instalador nativo),
`%APPDATA%\npm\node_modules\@anthropic-ai\claude-code\bin\claude.exe` (npm) e no `PATH`; roda com
`CREATE_NO_WINDOW`. No WSL: `wsl.exe -d <distro> -- bash -lc 'mkdir -p ~/.cache/zefiro-chat && cd
~/.cache/zefiro-chat && exec claude …'` (shell de login para achar o `claude` do usuário), com os
argumentos escapados para o bash.

`[chat] model`: vazio (padrão) usa o modelo que o Claude Code usaria; qualquer valor vai em
`--model`.

### Interface

- Chip `chat`: ícone de balão. Ponto de destaque no chip enquanto o Claude responde ou quando uma
  resposta chegou com o painel fechado (some ao abrir).
- Painel de 420 DIP: cabeçalho ("Claude", a origem em letra pequena — "Windows" ou o nome da
  distro — e o botão "Nova conversa"), a conversa, e o campo embaixo com o botão de enviar (vira
  "parar" enquanto responde).
- Mensagens: as do usuário à direita, em `accent_soft`; as do Claude à esquerda, em `raised`, com
  um ícone de copiar embaixo. Quebra de linha por palavra (palavras maiores que a linha quebram por
  letra), feita na crate portável com o `TextMeasurer`.
- Limpeza leve do Markdown que escapar do prompt: `**x**`/`__x__` e crases viram texto,
  `# Título` perde o `#`, `- item`/`* item` viram `• item`, as linhas ```` ``` ```` somem.
- A área da conversa cresce com o conteúdo até 460 DIP; daí em diante, rola. A roda anda 3 linhas.
- O campo cresce até 6 linhas e depois rola por dentro para manter o cursor à vista. Até 8000
  caracteres. Cursor fixo (sem piscar: nada é redesenhado por timer).
- Estados: vazio ("Pergunte qualquer coisa ao Claude"), iniciando, pensando, respondendo, erro (a
  mensagem de erro aparece como uma resposta, em cor secundária). Erros conhecidos em português:
  `claude` não encontrado, não logado, limite de uso, processo que caiu.
- Esc fecha o painel (a conversa e o rascunho ficam). Clique fora também.

### Teclado (decisão do agente)

O painel já recebe `WM_CHAR` e `WM_KEYDOWN` (a busca do VS Code usa isso). O campo é texto + cursor
na crate portável, sem o controle EDIT do Windows: o EDIT é uma janela GDI por cima do compositor,
não anima com a mola do painel e não segue o tema. O IME entrega o texto pronto por `WM_CHAR`;
emoji chega como par de surrogates, juntado no campo.

### Persistência e privacidade

- `%LOCALAPPDATA%\zefiro\chat\conversa.json`: as mensagens, o id da sessão e a origem da última
  conversa. "Nova conversa" apaga.
- O log registra só eventos ("chat: processo iniciado (Windows)", "resposta em 2,1 s", erros), nunca
  o texto de perguntas e respostas.

### Onde fica cada parte (regra da crate `windows`)

- `crates/integrations/chat` (portável): protocolo (linhas de entrada, leitura dos eventos),
  argumentos e escape para o bash, escolha da origem, a conversa (mensagens, estados, JSON).
- `crates/modules/bar/src/chat.rs` (portável): o campo (edição), quebra de linha, limpeza do
  Markdown e o desenho do painel com as áreas de clique.
- `crates/platform-windows/src/chat.rs`: o processo (pipes, thread de leitura, `post_user`) e a
  procura do `claude.exe`; área de transferência para colar e copiar.
- `crates/app/src/chat_loop.rs`: liga tudo no laço de eventos.

## Testes

TDD nas partes portáveis (protocolo com linhas reais capturadas, argumentos e escape, escolha da
origem, conversa, edição, quebra de linha, layout do painel). Sonda `chat_probe` em
`platform-windows/examples` para o caminho inteiro (processo real, pergunta, streaming, parar).

## Deixado para quem constrói

Medidas finas do painel, ícones exatos, o texto do prompt do chat, a política de coalescer os
redesenhos durante o streaming.

## Segunda rodada: Codex, origem, modelo e conversas antigas (2026-10-21)

Depois de usar o painel, o usuário pediu: escolher o WSL e o modelo pela interface, ver conversas
antigas como no `/resume` (só as feitas no chat da barra, decisão dele) e conversar também com o
Codex.

### Codex pelo `codex app-server` (decisão do agente)

- `codex exec --json` só entrega a resposta inteira no fim (~6 s). O `codex app-server` (JSON-RPC
  por linhas no stdin/stdout, o mesmo que os editores usam) manda `item/agentMessage/delta`, aceita
  `turn/interrupt`, retoma por `thread/resume` e devolve os modelos por `model/list`. Medido: ~5 s
  por resposta no WSL (0.163) e no Windows (0.149).
- Sequência: `initialize`, notificação `initialized`, `model/list`; na primeira pergunta,
  `thread/start` (`sandbox = "read-only"`, `approvalPolicy = "never"`, `developerInstructions` = o
  prompt do chat) e depois `turn/start` com o modelo escolhido; numa conversa antiga,
  `thread/resume` ao subir. O id da conversa no Codex é o `thread.id`.
- O modelo vai sempre explícito, tirado do `model/list` (padrão = o `isDefault`): o Codex 0.149 do
  Windows não aceita o `gpt-6.1-sol` do `config.toml` do usuário, e a lista dele só traz o que ele
  aceita.
- `codex.exe`: `%LOCALAPPDATA%\Programs\OpenAI\Codex\bin\codex.exe` ou o `PATH`; no WSL,
  `exec codex app-server` no shell de login, na mesma pasta do chat.

### Interface

- Cabeçalho: "Claude | Codex" (segmentos), e à direita os botões de conversas antigas (relógio) e
  de conversa nova (+). Embaixo, dois seletores: origem e modelo.
- Origem: "Automático", "Windows" (se o executável existe) e as distros **rodando** que têm o
  `claude`/`codex` (descobertas numa thread ao abrir o seletor). Trocar de origem ou de agente começa
  uma conversa nova (a sessão só existe onde nasceu); com a conversa vazia, só troca.
- Modelo: Claude = Padrão, Opus, Sonnet, Haiku (aliases do `--model`); Codex = o `model/list`. Trocar
  no meio da conversa mantém o contexto: o Claude reinicia o processo com `--resume --model`, o Codex
  manda o modelo no próximo `turn/start`.
- Conversas antigas: lista (mais recente primeiro) com o título (a primeira pergunta), agente,
  origem e data; clique reabre e continua; lixeira apaga a cópia do Zefiro (a sessão do
  `claude`/`codex` fica). Esc ou a seta voltam ao chat.
- As escolhas ficam em `[chat] agent` e em `[chat.claude]`/`[chat.codex]` (`origin`, `model`),
  gravadas pelo painel sem fechá-lo (como o `[terminal]`). Reabrir uma conversa antiga seleciona o
  agente, a origem e o modelo dela.

### Gravação

- Uma conversa por arquivo: `%LOCALAPPDATA%\zefiro\chat\conversas\<id>.json` (id do Zefiro; a
  sessão do agente fica dentro). O `conversa.json` da primeira rodada vira um arquivo desses.
- Ao abrir o painel pela primeira vez, volta a conversa mais recente.
