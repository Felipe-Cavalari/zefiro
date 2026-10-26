# Barra limpa: ocultar automaticamente, só no monitor principal e Configurações

- **Data:** 2026-10-22
- **Status:** rascunho para aprovação
- **Release:** entra na próxima versão (1.3.0), junto com a correção da tela cheia nos jogos (commit `f01bd0a`)

## 1. Intenção

Um amigo do Felipe usa o Zefiro e quer um setup mais limpo: a barra fora do caminho até ele precisar dela,
e tudo concentrado no monitor principal. O Felipe não usa assim, mas quer as duas opções para quem gosta.
Junto com isso, o menu do Z fica mais curto: as ações que se usam raramente vão para uma janela de
Configurações.

Sucesso é o amigo ligar as duas opções, a tela ficar limpa no dia a dia, e a barra aparecer só quando ele
leva o mouse ao topo, sem nunca aparecer por cima de um jogo.

## 2. Objetivos

1. **Ocultar automaticamente:** a barra só aparece quando o mouse vai ao topo da tela.
2. **Só no monitor principal:** os outros monitores ficam sem barra.
3. **Configurações:** uma janela própria com as ações raras do menu do Z.
4. **Menu do Z mais curto**, com o Acrílico dentro de Tema e sem a chave do mosaico.

## 3. Fora do escopo

- Janelas em mosaico: a chave sai do menu nesta versão (ver §6). O código e o `[wm]` continuam.
- Ocultar automaticamente em outra borda (a barra é sempre de cima).
- Escolher qual monitor recebe a barra além do principal do Windows.
- Novas configurações além das listadas aqui.

## 4. O que seria fracasso mesmo funcionando

- A barra aparecer por cima de um jogo ou app em tela cheia porque o mouse passou no topo.
- Clicar numa aba do navegador ou no X de uma janela maximizada, colados no topo, e a barra pular por cima
  ou "comer" o clique.
- A barra escondida deixar um vão vazio no topo (faixa reservada sem barra).
- Desligar uma opção e perder o layout que estava configurado para os outros monitores.

## 5. Comportamento

### 5.1 Ocultar automaticamente (`[bar] autohide`)

- **Escondida, ela não reserva espaço:** as janelas maximizadas vão até o topo do monitor. Nada da barra
  fica visível (nem uma linha).
- **Aparecer:** quando o cursor encosta na borda de cima do monitor (a linha de pixels do topo) e fica ali
  **300 ms**, a barra daquele monitor desce por cima das janelas, animada no compositor (mola, como o
  resto do Zefiro; ADR-0007). Sair da borda antes dos 300 ms cancela. *(300 ms foi escolha do Claude.)*
- **O gatilho não pode roubar o clique:** com a barra escondida, um clique na linha do topo vai para a
  janela que está lá (aba do navegador, barra de título). O Zefiro só observa o mouse; não coloca uma
  janela clicável no topo.
- **Sumir:** quando o cursor sai da área da barra, ela sobe de novo depois de **500 ms**, se não houver
  painel aberto, menu aberto ou Editar barra ativo nesse monitor. Voltar à barra antes dos 500 ms cancela.
  Fechar o painel com o cursor fora da barra conta como "sair". *(500 ms foi escolha do Claude.)*
- **Cada monitor reage à própria borda.**
- **Tela cheia manda:** com app ou jogo em tela cheia no monitor (a regra de `reconcile_fullscreen`), a
  borda de cima não faz nada e a barra continua fora. Isso vale também se a barra estava aparecendo quando
  o jogo ganhou o foco: ela sai.
- **Avisos da ilha não espiam:** troca de música, volume, notificação e qualquer outro aviso não trazem a
  barra. Ela fica quieta até o mouse ir ao topo. *(Decisão do Felipe.)*
- **A ilha e os chips vão junto com a barra:** quando a barra desce, tudo aparece no estado atual (a ilha
  pode mostrar o aviso que estiver ativo naquele momento). Escondida, a captura de áudio da ilha e o
  visualizador param como já param com a barra fora (`visible=false`).
- **Monitor com outro monitor logo acima:** o trecho da borda de cima que encosta em outro monitor não é
  gatilho (o cursor passa direto para o de cima); só conta o trecho que é borda de verdade da área de
  trabalho. Se um monitor não tiver nenhum trecho assim, a barra dele só aparece pela bandeja. *(Escolha do
  Claude; documentar no ADR.)*
- **Ilha externa (`[island] engine = "externa"`):** a ilha C++ é outro processo e não acompanha a barra.
  Nesse modo, a chave "Ocultar automaticamente" fica apagada no Editar barra, com a dica "indisponível com a
  ilha externa", e `autohide = true` no arquivo é ignorado com aviso no log. *(Escolha do Claude: a ilha
  externa é o caminho antigo, e mexer no fork custaria mais que o resto da feature.)*
- **Atalho e bandeja continuam:** `Ctrl+Alt+Shift+D` (HUD) funciona com a barra escondida. A bandeja:
  - **Clique esquerdo:** se há barra escondida por tela cheia, traz de volta (a "saída de emergência" de
    hoje, gesto explícito, vale até por cima de um jogo). Se não, abre o menu do Z no principal; com a
    barra escondida pela opção, ela desce e fica enquanto o menu estiver aberto.
  - **"Mostrar a barra"** no menu da bandeja: traz as barras escondidas por tela cheia (como hoje) e faz a
    barra do principal descer como se o mouse tivesse ido ao topo; ela sobe pela regra normal quando o
    cursor não estiver nela. Não desliga a opção.
  - O menu da bandeja continua com os itens de hoje (inclusive Reiniciar a ilha e Iniciar com o
    Windows): com a barra escondida, ele é o caminho que sobra. *(Escolha do Claude.)*
- Ligar ou desligar vale na hora em que o Editar barra é concluído. Desligar devolve a reserva da AppBar
  (as janelas maximizadas descem para baixo da barra, como hoje).

### 5.2 Só no monitor principal (`[bar] display`)

- `display = "todos"` (padrão, o comportamento de hoje) ou `display = "principal"`. O nome não é
  `monitors` porque `[bar.monitors.*]` já são os layouts próprios por monitor; as duas coisas não cabem
  no mesmo TOML. *(Escolha do Claude.)*
- Com `principal`, só o monitor principal do Windows tem barra, painel e reserva de espaço. Os outros
  ficam como se o Zefiro não existisse neles.
- **Nada é apagado:** os layouts próprios dos outros monitores (`[bar.monitors.*]`) continuam no
  `config.toml` e voltam quando a opção é desligada.
- Se o principal mudar (Win+P, trocar o principal nas configurações do Windows, desligar o monitor), a
  barra vai para o novo principal pelo mesmo caminho da recriação de barras que já existe
  (`DisplayChanged` → `TIMER_DISPLAY`).
- Os chips `player`, `weather` e `hardware` configurados só num monitor secundário deixam de aparecer
  enquanto a opção estiver ligada (estão num layout que não é desenhado).
- O mosaico (quando voltar) não depende disso; fora do escopo.

### 5.3 As duas opções no Editar barra

- Ficam no Editar barra, porque ele já é "o que aparece e onde". Valem ao **Concluir**, como o resto do
  rascunho (ADR-0008), e o Cancelar as desfaz.
- Aparecem numa linha própria acima do Cancelar/Concluir, visível nas duas páginas (Itens e Opções do
  relógio), com duas chaves no mesmo estilo das do menu do Z:
  - "Ocultar automaticamente"
  - "Só no monitor principal" — só aparece com mais de um monitor.
- Com "Só no monitor principal" ligado no rascunho, os botões de monitor e o "Mesmo layout em todos /
  Layout próprio" ficam apagados (sem efeito), e a prévia continua no principal. Ao concluir, as barras
  dos outros monitores saem.
- Enquanto o Editar barra estiver aberto, a barra em edição fica visível mesmo com "Ocultar
  automaticamente" ligado; ao concluir, ela segue a regra de §5.1 a partir da posição do cursor.

### 5.4 Configurações

- Item **Configurações** no menu do Z abre um painel próprio, do mesmo tipo e no mesmo lugar do
  Editar barra (o painel que nasce da barra do monitor onde o menu estava).
- Diferente do Editar barra, **não há rascunho**: cada item vale na hora, e não há Cancelar/Concluir, só
  fechar (Esc, clicar fora, ou um botão "Fechar"). *(Escolha do Claude: são decisões isoladas, e o
  "Iniciar com o Windows" já funciona assim.)*
- Conteúdo, nesta ordem:
  1. **Iniciar com o Windows** (chave)
  2. **HUD de debug** (chave; acende com o HUD aberto)
  3. **Reiniciar a ilha** (ação)
  4. **Exportar diagnóstico** (ação)
  5. Rodapé com a versão instalada ("Zefiro 1.3.0").
- Navegação por teclado como no menu do Z (setas, Enter, Esc).
- O painel fecha sozinho se o monitor entrar em tela cheia, como os outros painéis.

### 5.5 Menu do Z

Fica, nesta ordem:

1. Altura (Completa / Compacta)
2. Formato (Faixa / Pílulas / Ilhas / Fluent / Vidro)
3. Tema ›
4. "Atualizar para a x.y.z" (só quando houver versão nova, como hoje)
5. Editar barra
6. Configurações
7. Sair

- **Acrílico** vai para o topo da tela de temas, logo abaixo de "‹ Tema", antes da grade. Continua
  sumindo quando os "Efeitos de transparência" do Windows estão desligados.
- **Janelas em mosaico** sai do menu nesta versão. *(Escolha do Claude: só a chave sai; quem tiver
  `[wm] enabled = true` no arquivo continua com o mosaico, para o desenvolvimento seguir.)*
- Iniciar com o Windows, Reiniciar a ilha, HUD de debug e Exportar diagnóstico saem do menu (vão para
  Configurações).

## 6. Configuração

```toml
[bar]
autohide = false        # true: a barra só aparece com o mouse no topo (Editar barra)
display = "todos"       # "todos" | "principal" (Editar barra)
```

- Sem as chaves, vale o comportamento de hoje.
- Valor desconhecido em `display` cai em `"todos"` com aviso no log, como os outros campos.
- A gravação segue o `ConfigPatch` do editor (ADR-0008): só os campos alterados, preservando comentários.

## 7. Restrições

- **Regra de ouro da crate `windows`** (AGENTS.md): a decisão (quando aparecer, quando sumir, que
  monitores têm barra) é dado portável e testável; a `platform-windows` só observa o mouse, move e anima.
- **Animações no compositor**, nunca redesenhando por `WM_TIMER`. A descida e a subida são molas;
  "mover na hora" usa `Comp::move_offset` (ADR-0010).
- **Nada consulta por quadro.** Observar o mouse não pode custar CPU em repouso; com a opção desligada,
  nenhum custo novo.
- **Metas de performance** do `docs/PLANNING.md` continuam (RAM, CPU em repouso, primeiro quadro).
- O hook de mouse de baixo nível que já existe (`bar_context`, só botão direito) pode ser reaproveitado ou
  não; qualquer hook novo tem que repassar todos os eventos sem atraso perceptível.
- Nenhuma janela do Zefiro do tamanho exato do monitor (o Windows liga o Não perturbe; ADR-0007).
- TDD como no fluxo do AGENTS.md: o que é comportamento de tela usa sonda que falha antes e passa depois.

## 8. Deixado para quem implementa

- Como a reserva some com a barra escondida: tirar a AppBar, reservar 0 px ou `ABM_SETAUTOHIDEBAREX`.
  O critério é §5.1 (sem vão, maximizadas até o topo, sem briga com a barra de tarefas do Windows).
- Como detectar o cursor na borda sem roubar o clique (hook de mouse, `GetCursorPos` com temporizador só
  enquanto a opção está ligada, ou outro meio), desde que respeite §7.
- O desenho exato das chaves no Editar barra e do painel de Configurações, seguindo o visual do menu do Z.
- A divisão em tarefas e a ordem, com a sugestão de entregar primeiro as duas opções do amigo.

## 9. Ao terminar

- ADR novo para ocultar automaticamente e só no principal (como a reserva some, como o cursor é
  observado), e atualização do AGENTS.md (config, menu, coisas não óbvias) e do `docs/PLANNING.md`.
- Release 1.3.0 só quando o Felipe pedir.
