# Zefiro — Fase 1b: Plano de implementação do modo de edição

> **Data:** 2026-10-15
> **Status:** implementação e verificações automatizadas entregues na branch `codex/phase-1b-bar-editor`; verificações de ambiente pendentes conforme o [handoff](../handoff/2026-10-15-fase-1b-codex.md).
> **Spec:** [Modo de edição da barra](../specs/2026-10-15-fase-1b-modo-de-edicao-design.md), aprovada pelo pedido de implementação nesta conversa.

## Objetivo e restrições

Entregar a edição nativa da barra descrita na spec: três seções, galeria, opções, mouse e teclado, layouts por monitor, prévia reversível e confirmação transacional. A ilha permanece no processo C++ e no centro do primário.

Executar sequencialmente, sem delegação automática. Criar a branch `codex/phase-1b-bar-editor` a partir da `main` atual após a aprovação deste plano. Preservar alterações preexistentes em README, assets e scripts de identidade visual; não incluí-las nos commits da fase. Não fazer push nem criar PR.

Antes de implementar, reler AGENTS.md, PLANNING, ADRs 0001/0006/0007 e handoff da 1a. O caminho de worktree no handoff é histórico; esta fase usa o checkout informado pelo usuário nesta conversa. Não trabalhar na branch histórica `fase-1a/barra`.

Cada tarefa começa com teste que falha, recebe implementação mínima e termina com os testes passando. Para comportamento do Windows, uma sonda com janela auxiliar em outro processo é a evidência de entrada; enviar mensagens diretamente ao wndproc não prova passagem de clique. Nunca mover mouse, enviar teclas ou reiniciar a barra em uso sem combinar um momento com o usuário.

Ledger: `.superpowers/sdd/2026-10-15-fase-1b-modo-de-edicao/progress.md`. Registrar `Task N: complete (commits …, tests: … → …)` e desvios como `Task N: Ruling: decisão — motivo — custo se errado`. Tarefa com verificação real pendente não fica marcada como inteiramente concluída. Commits em inglês; documentação, UI e logs em pt-BR.

## Arquitetura e contratos

| Área | Arquivos existentes e novos previstos | Contrato |
|---|---|---|
| Configuração | `core/src/config.rs`, novo `core/src/config_edit.rs`, testes em `core/tests/` | `LayoutMode`, seções, mapas por ID de monitor, revisão por conteúdo e patches de campos. |
| Monitor | novo `platform/src/monitor.rs`, `platform-windows/src/monitors.rs` | ID opaco opcional, nome amigável, retângulo, DPI, primário e estado de ambiguidade. |
| Layout | `ui/src/bar_layout.rs`, novo `ui/src/edit_layout.rs` | Três regiões, chips visíveis, excedentes, alvos de inserção e reserva da ilha. |
| Editor | novos `modules/bar/src/{catalog,editor,editor_panel}.rs` | Máquina de estados portável, rascunho, campos alterados, comandos, foco e listas de desenho. |
| Relógio | `integrations/clock/src/{format,windows}.rs` | Formatação/previsão com erro explícito; formato continua sendo padrão de data do Windows. |
| Entrada | `platform-windows/src/window.rs`, possível novo `bar_context.rs` | Pressionar/soltar, captura, perda de captura, texto, modificadores e recuperação por botão direito. |
| Persistência | novo `app/src/config_worker.rs`, possível novo `platform-windows/src/config_file.rs` | Worker assíncrono por canal, revisão esperada, substituição segura e resposta com geração. |
| Integração | `app/src/{event_loop,bar_view,panel_view,robustness}.rs`, novo `editor_view.rs` | Config confirmada separada da prévia; resolução por monitor e ciclo de vida de superfícies. |
| Ilha | `modules/island-host`, `app/src/island.rs`, adaptação mínima em `island/` se necessária | Reserva visual válida dos estados recolhido/espiando publicada por evento. |

Esses nomes orientam os limites; ajustes de assinatura devem ser registrados no ledger. Não implementar uma árvore genérica de UI, linguagem de expressões ou novas integrações. `windows` só nos locais autorizados. `unsafe` pequeno e documentado com `// SAFETY:`.

## Task 1 — Estabelecer os três contratos de plataforma

**Arquivos:** sondas novas em `scripts/probes/`, testes em `platform-windows`, relatório em `.superpowers/sdd/2026-10-15-fase-1b-modo-de-edicao/platform-contracts.md`.

1. Escrever sondas para identidade de dois monitores, retângulo visual da ilha e entrada na faixa vazia. Usar fixtures e janelas auxiliares para execução isolada; marcar explicitamente os passos que precisam da barra/ilha real.
2. Identidade: correlacionar enumeração atual com configuração de displays e caminho de dispositivo. Demonstrar distinção de monitores iguais, ordem invertida e indisponibilidade; não salvar `DISPLAY1/2` como chave.
3. Ilha: localizar no fork a geometria visual recolhida/espiando e seus limites durante a transição. Determinar publicação por evento e valor inicial/conservador válido, inclusive após reinício. Janela transparente inteira não vale como reserva visual.
4. Recuperação: demonstrar botão direito em barra vazia e entrega do clique esquerdo ao outro processo. Avaliar observação de botão direito sem captura da área vazia; qualquer hook deve ter callback curto, instalação limitada e remoção comprovada. Não usar `HTTRANSPARENT` para outros processos.
5. Registrar resultados e a solução escolhida. Se um contrato exigir mudança de produto ou adaptação extensa do fork, propor ajuste da spec antes das tarefas dependentes.

**Verificar:** `cargo test -p zefiro-platform-windows`; cada sonda terá `-Verify` e código de saída não zero quando o contrato falhar. Registrar os comandos exatos de cada sonda no relatório, antes de executá-la. Não marcar a tarefa completa apenas por compilar sondas.

**Commit:** `test(platform-windows): add bar editor platform contract probes`.

## Task 2 — Estender a configuração sem alterar o padrão da 1a

**Arquivos:** `core/src/config.rs`, `core/tests/config.rs`, novo `core/tests/layout_config.rs`.

1. Testes de arquivo legado, arrays explicitamente vazios, `center`, `shared/per_monitor`, entradas completas, duplicação nas três seções, desconhecidos e limites.
2. Acrescentar modo padrão compartilhado, centro vazio e mapa de monitores. Exigir três seções em entradas próprias, mantendo padrões do layout base legado.
3. Validar 64 IDs por layout, 128 monitores e 256 bytes por ID de item; mensagens identificam tabela e motivo. Preservar IDs desconhecidos no modelo.
4. Ler não deve criar migração nem regravar arquivos existentes.

**Verificar:** `cargo test -p zefiro-core --test config`; `cargo test -p zefiro-core --test layout_config`.

**Commit:** `feat(core): add backward compatible monitor layouts`.

## Task 3 — Resolver identidade e layouts por monitor

**Arquivos:** `platform/src/monitor.rs`, `platform/src/lib.rs`, `platform-windows/src/monitors.rs`, novo módulo portável de resolução no `core`.

1. Testar resolução compartilhada/própria, ausência de associação, identidade ambígua, monitor desconectado, mudança de índice/primário e duas telas do mesmo modelo.
2. Implementar a identidade validada na Task 1; manter nomes amigáveis separados das chaves.
3. Implementar função pura que escolhe o layout por ID, retornando também motivo de fallback. Topologia não apaga entradas salvas.

**Verificar:** `cargo test -p zefiro-core`; `cargo test -p zefiro-platform-windows monitors`; sonda de identidade da Task 1.

**Commit:** `feat(platform-windows): expose persistent display identities`.

## Task 4 — Patches de TOML, revisões e reconciliação

**Arquivos:** novo `core/src/config_edit.rs`, export em `core/src/lib.rs`, novo `core/tests/config_edit.rs`.

1. Criar fixtures com comentários, opções desconhecidas, monitor desconectado e alterações concorrentes.
2. Modelar revisão pelo texto completo e conjunto explícito de campos editados; arrays são unidades de conflito, com inserção/remoção de tabela também representadas.
3. Gerar um documento final via `toml_edit`, preservando decoração dos valores substituídos e tabelas alheias. Não encadear `set_value` para confirmar uma sessão.
4. Mesclar campos independentes; retornar conflitos estruturados nos mesmos campos; escolhas explícitas resolvem os conflitos antes de nova validação.
5. Testar que cancelar não produz patch e que alteração externa inválida não pode ser alvo de gravação.

**Verificar:** `cargo test -p zefiro-core --test config_edit`.

**Commit:** `feat(core): add transactional config patches and conflict detection`.

## Task 5 — Persistência segura em worker

**Arquivos:** novo `app/src/config_worker.rs`, suporte de arquivo em `platform-windows` se necessário, testes de integração em diretório temporário.

1. Testar falhas de leitura, temporário, escrita e substituição; escritor concorrente entre revisão e substituição; duas respostas em ordem invertida.
2. Implementar protocolo `Save { generation, expected_revision, patch }` e resultados de sucesso, conflito e falha. Nenhuma API de COM do compositor vai ao worker.
3. Implementar exclusão e substituição conforme contrato validado no Windows. Demonstrar o comportamento com escritor não cooperante; se a exclusão não puder ser garantida, abortar sem substituir. Não usar um mutex apenas interno como prova contra editores externos.
4. Remover apenas temporários criados pela própria tentativa. Não apagar arquivos do usuário nem usar o config real nos testes.
5. Confirmar configuração em memória somente com sucesso do worker; reconciliar o evento do observador pelo texto/revisão gravados.

**Verificar:** `cargo test -p zefiro-app config_worker`; `cargo test -p zefiro-platform-windows config_file` quando o módulo existir; teste de corrida com dois processos auxiliares em diretório temporário.

**Commit:** `feat(app): save editor changes with revision checked worker`.

## Task 6 — Catálogo e estados fundamentais da sessão

**Arquivos:** novos `modules/bar/src/{catalog,editor}.rs`, `modules/bar/src/lib.rs`.

1. Testes de sessão única, seleção, adicionar/remover, ID indisponível, multiplicidade e reversão.
2. Catálogo com `zefiro/clock`, descrições, opções e disponibilidade; ilha apenas informativa.
3. Sessão com configuração base confirmada, rascunho, revisão externa, campos sujos, geração e estados de edição/salvamento/conflito.
4. Comandos portáveis produzem efeitos: aplicar prévia, pedir gravação, liberar captura e restaurar foco. Reentrada seleciona monitor sem criar sessão nova.
5. Esc hierárquico, cancelar, fechar e sucesso/falha de confirmação. Resposta de geração antiga é ignorada.

**Verificar:** `cargo test -p zefiro-bar editor`; `cargo test -p zefiro-bar catalog`.

**Commit:** `feat(bar): model transactional editor sessions and item catalog`.

## Task 7 — Layout compartilhado/próprio dentro da sessão

**Arquivos:** `modules/bar/src/editor.rs` e testes portáveis.

1. Testar cópia inicial para monitores sem entrada, recuperação de entradas existentes, volta ao compartilhado usando monitor selecionado e preservação de órfãos.
2. Implementar troca de modo como alteração do rascunho, com mensagem que explica a cópia. Identidade indisponível não pode criar chave persistente.
3. Copiar layout de monitor desconectado para o atual por ação explícita. Cancelar reverte a operação inteira.
4. Topologia atualiza a seleção e cancela gesto, preservando rascunho por ID; nenhuma tela suspende sessão.

**Verificar:** `cargo test -p zefiro-bar editor`.

**Commit:** `feat(bar): edit shared and per monitor layouts`.

## Task 8 — Geometria de três seções e excedentes

**Arquivos:** `ui/src/bar_layout.rs`, testes existentes e novos.

1. Antes de alterar as APIs, testar três seções, ordem, quatro combinações, centro condicionado à ilha, desconhecidos sem chip e reserva central.
2. Implementar regiões com margem externa/intervalos da spec, limite do centro e divisão lateral. Resultado inclui os excedentes, sem modificar o modelo salvo.
3. Testar reticências a 160 DIP, remoção a partir do fim para o botão Mais itens e botão único de recuperação quando nem os botões cabem.
4. Usar medições determinísticas e gerar casos de largura 0/pequena, reserva maior que a tela e escala fracionária. Nenhum retângulo clicável tem área negativa ou sai da região útil.

**Verificar:** `cargo test -p zefiro-ui bar_layout`.

**Commit:** `feat(ui): lay out three bar sections with overflow`.

## Task 9 — Reserva visual da ilha por evento

**Arquivos:** integração `app/src/island.rs`, `modules/island-host` e adaptação mínima do fork definida pela Task 1.

1. Testar reserva inicial, mudanças recolhido/espiando, transição, processo ausente, reinício e mudança de DPI. A reserva deve cobrir o percurso de animação, não só a largura final.
2. Implementar o contrato medido, com geração/estado obsoleto tratados. Falha do processo não habilita os chips centrais enquanto `[island] enabled = true`.
3. Mudanças no fork ficam dentro de `#ifdef ZEFIRO_ISLAND`, marcadas `// ZEFIRO:`. Não adicionar laço de polling na barra.
4. Compilar o fork somente após combinar interrupção se o exe em uso precisar ser fechado. Não encerrar a ilha automaticamente para permitir link.

**Verificar:** `cargo test -p zefiro-island-host`; sonda de geometria da Task 1; `./island/build.ps1` quando houver alteração C++.

**Commit:** `feat(island-host): track visual island reservation for bar layout`.

## Task 10 — Operações de movimento por teclado e mouse

**Arquivos:** `ui/src/edit_layout.rs`, `modules/bar/src/editor.rs`.

1. Testar limiar de 4 DIP, índices por ponto médio, empate estável, seção vazia, soltura inválida, cancelamento e centro bloqueado pela ilha.
2. Implementar alvos e marcador como dados. Soltura inválida não altera arrays; comando explícito permite mover ao centro com aviso de ocultação.
3. Implementar Espaço/setas/Enter/Esc e comandos Mover antes/depois/para seção com as mesmas operações de arrays usadas pelo arraste.
4. Mudança de índice só gera novo efeito de animação quando destino muda. Foco acompanha o item por ID, não por índice antigo.

**Verificar:** `cargo test -p zefiro-ui edit_layout`; `cargo test -p zefiro-bar editor`.

**Commit:** `feat(bar): support drag and keyboard item movement`.

## Task 11 — Eventos, captura e recuperação pela barra vazia

**Arquivos:** `platform-windows/src/window.rs` e suporte definido pela Task 1.

1. Acrescentar botão pressionado, captura/perda, modificadores, Tab, entrada textual e eventos necessários ao editor. Coordenadas de captura podem ser negativas e cruzar o monitor.
2. Testes garantem que hwnd de entrada é traduzido ao dono e que captura é liberada em cancelamento/destruição. Nunca repetir clique de término para o painel normal.
3. Implementar o mecanismo de recuperação demonstrado na Task 1, com ciclo de vida explícito. Encerrar observação quando não existir barra; respeitar fullscreen e sobreposições do próprio app.
4. Executar sonda com janela de outro processo: direito abre menu, esquerdo chega embaixo, regiões temporárias desaparecem ao sair.

**Verificar:** `cargo test -p zefiro-platform-windows window`; sonda de entrada da Task 1 com barra vazia em ambos os formatos.

**Commit:** `feat(platform-windows): add editor input capture and empty bar recovery`.

## Task 12 — Painel, galeria e foco

**Arquivos:** `modules/bar/src/editor_panel.rs`, `menu.rs`, `controller.rs`, novo `app/src/editor_view.rs`, `panel_view.rs`.

1. Testar layout do painel, hit-test, rolagem, controles de pelo menos 28 DIP, navegação Tab/Shift+Tab, seleção de monitor e foco após remover item.
2. Adicionar Editar barra ao menu e desenhar painel, rótulos, marcadores, galeria e mensagens com tokens existentes e DisplayLists portáveis.
3. Reaproveitar infraestrutura de painel e composição; distinguir subpainel transitório de sessão. Fechar subpainel não cancela a sessão implicitamente.
4. Implementar indicador de ilha fixa e centro oculto. Itens indisponíveis aparecem removíveis, nunca como relógio.
5. Callback de fechamento inclui geração; abrir/fechar rapidamente não esconde uma sessão nova. Material sólido e transições instantâneas respeitam preferências do Windows.

**Verificar:** `cargo test -p zefiro-bar`; `cargo test -p zefiro-app editor_view`; inspeção visual no checklist da Task 17.

**Commit:** `feat(bar): render native editor panel and item gallery`.

## Task 13 — Opções do relógio com validação e prévia

**Arquivos:** `integrations/clock/src/{format,windows}.rs`, `editor_panel.rs`, `editor.rs`.

1. Testar padrão vazio, formato personalizado, erro do formatador, texto longo e opções globais. A implementação falsa permite testar erro sem Windows.
2. Fazer o formatador Windows retornar erro verificável em vez de converter falha em texto vazio; preservar o formato de data aceito hoje.
3. Formulário de texto com entrada Unicode, seleção e comandos usuais de edição; considerar controle nativo de texto da plataforma, sem WebView. Explicar Aplica-se a todos os relógios.
4. Prévia altera temporizador de segundos; cancelar restaura. Erro bloqueia Concluir e aparece junto do campo; Usar formato do Windows limpa o padrão.

**Verificar:** `cargo test -p zefiro-integration-clock`; `cargo test -p zefiro-bar editor`.

**Commit:** `feat(clock): validate and preview editor clock options`.

## Task 14 — Aplicar prévias e ações de excedentes no app

**Arquivos:** `app/src/{app,event_loop,bar_view,panel_view,editor_view}.rs`, controlador de excedentes no módulo bar.

1. Testar separação entre config confirmada e prévia, entrada única, calendário bloqueado durante edição e restauração do estado ao cancelar.
2. Aplicar layout resolvido por monitor; abandonar o fallback que desenha todo ID desconhecido como relógio. Desenhar e animar grupos e chips das três seções.
3. Integrar painel Mais itens com ordem original e ações de menu/calendário. Itens centrais ocultos pela ilha não entram como excedentes acidentais.
4. Durante arraste, mover representação por transformação; redesenhar apenas conteúdo alterado e redirecionar molas de ajuste no compositor.
5. Aplicar prévia de opções, gerar efeitos de salvar no worker e confirmar apenas após sucesso. Sair libera recursos sem escrever rascunho.

**Verificar:** `cargo test -p zefiro-app`; `cargo test -p zefiro-bar`; `cargo build -p zefiro-app`.

**Commit:** `feat(app): integrate editor previews and three section bars`.

## Task 15 — Conflitos externos e confirmação

**Arquivos:** `event_loop.rs`, `config_worker.rs`, `editor.rs`, `editor_panel.rs` e testes.

1. Testar alteração externa válida/inválida, arquivo removido, mudanças só em comentários, alteração do mesmo array e novas mudanças durante reconciliação.
2. Observador entrega revisão ao editor; mantém rascunho e bloqueia confirmação até Recarregar arquivo ou Reaplicar minhas alterações. Campo conflitante exige escolha explícita.
3. Configuração externa válida vira a base de cancelamento. Tema/preferências podem atualizar apresentação sem reorganizar o rascunho.
4. Diferenciar evento da própria gravação pela revisão exata; não ignorar eventos por um timeout genérico.
5. Falha de worker mantém o editor aberto e reabilita tentativa. Confirmar desativa controles até resultado; cancelar durante gravação deve ser explicitamente indisponível até conhecer se a substituição ocorreu, evitando anunciar reversão após escrita efetiva.

**Verificar:** `cargo test -p zefiro-core --test config_edit`; `cargo test -p zefiro-bar editor`; `cargo test -p zefiro-app config_worker`.

**Commit:** `feat(app): reconcile external edits before saving bar layouts`.

## Task 16 — Reconstrução, suspensão e encerramento

**Arquivos:** `app/src/robustness.rs`, `event_loop.rs`, `editor_view.rs`, testes/sondas.

1. Testar mudança de primário, desaparecimento da tela selecionada, nenhum monitor, DPI e callbacks de janela antiga.
2. Antes de esconder/recriar, liberar captura e cancelar gesto; manter rascunho, seção selecionada e foco lógico por ID.
3. Alt+Tab suspende; Esc obedece à hierarquia; fechar editor cancela; encerrar barra executa shutdown normal. Tela cheia mantém política da 1a.
4. Sonda de 50 ciclos deve contar janelas/superfícies e verificar entrada do outro processo depois da saída. Mensagens sintéticas de display são complemento, não substituto de Win+P real.

**Verificar:** `cargo test -p zefiro-app`; sondas de captura/reconstrução em ambiente combinado com o usuário.

**Commit:** `fix(app): preserve editor drafts across display lifecycle changes`.

## Task 17 — Verificação integrada, visual e de desempenho

**Arquivos:** sondas em `scripts/probes/`, resultados ignorados pelo Git, ledger.

1. Rodar checks obrigatórios e registrar saídas: `cargo test --workspace`; `cargo clippy --workspace --all-targets -- -D warnings`; `pwsh -NoProfile -File scripts/check-windows-imports.ps1`. Executar check das portáveis no Linux pelo ambiente disponível/CI; indisponibilidade fica explícita.
2. Combinar janela de testes com o usuário antes de mexer na barra, mouse, teclas, Explorer ou telas. Usar diretório temporário/config de teste para testes destrutivos de arquivo; não substituir o config pessoal para facilitar automação.
3. Checklist: quatro alturas/formatos × Obsidian/Porcelain; duas telas e DPI fracionário; vazio; centro; galeria; opções; arraste; teclado; rolagem; excedentes; erro de gravação; conflito; retomada; menu/calendário normais após sair.
4. Medir reserva da ilha recolhida/espiando com as seções laterais, identidade na reconexão, clique atravessando, captura e foco com sondas reais.
5. Fechar YASB antes de medir, reabrir no fim. Com ilha desligada: primeiro quadro ≤250 ms, repouso privado ≤15 MB/CPU ≤0,05%, abertura aquecida ≤16 ms, evento de movimento p95 ≤4 ms. Registrar abertura a frio, memória aberta/pico e após 50 ciclos.
6. PresentMon elevado e taxa nativa para quadros perdidos; registrar comando e resultado reais. Se depender de elevação indisponível, entregar como pendência, sem alegar meta atingida.

**Commit:** `test(app): verify bar editor integration and performance`.

## Task 18 — Revisão contra a spec e documentação final

**Arquivos:** novo ADR com próximo número disponível, `docs/PLANNING.md`, novo handoff da 1b, spec/plano e ledger.

1. Revisar a branch completa contra cada seção e linha da matriz de aceite da spec. Classificar Critical/Important/Minor; corrigir Critical/Important com regressão que falhe antes.
2. Reexecutar checks completos após correções; repetir medições apenas quando a correção afetar seus resultados.
3. ADR registra decisões implementadas, contratos de plataforma, medições, divergências e restrição do centro até a 1c. Não sobrescrever decisões dos ADRs anteriores silenciosamente.
4. Atualizar status real da 1b no PLANNING e descrever configuração e recuperação; preservar a ordem das próximas fases.
5. Handoff identifica branch/commits, comandos, aprovações, resultados, validações pendentes e todos os rulings/Minor adiados. Não declarar a fase concluída se requisito obrigatório permanecer sem implementação.

**Verificar:** `git diff --check`; checks finais da Task 17; conferência dos links locais e matriz de cobertura.

**Commit:** `docs(adr): document phase 1b bar editor decisions and validation`.

## Aprovação e pontos de controle

O pedido de implementação aprovou a spec. Este plano ainda precisa de aprovação conforme AGENTS.md, Fluxo de trabalho, passo 2. Após a aprovação, executar sem pedir nova confirmação para decisões rotineiras dentro desse escopo.

Pontos que exigem retorno ao usuário: alteração material de produto para resolver os contratos da Task 1; mudança de orçamento; momento para testes que interferem no PC; impedimentos de permissão/elevação que não possam ser resolvidos em ambiente isolado. Não são motivo para interromper trabalho independente que esteja autorizado.
