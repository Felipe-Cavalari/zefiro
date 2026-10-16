# Zefiro — Fase 1b: Modo de edição da barra (spec)

> **Data:** 2026-10-15
> **Status:** aprovada pelo usuário ao solicitar a implementação em 2026-10-15; plano de implementação aguardando aprovação.
> **Base:** [PLANNING](../../PLANNING.md) (§4.2–4.3 e §12), [spec da 1a](2026-10-15-fase-1a-barra-design.md), [passagem da 1a](../handoff/2026-10-15-fase-1a-codex.md), [ADR-0001](../../adr/0001-pipeline-de-renderizacao.md), [ADR-0006](../../adr/0006-ilha-de-midia-fork.md) e [ADR-0007](../../adr/0007-barra-fase-1a.md).

## 1. Objetivo

Permitir organizar a barra pelo próprio Zefiro: entrar em **Editar barra**, mover itens entre **Início · Centro · Fim**, adicionar ou remover itens pela galeria, ajustar suas opções e escolher entre um layout compartilhado e layouts próprios por monitor.

A edição mostra uma prévia na barra real. **Concluir** grava tudo de uma vez no `config.toml`; **Cancelar** devolve a configuração anterior. O arquivo continua legível, editável à mão e preserva comentários e chaves não reconhecidas.

A entrega prepara a barra para receber os widgets da 1d–1f e os itens declarativos da 1e, usando as mesmas regras de catálogo, layout e edição. A 1b entrega o editor com os itens que existem hoje, sem antecipar essas integrações.

## 2. Base existente e propostas desta spec

Já estão definidos pelo planejamento: edição por seções, arrastar para reordenar, galeria, opções por item, escolha de layout por monitor e gravação ao concluir. Permanecem as duas alturas, os dois formatos, os temas e a arquitetura nativa da 1a.

As decisões abaixo são **propostas para aprovação**, não decisões anteriores do usuário:

| Tema | Proposta |
|---|---|
| Transação | Uma sessão de edição para todo o app; prévia em memória, com Concluir e Cancelar explícitos. |
| Itens disponíveis | Zefiro e Relógio/calendário; uma ocorrência de cada por layout. |
| Ilha na 1b | Continua fixa no centro do primário, gerida por `[island]`. O editor a identifica, mas não a arrasta nem a duplica. |
| Centro ocupado pela ilha | Itens comuns podem ser configurados no centro, mas ficam ocultos no primário enquanto a ilha estiver habilitada; continuam visíveis nos monitores sem ilha. A interface explica essa condição antes da mudança. |
| Opções | Posição e presença pertencem ao layout; opções do relógio continuam globais em `[clock]`. |
| Identidade do monitor | Identidade persistente obtida pela plataforma; posição, índice de enumeração e `DISPLAY1/2` não são a identidade salva. |
| Conflito externo | Não sobrescrever alterações externas silenciosamente; oferecer recarregar ou reaplicar o rascunho sobre o arquivo atual. |
| Falta de espaço | Encurtar texto e usar um painel de itens excedentes, sem sobrepor seções. |

Ao aprovar esta spec, essas propostas passam a fazer parte do escopo. O plano de implementação será escrito e submetido à aprovação depois.

## 3. Escopo

**Dentro:**

1. Entrada e saída do modo de edição; sessão, prévia, cancelamento e persistência.
2. Layout portável com três seções e tratamento de falta de espaço.
3. Arrastar dentro da mesma barra, entre seções, e operações equivalentes por teclado.
4. Galeria local com adicionar, remover e opções dos itens implementados.
5. Layout compartilhado ou próprio por monitor, identificação persistente e comportamento ao reconectar.
6. Compatibilidade com o `config.toml` da 1a, recarga ao vivo e detecção de conflitos.
7. Validação funcional, visual e de desempenho; registro das decisões e atualização da documentação ao concluir a fase.

**Fora:**

- Porte, posicionamento livre, múltiplas instâncias ou edição interna da ilha C++ (1c).
- Widgets de áudio, brilho, uso de IA, projetos e demais integrações futuras.
- Parser, instalação ou execução de itens declarativos (1e); downloads e catálogo remoto (Fase 5).
- Temas por monitor, alturas por monitor e opções de relógio por ocorrência.
- Arrastar itens de uma tela para outra; selecionar outro monitor no editor cobre essa necessidade.
- App completo de configurações, atalhos globais novos e integração com komorebi.

## 4. Experiência de edição

### 4.1 Entrar

O menu do Zefiro, aberto pelo chip ou pelo botão direito na barra, ganha **Editar barra**. A ação funciona mesmo com todos os chips removidos: no formato pílulas, a área da faixa reservada recebe botão direito, sem capturar o botão esquerdo destinado às janelas de baixo. Esse caminho de recuperação precisa ser verificado com uma sonda de entrada, pois a região de entrada da 1a só cobre as áreas clicáveis.

Entrar fecha calendário ou menu aberto, guarda a janela que tinha foco e cria um rascunho a partir da configuração válida atual. Apenas uma sessão existe, mesmo que a ação seja acionada em outra tela. Nesse caso, ela seleciona a tela solicitada na sessão existente.

### 4.2 Apresentação

A barra selecionada destaca as três seções com contorno discreto e rótulos **Início**, **Centro** e **Fim**. Cada seção vazia tem um alvo de inserção. Os adornos de edição podem ocupar uma faixa auxiliar abaixo da barra, sem alterar a reserva da AppBar.

Um painel abaixo da barra oferece:

- seletor **Monitor**, com nome amigável, indicação do principal e número temporário para identificação visual;
- escolha **Mesmo layout em todos** ou **Layout próprio por monitor**;
- itens da seção selecionada, **Adicionar item**, opções e remoção;
- **Cancelar** e **Concluir**, com indicação de alterações pendentes.

Painel limitado à área útil do monitor, com rolagem quando necessário. Controles têm área de interação de pelo menos 28 × 28 DIP. Material, tipografia, cores, foco e raios vêm dos tokens existentes. O editor funciona nas quatro combinações de altura/formato e nos temas claro e escuro.

No modo compartilhado, mudanças aparecem em todas as barras. No modo próprio, aparecem somente na barra selecionada. A seleção de monitor não move o foco para uma janela de outro aplicativo.

### 4.3 Enquanto se edita

Clicar num chip o seleciona para editar; não abre seu painel habitual. Relógio e destaque de tema continuam recebendo atualizações. Painéis normais não abrem enquanto a sessão estiver ativa.

Clique fora ou Alt+Tab fecha subpainéis transitórios e suspende a interação, mas mantém o rascunho e a prévia. Ao retornar pelo menu, a sessão continua. Não há gravação implícita ao perder foco.

**Esc** cancela primeiro um arraste ou uma operação de teclado em andamento; depois fecha opções/galeria; no nível principal equivale a **Cancelar**. Fechar a janela do editor também cancela. Encerrar o app descarta a sessão e executa o encerramento normal, removendo AppBars e parando a ilha.

### 4.4 Concluir e cancelar

**Concluir** valida o rascunho inteiro, verifica conflitos e grava em uma única transação. Durante a gravação, os controles de confirmação ficam indisponíveis, com indicação de progresso. A sessão só termina quando a substituição do arquivo tiver sido confirmada.

Erro de validação, falta de permissão ou erro de gravação mantém o editor aberto e o rascunho recuperável; a mensagem explica o problema em pt-BR e permite tentar novamente ou cancelar. Nenhuma configuração parcial passa a ser a configuração confirmada.

**Cancelar** restaura a última configuração externa válida, que pode ter mudado desde a entrada, sem escrever no disco. Ambos os caminhos liberam captura, adornos e superfícies temporárias e devolvem o foco quando a janela anterior ainda existir.

## 5. Mover, adicionar e configurar itens

### 5.1 Arrastar

Pressionar num item e mover pelo menos 4 DIP inicia o arraste. Antes desse limite, soltar apenas seleciona. O item mantém seu espaço original até existir um destino válido; uma representação visual acompanha o ponteiro e um marcador mostra seção e índice de inserção.

O destino é calculado pelos pontos médios dos itens, com desempate estável. Cruzar uma seção vazia oferece índice zero. A representação arrastada usa posição direta do ponteiro; os outros itens mudam de posição com a mola de ajuste 280/24. Não se reinicia a mola quando o índice de destino não mudou.

Soltar num destino válido aplica ao rascunho; fora da barra selecionada, sobre um destino bloqueado ou após perda de captura cancela apenas o gesto. Arrastar para fora não remove. Esc, perda de captura, bloqueio de sessão ou destruição da janela sempre liberam a captura e eliminam o marcador.

### 5.2 Teclado

Tab/Shift+Tab percorrem os controles e itens; setas navegam nas listas. No item selecionado, **Espaço** inicia o movimento por teclado; esquerda/direita escolhem o índice e cima/baixo alternam Início, Centro e Fim. **Enter** confirma; **Esc** restaura a posição anterior ao movimento.

Adicionar, remover, escolher monitor, abrir opções, concluir e cancelar são acessíveis pelo teclado. O painel também oferece comandos explícitos **Mover para Início/Centro/Fim** e **Mover antes/depois**, evitando depender da descoberta dos atalhos. O foco fica visível e acompanha o item movido.

### 5.3 Galeria

Galeria local, com nome, descrição curta e estado de cada item. Na 1b:

| ID | Nome | Opções | Multiplicidade |
|---|---|---|---|
| `zefiro` | Zefiro | Explica que abre o menu; sem opções próprias nesta fase. | Uma por layout. |
| `clock` | Relógio e calendário | Formato e exibição de segundos. | Uma por layout. |

Adicionar insere no fim da seção selecionada. Um item já presente mostra **Já adicionado** e permite localizá-lo. Remover só altera o rascunho e preserva suas opções para uma adição posterior. É permitido remover o Zefiro e deixar todas as seções vazias, graças à entrada de recuperação por botão direito.

A ilha aparece como informação **Ilha de mídia — centro do monitor principal**, com explicação do limite da 1b. Não é um terceiro chip do catálogo. Se o executável estiver ausente ou a ilha tiver parado, a reserva lógica continua enquanto `[island] enabled = true`; o editor não altera o layout em resposta a uma falha do processo.

Nenhum widget futuro aparece como disponível. O catálogo tem descritores de ID, nome, descrição, disponibilidade, multiplicidade e opções; as telas não espalham decisões por comparações de strings.

### 5.4 Opções do relógio

Reaproveitar `clock.format` e `clock.seconds`. Campo de formato com prévia e ação **Usar formato do Windows** (string vazia). A opção de segundos deve atualizar o temporizador na prévia e ser revertida ao cancelar.

O formulário informa **Aplica-se a todos os relógios**. Formato inválido mostra erro e bloqueia Concluir; texto longo usa reticências na barra. O contrato de validação e formatação fica na integração de relógio, testável com uma implementação falsa de plataforma. Não introduzir uma segunda linguagem de formato.

## 6. Layout e convivência com a ilha

Início se ancora à esquerda, fim à direita e centro ao centro geométrico da barra. Ordem dos arrays é ordem visual da esquerda para a direita. Preservar métricas da 1a: margem externa de 8 DIP, intervalo de 4 DIP e alturas existentes. Seção vazia não cria pílula em uso normal.

No primário com a ilha habilitada, nenhum chip comum do centro é desenhado nem recebe entrada. Seu conteúdo fica preservado no arquivo; galeria, lista e alvo do centro explicam **Oculto neste monitor enquanto a ilha estiver ligada**. Esse destino exige uma ação explícita no painel, sem aceitar uma soltura que faria o item desaparecer inesperadamente. Nos demais monitores, ou com a ilha desligada, o centro funciona normalmente.

A ilha continua com o ciclo de vida, a geometria e o tema definidos na 1a. A prévia de reorganização não reinicia, esconde nem move o processo C++. Expandir a ilha continua sendo uma sobreposição transitória, como na 1a. Alterar esse contrato exige rever esta spec e registrar a decisão para a 1c.

### 6.1 Falta de espaço

O layout recebe uma área central reservada à ilha no primário. Antes do plano, uma sonda de geometria deve estabelecer o limite dos estados recolhido e espiando do fork e como obter suas mudanças por evento, sem polling por quadro. Não confundir a janela transparente da ilha com o retângulo visual da pílula. Se isso exigir uma adaptação mínima no fork, ela segue `#ifdef ZEFIRO_ISLAND` e `// ZEFIRO:`; nenhuma alteração de comportamento da mídia entra nessa adaptação.

Esse contrato é um **pré-requisito técnico a resolver no plano**, com medição registrada. Não considerar concluído o layout de três seções se a reserva for apenas uma largura estimada. Se não houver solução pequena, revisar a proposta antes da implementação dessa parte.

Sem ilha, a largura natural do centro é limitada a metade da largura útil; o restante é dividido igualmente entre início e fim. Com ilha, sua reserva central substitui essa largura. Folgas entre regiões são de pelo menos 8 DIP. Reserva maior que a tela ocupa toda a largura útil e envia os chips laterais para os excedentes.

Em cada seção, primeiro limitar textos a 160 DIP por chip, com reticências. Persistindo a falta de espaço, reservar um botão **Mais itens** e retirar itens a partir do fim da seção até caber. O painel de excedentes mantém a ordem e oferece as ações dos chips originais. No editor, os itens ocultos também aparecem na lista para reorganização.

Se nem os botões de excedentes couberem, mostrar um único botão de recuperação no início, com acesso aos itens e ao editor. Não desenhar alvos fora da tela ou sobrepostos. A ocultação por falta de espaço nunca altera os arrays salvos. As regras são determinísticas e testadas com larguras pequenas, textos longos, escalas fracionárias e coordenadas negativas de monitor.

## 7. Layouts por monitor

O padrão é **Mesmo layout em todos**. A resolução de layout é uma função portável: modo compartilhado usa o layout base; modo próprio usa a entrada do monitor, com fallback para o base.

Ao passar para **Layout próprio por monitor**, os monitores sem entrada própria recebem cópias do layout compartilhado no rascunho; entradas já existentes são recuperadas. Ao voltar para **Mesmo layout em todos**, o editor informa que o layout selecionado será usado em todos e copia esse layout para o base. As entradas próprias ficam preservadas para alternar de volta. Tudo só é persistido ao concluir.

Um monitor novo herda o layout base; um desconectado mantém sua configuração no arquivo. O editor não apaga entradas órfãs automaticamente.

### 7.1 Identidade

A plataforma fornece ID opaco persistente e nome amigável. A proposta é usar o caminho de dispositivo do monitor obtido pela configuração de displays do Windows; o plano deve validar sua estabilidade na máquina, incluindo dois monitores do mesmo modelo. Não usar apenas EDID/modelo, resolução, posição ou índice de enumeração como identidade.

Em identidade indisponível ou ambígua, usar o layout base e indicar **Identificação indisponível; usando layout compartilhado**, sem criar ou sobrescrever associação permanente. Não prometer que trocar cabo ou porta preservará a identidade; permitir copiar o layout de uma entrada desconectada para o monitor atual pelo editor.

Mudança de topologia ou DPI cancela o gesto em andamento e recria as janelas pelas regras da 1a, mantendo o rascunho por identidade. Se a tela selecionada sumir, selecionar o primário disponível; sem telas disponíveis, suspender a sessão até existir uma. A mudança do monitor principal recalcula onde o centro está reservado à ilha.

## 8. Configuração e compatibilidade

Estender o formato existente, sem renomear suas chaves. Exemplo proposto:

```toml
theme = "obsidian"

[bar]
height = "completa"
format = "faixa"
layout_mode = "shared" # shared | per_monitor

[bar.sections]
start = ["zefiro"]
center = []
end = ["clock"]

# IDs opacos reais são preenchidos pelo Zefiro.
[bar.monitors."monitor-exemplo".sections]
start = ["zefiro", "clock"]
center = []
end = []

[island]
enabled = true
exe = ""

[clock]
format = ""
seconds = false
```

Arquivo da 1a sem `layout_mode`, `center` ou `monitors` mantém exatamente o comportamento anterior: compartilhado, centro vazio, ilha no primário. Ler não migra nem regrava o arquivo. Arrays presentes e vazios significam seção vazia; chave ausente recebe o padrão documentado. Uma entrada própria exige as três seções, evitando herança parcial ambígua.

IDs dos itens são estáveis. Na 1b, são únicos nas três seções de cada layout; repetidos invalidam a edição/configuração, com mensagem indicando as ocorrências. IDs desconhecidos são preservados e mostrados como **Item indisponível**, removíveis pelo editor; não são desenhados como relógio, como pode acontecer no caminho genérico atual. Chaves de opções desconhecidas também são preservadas. Limites: 64 IDs por layout, 128 entradas de monitor e 256 bytes UTF-8 por ID de item; exceder os limites produz erro, sem truncamento silencioso.

### 8.1 Gravação e concorrência

O editor guarda o texto e a revisão de origem, além do modelo tipado. Revisão compara conteúdo do arquivo, não apenas data de modificação. A gravação usa `toml_edit`, altera somente os campos editados, preserva comentários e campos alheios e substitui o arquivo por temporário no mesmo diretório. Falhas deixam a versão anterior íntegra; uma gravação não é uma sequência de chamadas independentes a `set_value`.

Leitura e gravação ficam fora da thread de UI. O worker devolve resultado com geração da sessão; respostas antigas não encerram uma sessão nova. Gravação confirmada e seu evento do observador são reconciliados, sem reaplicar a prévia ou criar um conflito próprio.

Quando o arquivo muda durante a edição, manter o rascunho visível e avisar **A configuração mudou fora do editor**. Concluir fica bloqueado até escolher:

- **Recarregar arquivo:** descartar o rascunho e começar com a última versão válida.
- **Reaplicar minhas alterações:** aplicar somente os campos editados sobre a versão atual; preservar os demais. Para um mesmo array de seção alterado dos dois lados, mostrar o conflito e exigir escolha explícita entre a seção do arquivo e a do rascunho. Não concatenar arrays automaticamente.

Revalidar antes de gravar. A comparação final e a substituição devem proteger contra outro escritor no intervalo de gravação; se não for possível obter exclusão apropriada no Windows, abortar e pedir nova reconciliação, sem prometer mesclagem segura. Arquivo externo inválido não é sobrescrito pelo editor: mostrar erro e permitir cancelar ou aguardar sua correção.

Mudanças de tema e preferências do Windows podem atualizar a aparência do rascunho sem alterar sua organização. Um conflito externo de configuração continua sujeito às regras acima, mesmo quando seus efeitos visuais já puderem ser apresentados.

## 9. Arquitetura e entrada

| Camada | Responsabilidade acrescentada |
|---|---|
| `core` | Modelo de layouts, compatibilidade, validação, patch de documento e persistência transacional. |
| `platform` | Dados portáveis de identidade e capacidades de monitor, sem handles Windows. |
| `ui` | Layout de três seções, excedentes, alvos de inserção e geometria de edição. |
| `modules/bar` | Catálogo, máquina de estados da sessão, comandos de edição, galeria e formulários em `DisplayList`. |
| `integration-clock` | Validação do formato e prévia do relógio. |
| `platform-windows` | Identidade de display, captura do mouse, eventos de botão pressionado/solto e perda de captura, foco, regiões, composição e suporte à substituição segura do arquivo. |
| `app` | Orquestração, worker de persistência, resolução por monitor e aplicação das prévias. |

Nenhuma nova dependência de `windows` fora dos locais autorizados pelo AGENTS.md. A máquina de estados recebe eventos e produz dados/efeitos; não faz I/O nem chama Win32. Não é necessário implementar a árvore genérica de nós ou a linguagem de expressões nesta fase.

Preservar o modelo de duas janelas por barra/painel do ADR-0007. A janela de desenho deixa os cliques passar; entrada cobre apenas controles ativos. Não usar `HTTRANSPARENT` como substituto para passagem entre processos. A recuperação pelo botão direito pode exigir tratamento específico de entrada: demonstrar que o clique esquerdo continua chegando à janela de baixo, inclusive com layout vazio.

Molas no compositor, `Period = 1/ω`. Conteúdo redesenhado quando muda; movimento do ponteiro não deve rasterizar a barra inteira. Regiões de entrada precisam acompanhar os alvos que estão interativos durante a edição. Ao sair, nenhuma região temporária pode bloquear o desktop. Operações de fechamento usam gerações para impedir callbacks atrasados de esconder uma sessão nova.

## 10. Robustez e desempenho

Explorer reiniciado, tela cheia, bloqueio, mudança de DPI e remoção de monitor não confirmam nem perdem silenciosamente o rascunho. Antes de esconder/recriar uma janela, cancelar o gesto e liberar a captura. Tela cheia mantém a política da 1a; ao voltar, o editor pode ser retomado.

Sem animações do Windows, posições mudam imediatamente e o gesto continua funcional. Transparência desligada produz material sólido. Exe da ilha ausente não impede edição de início/fim nem gravação.

Metas do processo da barra, com ilha desligada, no mesmo ambiente da medição da 1a:

| Métrica | Critério |
|---|---|
| Primeiro quadro | ≤ 250 ms; carregar o editor apenas quando solicitado. |
| Repouso após fechar o editor | Memória privada ≤ 15 MB e CPU média ≤ 0,05%, como a 1a; registrar também working set. |
| Abrir editor ou galeria | Comando → conteúdo pronto para composição ≤ 16 ms após aquecimento; registrar também abertura a frio. |
| Gesto | Processamento de evento de movimento/decisão de destino p95 ≤ 4 ms; sem I/O na UI. |
| Animação | Meta de zero quadros perdidos na taxa nativa; medir com PresentMon e registrar limitações da atribuição ao DWM. |
| Ciclos de uso | 50 entradas/saídas sem crescimento de janelas, captura presa ou recursos vivos do editor; registrar memória antes/depois. |

Registrar separadamente memória com editor aberto e pico de uso; não tratá-los como memória de repouso. A meta de 15 MB já está no limite na 1a: se não for atingida, investigar/liberar recursos e submeter qualquer mudança de orçamento ao usuário, sem redefinir a métrica. Esta fase não declara atingida a meta global de 35 MB do PLANNING, que inclui todos os widgets e ainda depende da 1c.

Antes de medir, fechar o YASB e reabrir no fim. Testes que movam mouse, enviem teclas ou reiniciem a barra exigem combinar um momento com o usuário. Sair normalmente ou enviar `WM_CLOSE` à janela correta; preservar `%APPDATA%\zefiro\` e `codex-backup\`.

## 11. Validação e critérios de aceite

Implementar com TDD: primeiro teste falhando, depois código e teste passando. Para entrada real, usar sonda que demonstre a falha antes e o comportamento correto depois; screenshots sozinhas não provam passagem de clique, captura ou foco.

| Cenário | Resultado exigido |
|---|---|
| Configuração da 1a sem novas chaves | Mesmo layout e comportamento, sem regravação na leitura. |
| Mover e reordenar por mouse e teclado | Mesmos arrays finais, sem duplicação; gesto cancelado restaura posição. |
| Remover tudo, concluir e abrir novamente | Menu de recuperação acessível; clique esquerdo atravessa as áreas vazias. |
| Adicionar/remover e mudar opções | Prévia correta; cancelar restaura layout e temporizadores; concluir persiste em uma transação. |
| Centro com e sem ilha, primário e secundário | Regra de reserva/ocultação explícita, sem chips invisíveis recebendo cliques. |
| Layout próprio, troca de primário e reconexão | Associação por identidade, entradas desconectadas preservadas e fallback previsível. |
| Nomes de display/ordem mudam | Layout não segue índice nem `DISPLAY1/2`. Identidade ambígua usa base. |
| IDs desconhecidos, repetidos e limites excedidos | Preservação de desconhecidos; erros claros para duplicação/limites; nunca virar relógio por fallback. |
| Comentários e chaves externas ao editor | Preservados ao concluir, incluindo alterações concorrentes não conflitantes. |
| Alterar o arquivo durante edição/gravação | Conflito reconhecido; nenhuma perda silenciosa; versão externa inválida preservada. |
| Falha de gravação e resposta de sessão antiga | Rascunho continua disponível; arquivo íntegro; resposta atrasada não fecha nova sessão. |
| Layout estreito, texto longo e escala 100/125/150% | Excedentes acessíveis, nenhum alvo sobreposto ou fora da tela. |
| Perda de captura, Alt+Tab, Esc e fechamento rápido | Nenhum gesto, região, foco ou callback fica preso. |
| Explorer, DPI, Win+P e bloqueio | Janelas reconstruídas corretamente e rascunho preservado. |

Testes portáveis cobrem validação, resolução por monitor, transações, reconciliação, estado do editor e geometria com medições de texto determinísticas. Testes de integração cobrem persistência em diretório temporário, injeção de falhas e ordenação de respostas do worker. Sondas Windows cobrem roteamento de entrada, captura, foco, reconstrução e identidade dos monitores.

Checklist visual: quatro combinações de altura/formato, Obsidian e Porcelain, duas telas, galeria, opções, seção vazia, arraste, foco por teclado, aviso de conflito e excedentes. Comparar calendário e menu normais após sair para detectar regressões.

Verificações obrigatórias ao implementar:

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pwsh -NoProfile -File scripts/check-windows-imports.ps1
```

Manter o check das crates portáveis no Linux conforme o CI. Registrar medições reais, verificações manuais feitas e pendentes; não declarar uma validação visual ou de desempenho apenas porque os testes unitários passaram.

## 12. Entrega e sequência

A próxima etapa é aprovar esta spec. Depois, escrever o plano em tarefas pequenas, cada uma com seu teste e comando de verificação, incluindo primeiro a resolução dos contratos de identidade de monitor, reserva da ilha e entrada de recuperação. Não começar a implementação antes da aprovação do plano.

Durante a execução, manter o ledger em `.superpowers/sdd/<plano>/progress.md`, com tarefas, commits, testes e rulings. Ao fim, revisar a branch inteira contra esta spec, corrigir Critical e Important com regressões e entregar a lista de rulings e Minor adiados.

Registrar as decisões implementadas em um novo ADR, preservando os ADRs anteriores, atualizar o PLANNING e escrever o handoff da 1b com o estado real da entrega. A documentação deve destacar a restrição temporária do centro no primário, que a 1c poderá substituir por uma ilha integrada ao layout.
