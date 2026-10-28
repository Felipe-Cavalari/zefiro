# ADR-0026: Remoção da ilha C++ e do motor externo

- **Status:** aceito
- **Data:** 2026-10-28
- **Substitui:** a [ADR-0006](0006-ilha-de-midia-fork.md) (ilha como fork em processo separado) e a parte da [ADR-0009](0009-ilha-embutida-na-barra.md) que mantinha o fork com `engine = "externa"`.
- **Spec:** [Remover a ilha C++](../superpowers/specs/2026-10-28-remover-ilha-cpp-design.md)
- **Plano:** [2026-10-28-remover-ilha-cpp](../superpowers/plans/2026-10-28-remover-ilha-cpp.md)

## Contexto

Desde a ADR-0009 a ilha padrão é a embutida em Rust, e o fork em C++ do Dynamic Island for Windows (`island/`, cerca de 25 mil linhas) só rodava com `[island] engine = "externa"`. Ninguém usava a externa: a config do usuário nem tinha a chave `engine`. Mesmo assim, o fork era perto de 40 % do código do repositório, e o AGENTS.md abria dizendo que a ilha era um fork em C++. Por isso, quem analisava o projeto, gente ou IA, lia o Zefiro como um fork da ilha.

## Opções consideradas

1. **Só ajustar os docs** e marcar `island/` como `linguist-vendored`: rápido, mas as 25 mil linhas continuavam no repositório.
2. **Mover o fork para outro repositório** e baixar o exe como opcional: mantinha um motor que ninguém usa, e o custo de manter dois motores.
3. **Remover o fork e o motor externo:** o Zefiro fica todo em Rust. Escolhida.

## Decisão

Saem a pasta `island/`, o crate `zefiro-island-host`, o `engine`/`exe` do `[island]` e tudo que só existia por causa da externa: o processo e a política de reinício, a ordem de janelas (`island_zorder`), a reserva do centro, a mensagem de geometria, o destaque lido do `island-state.ini`, "Reiniciar a ilha" (Configurações e menu da bandeja), o centro travado no modo de edição e a exceção do ocultar automaticamente.

- **Config antiga:** `engine` e `exe` são ignorados; `removed_island_keys` faz o log avisar uma vez no início. O arquivo do usuário não é reescrito, e o `island.ini` e o `island-state.ini` antigos ficam onde estão.
- **Instalador:** não leva mais o `zefiro-island.exe` e apaga o de uma instalação antiga (`[InstallDelete]`).
- **Créditos:** o README cita o Dynamic Island for Windows como inspiração. O `THIRD_PARTY_NOTICES.md` mantém o aviso MIT, com o texto da licença, porque trechos da lógica (molas, tokens, calendário, tempos dos estados, visualizador, analisador e cor da capa) foram reescritos em Rust a partir dele.

## Recursos que deixaram de existir

Só existiam na ilha C++ e não foram portados: pesquisa rápida (Ctrl+Alt+Espaço), anel de progresso externo, bateria fraca, bandeja de arquivos, sobreposição de jogo, fixar expandido (Ctrl+Alt+D), letras na pílula recolhida, duas pílulas lado a lado e os visualizadores LED, plasma e orbe. O usuário não usava nenhum deles e preferiu não portar.

## Consequências

- O repositório fica todo em Rust (menos 25.860 linhas), e o CI não compila mais o `zefiro-island-host`.
- O `scripts/stability.ps1` só conta os eventos do `zefiro.exe`.
- Minor: o parâmetro `island_width` de `zefiro_ui::bar_layout::layout_sections` (reserva do centro para a ilha externa) ficou, sempre `None` no app.
- O histórico do git continua com o fork, o que é o certo: quem olhar commits antigos vê de onde veio.
