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
