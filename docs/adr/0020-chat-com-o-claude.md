# ADR-0020: Chat com o Claude num painel da barra

- **Status:** aceito
- **Data:** 2026-10-21
- **Spike/branch:** main

## Contexto

O usuário quis "lançar um chat pelo Zefiro": digitar no painel de um chip e ler a resposta ali,
nativo, sem WebView, usando a assinatura do Claude (sem chave de API), com a conta do Windows ou
do WSL (`docs/superpowers/specs/2026-10-21-chat-claude-design.md`). A barra não tinha campo de
texto multilinha, rolagem por pixels nem um processo filho de longa duração.

## Opções consideradas

1. **Backend:** API HTTP direta (precisa de chave; o token OAuth do Claude Code não pode ser usado
   nem renovado pelo Zefiro, ADR-0011) ou **o `claude` CLI**. Dentro do CLI: um `claude -p` por
   pergunta com `--resume` (~6 s por turno, quase tudo partida) ou **um processo vivo com
   `--input-format stream-json`** (~1–3 s por turno depois do primeiro).
2. **Campo de texto:** o EDIT do Windows (`text_input.rs`, já usado no formato do relógio) ou
   **texto e cursor na crate portável, desenhados no painel** (como a busca do VS Code). O EDIT é
   uma janela GDI por cima do compositor: não anima com a mola do painel e não segue o tema.
3. **Conta:** só Windows, ou **Windows e WSL** pelo `wsl.exe` com shell de login.

## Números medidos

| Métrica | Windows | WSL (Ubuntu-24.04) |
|---|---|---|
| Primeiro texto, processo frio | 3,0 s | 3,5 s |
| Primeiro texto, processo vivo | 2,5 s | 3,3 s |
| Primeiro texto, processo novo retomando a sessão | 2,9 s | 4,0 s |
| Parar no meio (`interrupt`) | ok, processo segue vivo | ok |
| Contexto depois de `--resume` | lembrou | lembrou |

Ambiente: Windows 11 Pro 26200, Claude Code 2.1.307, sonda `chat_probe`, 2026-10-21.

## Decisão

Um processo `claude -p` com stream-json na entrada e na saída por conversa, sem ferramentas e sem
as configurações do usuário (`--tools "" --setting-sources "" --strict-mcp-config
--disable-slash-commands`), que sobe ao abrir o painel, sai depois de 10 min sem uso e retoma a
sessão gravada pelo próprio `claude` (`--session-id`, depois `--resume`) numa pasta só do chat. O
campo é desenhado pelo painel. A origem é `[chat] origin` (`auto` = Windows; sem ele, uma distro
**rodando** com `claude`).

## Consequências

- A sessão de `--resume` só existe na origem em que foi criada: trocar a origem (ou uma sessão
  apagada) dá "No conversation found"; o laço cria outra sessão e reenvia a pergunta uma vez. O
  inverso ("already in use") retoma.
- Sem `--setting-sources ""` o `claude` carrega hooks e plugins do usuário (um SessionStart de plugin
  custou ~11 mil tokens de cache por partida na medição).
- O processo filho sai sozinho quando o stdin fecha (é assim que o serviço o encerra, com `kill`
  só depois de 1,5 s), então o Zefiro fechando ou caindo não deve deixar `claude` órfão. Conferido
  depois da sonda: nenhum `claude` do chat ficou rodando, nem no Windows nem na distro.
- Sem seleção com o mouse nem Markdown rico: o prompt pede texto simples e `plain_text` limpa o que
  escapar. Ctrl+A, Ctrl+C (sem seleção, copia a última resposta) e o ícone de copiar cobrem o uso.
- O cursor do campo não pisca: nada é redesenhado por timer (ADR-0007).

## Adendo (2026-10-21): Codex, seletores e conversas antigas

- **Codex pelo `codex app-server`**, não pelo `codex exec --json`: o `exec` só entrega a resposta
  inteira no fim; o `app-server` (JSON-RPC por linhas) manda `item/agentMessage/delta`, aceita
  `turn/interrupt`, retoma por `thread/resume` e lista os modelos. Sonda `chat_probe` com
  `ZEFIRO_CHAT_AGENT=codex`: Windows (0.149) e WSL (0.163) responderam, pararam e lembraram o
  contexto num processo novo.
- O modelo do Codex vai sempre explícito: o `config.toml` do Codex do usuário pede `gpt-6.1-sol`,
  que o 0.149 do Windows recusa. O serviço guarda o padrão do `model/list` e o usa quando a
  interface ainda não escolheu.
- Uma conversa por arquivo em `%LOCALAPPDATA%\zefiro\chat\conversas\<id>.json` (id do Zefiro; a
  sessão do agente fica dentro); o `conversa.json` antigo é migrado. A lista de conversas antigas
  mostra só as do chat da barra (escolha do usuário).
- Agente, origem e modelo ficam em `[chat] agent` e `[chat.claude]`/`[chat.codex]`, gravados pelo
  painel sem fechá-lo (`Config::same_except_panels`). Trocar de agente ou de origem começa outra
  conversa; trocar de modelo, não (o Claude reinicia com `--resume --model`).
- O automático tenta primeiro o que `[chat.<agente>] prefer` diz (`windows`, o padrão, ou `wsl`) e
  depois o outro; a chave fica na página de origem ("No automático, primeiro: Windows | WSL").
  Mudá-la com a origem no automático pode mudar o processo de lugar, então começa outra conversa.
- **Imagens (Ctrl+V):** sem texto na área de transferência, o Ctrl+V cola uma imagem (formato
  "PNG", arquivo de imagem copiado no Explorer ou `CF_DIB`, que vira `.bmp` com o cabeçalho de 14
  bytes). Com texto, o texto ganha: o Excel e o Word copiam os dois. Numa thread, a imagem vira PNG
  com o lado maior em até 1568 px (`imaging::to_png`), vai para `chat\imagens\<id>.png` e a
  pergunta guarda só o id. Até 4 por pergunta; só imagem, sem texto, vale. O Claude recebe blocos
  `image` em base64 na mensagem do stream-json; o Codex, `{"type":"image","url":"data:image/png;base64,…"}`
  no `turn/start` (vale no Windows e no WSL, sem traduzir caminhos). Sonda `chat_image_probe`: o
  Claude e o Codex (Windows e WSL) leram o número das imagens de teste; um BMP de 11 MB virou um PNG
  de 16 KB em ~50 ms. Apagar a conversa apaga as imagens dela.
- **Imagens geradas pelo Codex:** o `item/completed` com `item.type == "imageGeneration"` traz o
  PNG em base64 no `result` (o `savedPath` é do lado do Codex e não vale no WSL, então não é
  usado). O laço dá um id novo, liga o id à resposta (`Message.images` da mensagem do agente, como
  nas perguntas) e uma thread grava o PNG sem reduzir em `chat\imagens\<id>.png` (o original, para
  abrir no visualizador) e decodifica a versão da tela (800 px). No balão da resposta, a imagem vem
  depois do texto, na largura do balão e com até 260 DIP de altura; enquanto gera
  (`item/started`), um lugar reservado diz "Gerando imagem…". Clique em qualquer imagem da conversa
  (pergunta ou resposta) abre o arquivo no app padrão do Windows e fecha o painel. Ela volta ao
  reabrir a conversa e vai embora junto ao apagá-la. Sonda `chat_generated_probe`.
- **Várias mensagens no mesmo turno do Codex:** um turno pode ter mais de um `agentMessage` (o
  aviso "Vou usar a skill…" e a resposta final). Os deltas trazem `itemId`; quando o item muda e a
  resposta já tem texto, entra uma linha em branco. Um `agentMessage` de outro item que chega
  inteiro, sem deltas, também é acrescentado (antes ele era descartado).
- **Biblioteca de imagens** (spec `2026-10-22-biblioteca-de-imagens-do-chat-design.md`): o botão
  de imagem no cabeçalho abre a página "Imagens", com uma grade de 3 colunas (3 fileiras à vista,
  a roda rola de fileira em fileira) das imagens de todas as conversas, geradas e coladas, da
  conversa mais recente para a mais antiga. Não há índice novo: a lista sai dos JSON das conversas
  ao abrir a página (`history::library`), sem as imagens cujo arquivo sumiu. Só as fileiras à
  vista e a próxima são decodificadas, a 256 px; a memória das miniaturas guarda o lado em que
  cada uma foi lida e relê a que está menor que o pedido. Clique abre no visualizador; o botão no
  canto do quadro (com o mouse em cima) reabre a conversa de origem; "Abrir pasta" abre
  `chat\imagens`. Apagar uma imagem sozinha ficou de fora.

## Adendo de 2026-10-28: Grok

O chat ganhou um terceiro agente, o Grok, pelo `grok agent stdio` do Grok Build (ACP), com um
perfil de agente que tira as ferramentas. A decisão e as medidas estão na
[ADR-0027](0027-grok-no-chat-e-chip-de-uso.md). `[chat] agent` aceita `grok`, e `[chat.grok]` tem
`origin`, `prefer` e `model`, como os outros. O `ChatConfig::of` passou a receber o nome do agente.
