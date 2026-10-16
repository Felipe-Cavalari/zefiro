# ADR-0008: Edição transacional e layouts por monitor na barra

- **Status:** aceito
- **Data:** 2026-10-15
- **Branch:** `codex/phase-1b-bar-editor`
- **Spec:** [Fase 1b](../superpowers/specs/2026-10-15-fase-1b-modo-de-edicao-design.md)
- **Plano:** [Implementação da Fase 1b](../superpowers/plans/2026-10-15-fase-1b-modo-de-edicao.md)

## Contexto

A 1a entregou a barra com seções início/fim, mas o centro ainda era exclusivo da ilha C++. A 1b precisa permitir organizar os itens visualmente, configurar o relógio e manter layouts próprios por monitor, preservando a edição manual do TOML e a arquitetura dos ADRs 0001/0006/0007.

O usuário aprovou a spec, o plano e os testes de interação nesta sessão. Durante a implementação, aprovou explicitamente a concorrência otimista descrita abaixo em lugar da exigência inicial de exclusão absoluta de outros editores.

## Opções consideradas

1. Gravar a cada gesto. Simples, mas torna cancelamento e alterações externas difíceis de distinguir.
2. Manter rascunho em memória e gravar ao concluir. Permite prévia reversível e confirmação única; escolhido.
3. Identificar monitor por índice ou `DISPLAY1/2`. Rejeitado porque a ordem e o nome GDI não são a associação persistente do layout.
4. Usar o retângulo da janela da ilha para reservar o centro. Rejeitado: o palco transparente do fork é maior que sua pílula.
5. Implementar entrada de texto no compositor. Rejeitado para o campo de formato: seleção, clipboard, IME e Unicode já existem no controle EDIT do Windows.

## Decisões

### Modelo portável e prévia

`core` conhece as três seções, o modo compartilhado/próprio e o mapa de monitores. A configuração da 1a continua válida e não é regravada ao ler. Arrays vazios são intencionais; entradas próprias exigem as três seções; IDs desconhecidos permanecem no arquivo e aparecem como indisponíveis no editor.

O módulo `bar` mantém uma sessão com configuração confirmada, rascunho, revisão de origem, revisão externa e campos alterados. Mover, adicionar e remover produzem alterações no rascunho. A aplicação resolve o layout de cada monitor e apresenta a prévia real. Cancelar volta à última versão externa válida, inclusive depois de uma edição manual concorrente.

As operações de mouse e teclado usam os mesmos arrays e regras. O foco é associado ao ID do item e recalculado após movimentos. Representação arrastada e marcador usam visuais do compositor; reposicionamento do conteúdo usa molas nos componentes de `Offset`.

### Persistência e conflitos

Um worker faz leitura e gravação do editor. `ConfigPatch` aplica apenas campos modificados usando `toml_edit`, preserva comentários e chaves futuras, identifica conflito por valor semântico e exige escolha nos campos alterados dos dois lados. Arquivo externo inválido bloqueia recarregar/reaplicar/concluir sem destruir o rascunho.

A revisão esperada é comparada ao texto completo, antes da preparação e imediatamente antes da substituição por temporário no mesmo diretório. O temporário é exclusivo, sincronizado e removido se a tentativa falhar. O resultado inclui geração; respostas antigas não encerram sessões novas. O observador reconhece a revisão gravada, e o encerramento pede uma releitura para captar mudanças posteriores.

**Limitação aceita pelo usuário:** existe uma pequena janela entre a última comparação e a substituição atômica, na qual um editor externo não cooperante ainda pode salvar. Não se afirma que um mutex interno elimina essa corrida. A troca foi aprovada nesta conversa e incorporada à spec §8.1.

### Monitores e ilha

A plataforma correlaciona o nome GDI da tela com seu caminho de interface por `EnumDisplayDevicesW` e `EDD_GET_DEVICE_INTERFACE_NAME`. Esse caminho, normalizado, é a chave opaca. Nomes amigáveis e posições são apresentados separadamente. Caminhos indisponíveis ou duplicados resultam em fallback para o compartilhado; não criam associação permanente ambígua.

Na máquina de teste, as duas telas expuseram caminhos distintos (AOC2702 e GSM5BB2). Troca de ordem e colisões são cobertas por testes; reconexão física/troca de porta continua pendente. Não se promete conservar o caminho ao mudar a conexão física.

A ilha continua fixa no primário. Itens comuns do centro ficam preservados, mas ocultos nesse monitor enquanto a ilha estiver habilitada. O editor informa a condição e não aceita uma soltura que esconda o item; o comando explícito de mover permite configurar esse centro para outros monitores.

O fork publica a propriedade `Zefiro.ReservedWidth` e avisa a barra por mensagem registrada quando o limite muda. No material padrão, o limite é **880 × escala física**, exatamente o clamp de `Renderer::Render`, cobrindo também o overshoot. No caminho legado sem palco fixo, o limite conservador é a largura do monitor. Até a publicação ou com exe antigo/ausente, a barra usa a reserva conservadora e mantém acesso aos itens pelos excedentes. Não há consulta de geometria por quadro nem largura estimada da pílula.

### Entrada, foco e recursos

Preservam-se as duas janelas do ADR-0007. Um hook `WH_MOUSE_LL` observa exclusivamente botão direito nas faixas vazias cadastradas de barras visíveis, encaminhando a recuperação do menu. Eventos alheios, incluindo clique esquerdo, seguem para o Windows. O hook é removido ao encerrar. A sonda em dois processos falhou sem ele e passou com ele: esquerdo entregue ao outro processo; direito entregue à recuperação.

O campo de formato usa EDIT em uma pequena janela GDI própria, associada ao painel. Um filho GDI direto da janela `WS_EX_NOREDIRECTIONBITMAP` recebia texto, mas ficava invisível; isso foi encontrado na inspeção visual, reproduzido por teste e corrigido. Fechar seu host encaminha o fechamento ao editor. Foco, captura e campo são liberados antes da recriação dos pais.

Concluir/cancelar e fechamento de painéis usam gerações globais. Um callback antigo não esconde um painel reaberto ou uma janela recriada que reutilize um handle. Após o fechamento assentar, a superfície grande é substituída por 1×1; caches Direct2D/DXGI ociosos são liberados depois de 1 s sem painel/editor aberto. Grupos da barra são criados somente quando há uma pílula a desenhar.

`ZEFIRO_CONFIG_DIR` permite testar barra e ilha em perfil isolado, sem substituir a configuração pessoal. O padrão permanece `%APPDATA%\zefiro`. O perfil não cria uma segunda identidade de instância única: não iniciar dois Zefiros simultâneos supondo isolamento de mutex.

## Validação e medições

Ambiente: Windows 11, duas telas a 100%; build release. Medições da barra isolada, sem ilha e sem YASB, em Obsidian/faixa completa. Valores em MB seguem o script `perf.ps1` (divisão por 1 MB do PowerShell).

| Verificação | Resultado |
|---|---|
| Testes do workspace | **141 passaram**; incluem configuração, conflito durante preparação, falha de substituição, sessão, foco, limites, camadas, geometria, captura, campo Unicode e geração de fechamento. |
| Clippy com `-D warnings` | Passou. |
| Regra de imports Windows | Passou. |
| Check das sete crates portáveis para Linux | Passou com `--target x86_64-unknown-linux-gnu`. |
| Recuperação da barra vazia | Verificada por sonda de dois processos e na barra real em pílulas. |
| Campo nativo, opções e persistência | Verificados na tela, com configuração isolada. |
| Galeria, arraste centro → fim e cancelamento | Verificados na tela e no arquivo de teste. |
| Alteração externa e reaplicação | Verificadas na tela; tema externo e comentário de chave futura preservados. |
| 50 ciclos de edição em release | 8 → 8 janelas do Zefiro. O Windows criou sua janela `MSCTFIME UI` ao primeiro foco; ela é discriminada no resultado da sonda. |
| Memória privada após 50 ciclos e repouso | **14,87 MB**, mediana de 13 amostras de 1 s, após 5 s de espera. |
| Working set no mesmo teste | **41,54 MB**; acompanhado separadamente, sem redefinir a meta de memória privada da fase. |
| CPU em repouso | **0,0%**. |
| Processamento de movimento | p95 **2,63 ms**, em um arraste real; amostra curta, não um benchmark abrangente de todos os monitores/DPI. |
| Abertura do editor, 50 ciclos | Mediana **4,74 ms**, p95 **5,37 ms**, máximo **5,50 ms**. |
| Primeiro quadro, duas aberturas release | **36 e 38 ms**, dois monitores. |

A transição menu → editor mantém o foco no processo; devolver temporariamente o foco ao app anterior custava aproximadamente 9 ms desnecessários. A métrica de abertura mede comando → conteúdo entregue ao compositor; não equivale a uma medição de apresentação física.

A galeria das oito combinações encontrou uma regressão da criação adiada: grupos novos ficavam acima dos chips já existentes ao trocar faixa por pílulas. A ordem foi corrigida e ganhou um teste de composição; a correção foi conferida na tela. Também foram verificados movimento e cancelamento por teclado, e preservação de rascunho após três `WM_DISPLAYCHANGE` agrupados numa recriação. Isso não equivale a trocar fisicamente um monitor.

Artefatos locais ficam em `.superpowers/sdd/2026-10-15-fase-1b-modo-de-edicao/`: JSONs de desempenho, ciclos e capturas. São ignorados pelo Git. Sondas reproduzíveis ficam em `scripts/probes/` e nos exemplos de `platform-windows`.

## Consequências e validações pendentes

- A 1c pode substituir a reserva conservadora por uma ilha integrada ao layout e ao compositor Rust.
- O limite de 15 MB de memória privada foi mantido; o consumo transitório do editor e os caches em liberação são maiores que o repouso e não foram ocultados na medição.
- O PresentMon foi encontrado, mas recusou iniciar a sessão ETW por falta de privilégio administrativo/participação no grupo Performance Log Users. **Quadros perdidos continuam sem medição; não há declaração de zero perdas.**
- Ainda precisam de validação real: reconectar telas/Win+P, mudança de DPI, reinício do Explorer, bloqueio e preferências de animação/transparência. Os caminhos implementados e testes portáveis não substituem esses testes de ambiente.
- Limitações de acabamento adiadas: o campo nativo usa a fonte padrão de UI do Windows; a representação arrastada fica recortada aos limites da janela da barra ao sair verticalmente. Isso não altera os arrays persistidos ou a soltura inválida.
