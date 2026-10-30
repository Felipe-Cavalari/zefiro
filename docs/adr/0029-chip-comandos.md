# ADR-0029: Chip `commands`, a cola de comandos para devs

- **Status:** aceito
- **Data:** 2026-10-30
- **Spike/branch:** main

## Contexto

No meio do trabalho o usuário esquece um comando ("como apago o cache do Docker?", "como subo o
FastAPI com o uvicorn?") e hoje abre o navegador. Ele quer um chip com um dropdown de ferramenta
(Docker, Linux, PowerShell, Python…), a busca logo abaixo e a lista de comandos com uma linha do que
cada um faz. Spec: `docs/superpowers/specs/2026-10-30-chip-comandos-design.md`; plano:
`docs/superpowers/plans/2026-10-30-chip-comandos.md`.

## Opções consideradas

1. **API de documentação de todas as linguagens.** Não existe uma. O DevDocs serve JSONs estáticos de
   centenas de linguagens, mas é referência de API (páginas longas), em inglês, e pede rede.
2. **tldr-pages** (CC BY 4.0, `pt_BR` parcial) como base. Cobre comandos, mas descreve em inglês: a
   busca por intenção em português ("limpar cache") não acharia nada. Pede atribuição e um download.
3. **Pacote curado em português, embutido no executável**, mais arquivos do usuário. Escolhida.

## Decisão

- Um pacote de cerca de 40 comandos por ferramenta (Docker, Linux, PowerShell, Python com FastAPI e
  uvicorn, Git) em TOML dentro do executável, escrito em português. Cada comando tem `cmd`, `what`,
  `keywords` de intenção e `popular`. Todo comando que apaga ou sobrescreve dados começa o `what` com
  `⚠ ` (um teste confere).
- O usuário põe `*.toml` no mesmo formato em `%APPDATA%\zefiro\commands\` (até 64 arquivos de 256 KB);
  `cmd` igual na mesma ferramenta vale sobre o pacote. O Zefiro só lê essa pasta.
- Busca portável na crate `zefiro-integration-commands`: sem acento nem maiúsculas, palavras vazias
  em português não contam, termos com 5+ letras casam pelo prefixo de 4 ("apago" acha "apagar") e a
  ordem é `keywords` > `cmd` > `what`.
- Painel em `zefiro-bar::commands` (estado, teclado e `DisplayList`), sem depender da crate de
  comandos; o `app` (`commands_loop.rs`) liga os dois, copia e grava `[commands] last`.
- Copiar fecha o painel na hora, sem quebra de linha no fim do texto.
- `Family::Mono` (Cascadia Mono, ou Consolas) entra em `zefiro_platform::text` para o comando.
- O dropdown aberto cobre a busca e os resultados e eles deixam de ser desenhados (os tokens do tema
  são translúcidos); a altura do painel não muda.
- O glifo do chip é `\u{E943}` (`</>`): o `\u{E756}` (`>_`) já é o do chip Terminal.
- O tldr-pages fica para uma fase 2, se o usuário quiser (download na primeira vez, em inglês,
  atribuição CC BY 4.0).

## Consequências

- Painel fechado = zero custo: sem serviço, thread nem rede. Os dados são lidos ao abrir.
- A qualidade da busca depende das `keywords` do pacote; o usuário amplia pelos próprios arquivos.
- Novas ferramentas (npm, Cargo, WSL, SQL…) entram adicionando um TOML em `data/` e na lista de
  `builtin()`.
