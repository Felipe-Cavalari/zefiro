# Zefiro — Fase 1a: Barra com a linguagem visual da ilha (spec)

> **Data:** 2026-10-15 · **Autor:** Felipe Cavalari
> **Status:** rascunho para revisão
> **Base:** [`docs/PLANNING.md`](../../PLANNING.md) (§2 Fase 1, §3.5 animação, §4 tema), [ADR-0001](../../adr/0001-pipeline-de-renderizacao.md) (compositor, WARP, molas), [ADR-0006](../../adr/0006-ilha-de-midia-fork.md) (ilha como processo separado) e [notas de porte da ilha](../../island-port-notes.md)

---

## 1. Objetivo

Construir **do zero** a barra do Zefiro com a **mesma linguagem visual da ilha** (Dynamic Island for Windows): os temas, o material, o raio das pílulas e a fluidez das molas. Também: **painéis que abrem e fecham como a ilha** e a **ilha embutida no centro**.

Ao final da Fase 1a, o `zefiro.exe` substitui a barra do YASB para o básico do dia a dia: barra nos dois monitores, relógio com calendário e a ilha no centro, com a cara e a sensação da ilha. Os outros widgets (áudio, brilho etc.) entram nas etapas seguintes **reaproveitando o sistema de pílula → painel** definido aqui.

## 2. Decisões do usuário

| Tema | Decisão |
|---|---|
| Ponto de partida | **Do zero.** A barra experimental do Codex (`codex/zefiro-bar`) é ignorada. Os spikes S1 e S6 servem de referência; nada é copiado deles sem revisão. |
| Alturas | Duas: **completa = 44 DIP** (a altura da pílula da ilha) e **compacta = 32 DIP**. |
| Formatos | Dois: **faixa** (ponta a ponta, contínua) e **pílulas** (três grupos flutuantes: início, centro, fim). |
| Tema | **Obsidian** é o padrão. Vêm junto os outros 7 temas da ilha (Graphite, Slate, Nord, Evergreen, Espresso, Plum, Porcelain). Todos são arquivos editáveis pelo usuário. |
| Painéis dos widgets | Mesma função dos painéis do YASB (lista de saídas de som, sliders de brilho…), mas abrindo e fechando **como a ilha**: a pílula se transforma no painel com molas, sem engasgo, na taxa do monitor (≥ 60 fps). |
| komorebi | **Fora até a versão 1.0.** |
| Ilha no modo compacto | **A pílula recolhida encolhe junto com a barra;** o painel expandido da ilha continua do tamanho normal. _(Recomendação aceita por padrão, porque o usuário não respondeu a essa pergunta. Trocar aqui, se quiser outra.)_ |

## 3. Escopo

**Dentro:**
1. Workspace Rust novo e a fundação mínima para a barra (§9).
2. Barra AppBar por monitor, nas 2 alturas × 2 formatos, trocáveis ao vivo.
3. Temas: os 8 temas da ilha como arquivos TOML, tokens derivados iguais aos da ilha, temas do usuário, recarga ao salvar.
4. **Sistema pílula → painel**, genérico, usado por todos os widgets.
5. Widgets: **Zefiro** (pílula do início, com o menu da barra) e **Relógio + calendário**.
6. **Ilha embutida**: a barra inicia, posiciona, sincroniza o tema e encerra o `zefiro-island.exe`.
7. Medições (RAM, CPU, primeiro quadro, quadros perdidos) e o **ADR-0007**.

**Fora** (etapas seguintes da Fase 1):
- Modo de edição, galeria, layout por monitor (1b).
- Porte da ilha para Rust (1c, ver §7.6).
- Áudio, brilho (1d); itens declarativos, Claude, Codex (1e); VS Code, Pomodoro, foco, noturno, notificações (1f).
- komorebi (até a 1.0).
- App de configurações (Fase 2). Na 1a, configura-se pelo menu do Zefiro e pelos arquivos.

---

## 4. Linguagem visual (tirada da ilha)

Os números vêm do código da ilha (ver [notas de porte](../../island-port-notes.md) §2, §6, §10). Tudo em DIP, multiplicado pela escala do monitor.

### 4.1 Temas

Os 8 temas da ilha, com os mesmos valores:

| Tema | Fundo | Texto | Secundário | Contorno |
|---|---|---|---|---|
| **Obsidian** (padrão) | `#08080A` | `#FFFFFF` | `#9B9BA5` | `#1E1E22` |
| Graphite | `#1C1C1E` | `#FFFFFF` | `#A8A8AE` | `#323236` |
| Slate | `#111721` | `#E9EEF6` | `#92A2B8` | `#232C3A` |
| Nord | `#2E3440` | `#ECEFF4` | `#A7B0C0` | `#3B4252` |
| Evergreen | `#0C1512` | `#E4F1EA` | `#8CAE9D` | `#1A2A23` |
| Espresso | `#1A1512` | `#F6EEE7` | `#B7A395` | `#2E2420` |
| Plum | `#16111C` | `#F1EAF6` | `#AC9BB9` | `#281F33` |
| Porcelain (claro) | `#F5F6F8` | `#14161A` | `#5B6270` | `#D8DBE1` |

### 4.2 Tokens derivados

Calculados a partir do tema, com as mesmas regras da ilha (`BuildMaterialTokens`):

| Token | Regra |
|---|---|
| `escuro` | luminância relativa do fundo < 0,45 |
| `elevado` (cartões, linha sob o mouse) | branco com alfa 0,085 (tema claro: preto, 0,062) |
| `elevado_forte` (pressionado, ativo) | 0,150 (claro: 0,105) |
| `divisoria` | 0,100 (claro: 0,085) |
| `sombra` | preto, alfa 0,55 |
| `texto` / `texto_secundario` | cores do tema com alfa 0,98 / 0,88 |
| `texto_terciario` | secundário misturado 35 % ao fundo, alfa 0,72 |
| `destaque` | `auto` (cor da capa, vinda da ilha), `sistema` (cor de destaque do Windows) ou hex; sempre corrigido para ter contraste com o fundo |

### 4.3 Material

- **Preenchimento:** cor do fundo com a opacidade do tema (padrão 0,96, como o `PillOpacity` da ilha).
- **Sombreamento de profundidade:** gradiente vertical suave para baixo dentro da forma. Desligável.
- **Sombra:** sombra suave abaixo de cada pílula e painel (`DropShadow` do compositor, cor `sombra`). Desligável.
- **Contorno:** 1 DIP na cor `contorno`. Pode ficar sem contorno (`borderless`).
- **Sem brilho de aro nem bordas claras**, como na ilha.
- **Fundo do Windows (opcional):** `backdrop = "acrylic"` usa o `HostBackdropBrush` recortado pela forma (ADR-0001), com `backdrop_fill` controlando quanto do preenchimento fica por cima. O padrão é `"none"`, como na ilha.

### 4.4 Forma

- **Pílula:** raio = metade da altura, limitado a `radius_max` (padrão 44 DIP, o da ilha).
- **Painel:** raio `panel_radius` (padrão 24 DIP).
- **Estilo alternativo `w11`:** raio fixo de 8 DIP em tudo (o "Windows 11" da ilha).
- Durante a transformação pílula → painel, o raio é animado junto (§6).

### 4.5 Tipografia

`Segoe UI Variable` (Text para corpo, Display para o relógio do painel), com escala de texto do tema (padrão 1,0). Tamanhos-base: 13 DIP na barra completa, 12 na compacta, 14 no corpo dos painéis.

### 4.6 Movimento

Molas da ilha, rodando **no compositor** (`SpringScalarNaturalMotionAnimation` / `SpringVector2NaturalMotionAnimation`; ω = √k, ζ = c / (2√k)):

| Movimento | k | c |
|---|---|---|
| Crescer (abrir) | 380 | 26 |
| Encolher (fechar) | 200 | 28 |
| Ajuste parado (troca de tamanho de conteúdo) | 280 | 24 |
| "Cutucada" ao abrir | 280 | 24, começando em −6 DIP no eixo vertical |

Estilos do tema multiplicam k × c: suave 1,0 × 1,35 · **padrão** 1,0 × 1,0 · elástico 1,1 × 0,70 · ágil 1,5 × 1,25. A velocidade (0,5×–2,0×) multiplica k por velocidade² e c por velocidade, o que equivale a acelerar o tempo da animação na mesma proporção (é o que a ilha faz ao multiplicar o `dt`).

Se os "Efeitos de animação" do Windows estiverem desligados, as transições são instantâneas (PLANNING §3.5).

---

## 5. A barra

### 5.1 Geometria

Uma janela de barra por monitor, registrada como **AppBar no topo**, reservando a **altura da faixa H** (completa 44, compacta 32). Todos os widgets da 1a ficam numa pílula de altura **P**:

| | Faixa | Pílulas |
|---|---|---|
| **Completa** (H = 44) | faixa de 44; widgets com P = 32 (margem 6); a ilha com 44 | faixa transparente de 44; pílulas com P = 36 (margem 4 em cima e embaixo, 8 nas pontas da tela) |
| **Compacta** (H = 32) | faixa de 32; widgets com P = 24 (margem 4); a ilha com 28 | faixa transparente de 32; pílulas com P = 24 (margem 4) |

- **Faixa:** um retângulo de ponta a ponta com o material do tema, sem cantos arredondados e com uma divisória de 1 DIP na borda de baixo. Os widgets são "chips" sem fundo, que ganham fundo `elevado` com o mouse em cima e `elevado_forte` com o painel aberto.
- **Pílulas:** a faixa é transparente (o desktop aparece) e os cliques nela passam para baixo. Cada **seção** (início, fim) é **uma pílula** com o material completo (preenchimento, profundidade, sombra, contorno), e os widgets dentro dela são chips. O centro **é a própria ilha**.
- Espaço entre widgets numa seção: 4 DIP; *padding* interno do chip: 10 DIP nas laterais.
- **Ilha:** sempre no centro do monitor principal (§7). Nos outros monitores, o centro fica vazio.
- **Mesmo layout em todos os monitores** na 1a (layout por monitor é da 1b).

### 5.2 Widgets da 1a

| Widget | Na barra | Painel |
|---|---|---|
| **Zefiro** (seção início) | Monograma "Z" num quadrado arredondado + "Zefiro" (só o "Z" na compacta) | Menu da barra: **Altura** (completa/compacta), **Formato** (faixa/pílulas), **Tema** (os 8 + os do usuário), **Recarregar tema**, **Abrir pasta de temas**, **Reiniciar a ilha**, **Sair**. As opções marcadas mostram o estado atual, e a troca vale na hora. |
| **Relógio** (seção fim) | Data e hora no formato do Windows (pt-BR: `qui 15 15:47`), atualizado na virada do minuto (sem segundos por padrão) | **Calendário** do mês, no estilo do calendário da ilha: dia de hoje com o destaque; primeiro dia da semana pelo Windows; ←/→ ou a roda do mouse trocam de mês; um título com o dia completo ("quinta-feira, 15 de outubro"). |

O botão direito em qualquer parte da barra também abre o painel do Zefiro.

### 5.3 Ao vivo

Trocar a altura, o formato ou o tema não fecha nada: a barra **anima** para o novo estado com a mola de ajuste (280/24). Isso inclui a altura da faixa, as pílulas aparecendo ou sumindo e a ilha mudando de tamanho. A reserva da AppBar muda no fim da animação.

---

## 6. Sistema pílula → painel

O mesmo componente serve para todos os painéis, da 1a e das próximas etapas.

### 6.1 Comportamento

1. **Abrir:** clicar num chip. O painel nasce **do retângulo do chip** e cresce com a mola de crescer (380/26) até o tamanho final, logo abaixo da faixa (6 DIP de distância), alinhado ao chip e preso à borda da tela com 8 DIP de folga. Largura, altura, posição e raio animam juntos (raio do chip → `panel_radius`), mais a "cutucada" vertical.
2. **Conteúdo:** o conteúdo do chip some em 0,17 s e o do painel aparece quando a forma passa de 60 % da altura final. É a mesma regra da ilha (`alfa = clamp((h − 0,6·H) / (0,4·H))`).
3. **Fechar:** clicar fora, clicar no chip de novo, **Esc**, ou o painel perder o foco (Alt+Tab, Win, outra janela ativada). O painel volta para o chip com a mola de encolher (200/28) e desaparece.
4. **Um painel por vez.** Abrir outro fecha o atual (o novo começa sem esperar o antigo terminar).
5. **Teclado:** o painel recebe o foco ao abrir. As setas navegam nas listas, Enter ativa, e ao fechar o foco volta para a janela de antes (como as setas da ilha).
6. **Conteúdo que muda de tamanho** (ex.: o calendário de um mês com 6 semanas) anima com a mola de ajuste.

### 6.2 Como é desenhado (para garantir a fluidez)

- Cada monitor tem **uma janela de painel**, reaproveitada, sem bitmap de redirecionamento (`WS_EX_NOREDIRECTIONBITMAP`) e com o conteúdo inteiro no **Windows.UI.Composition** (ADR-0001).
- A forma é um `ContainerVisual` com `CompositionGeometricClip` (retângulo arredondado), cujo tamanho, posição e raio são **animações de mola do compositor**. Nada é redesenhado por quadro, então a CPU do Zefiro fica parada durante a animação.
- O conteúdo do painel é desenhado **uma vez** com Direct2D (dispositivo WARP) numa superfície do tamanho final e revelado pelo recorte. Só é redesenhado quando um dado muda.
- A janela tem o tamanho do retângulo final + a sombra. Fora da forma atual, os cliques passam para baixo (região de clique atualizada no fim de cada animação).
- **Meta:** clique → primeiro quadro ≤ 16 ms; **0 quadros perdidos** na taxa do monitor (165 Hz no principal), medidos com o contador interno (`FrameWatch`, ADR-0001) e o `scripts/frames.ps1`.

---

## 7. A ilha embutida

### 7.1 Modelo

Na 1a, a ilha continua sendo o **`zefiro-island.exe`** (fork do Dynamic Island, ADR-0006), num processo separado. A barra é dona do ciclo de vida dele.

### 7.2 Ciclo de vida

- A barra inicia o `zefiro-island.exe` depois de mostrar a primeira barra, para não atrasar o primeiro quadro. O executável é procurado ao lado do `zefiro.exe` ou no caminho de `[island] exe` da configuração.
- Ao sair, a barra chama `zefiro-island.exe --stop` e espera até 3 s.
- Se a ilha morrer, a barra a reinicia: até 3 vezes em 60 s. Depois disso, registra no log e deixa o centro vazio até alguém usar **Reiniciar a ilha** no menu.
- `[island] enabled = false` desliga a ilha.

### 7.3 Configuração enviada à ilha

Antes de iniciar, e a cada mudança, a barra grava no `%APPDATA%\zefiro\island.ini` (a ilha relê sozinha):

| Chave | Valor |
|---|---|
| `[Zefiro] Anchor` | `bar` |
| `[Zefiro] BarHeight` | H |
| `[Zefiro] PillHeight` (nova) | altura da pílula recolhida da ilha (§5.1: 44 / 28 na faixa, 36 / 24 nas pílulas) |
| `Themes.ThemePreset` | o tema atual, se for um dos 8; para um tema do usuário, `custom` + `PillBgColor`, `TextPrimaryColor`, `TextSecondaryColor`, `ContourBorderHex` |
| `Animations.AnimationStyle` / `AnimationSpeed` | os do tema |

As outras chaves do `island.ini` não são tocadas: as preferências que o usuário mudou na ilha continuam.

### 7.4 Mudança necessária na ilha: pílula recolhida menor

A ilha só tem uma escala (`SizeScale`) para tudo. Para a pílula recolhida acompanhar a barra sem encolher o painel expandido, entra um novo bloco `// ZEFIRO:` no fork:
- `PillHeight` define a **escala recolhida** `s_r = PillHeight / 44` (a pílula de mídia tem 44 px de conteúdo; a ociosa, 36 px, usa `PillHeight / 36`).
- O tamanho-alvo das pílulas recolhidas usa `s_r`; o painel expandido continua com `SizeScale`.
- O conteúdo é desenhado com uma escala interpolada entre `s_r` e `SizeScale` pela fração de expansão (a mesma `t` do `ResolveOffsetY`). Assim, a transformação continua contínua.

**Plano B,** se a mudança ficar invasiva demais (mais de ~150 linhas no arquivo do mod): no modo compacto, a pílula recolhida fica com 44 px e passa da faixa, como hoje, até o porte da 1c.

### 7.5 Ordem das janelas

Barra e ilha ficam sempre no topo. Depois de qualquer mudança de ordem das janelas da barra (criação, recriação por troca de monitor, reinício do Explorer), a barra põe a janela da ilha (classe `Windhawk.DynamicIslandForWindows`) **acima** de si com `SetWindowPos(HWND_TOPMOST)`.

### 7.6 Depois (1c)

A 1c deixa de "construir a ilha" e passa a **portar a ilha para Rust dentro da barra**, usando o sistema pílula → painel desta spec e as notas de porte. Isso acaba com o segundo processo e com o redesenho a 60 Hz.

---

## 8. Temas e configuração

### 8.1 Arquivos de tema

- Os embutidos ficam em `resources/themes/<id>.toml` (dentro do executável).
- Os do usuário ficam em `%APPDATA%\zefiro\themes\<id>.toml`. Um tema do usuário com o mesmo id de um embutido o substitui. **Abrir pasta de temas** cria a pasta e copia o tema atual para lá, pronto para editar.
- Salvar o arquivo aplica o tema na hora (observador de arquivo, com espera de 150 ms).
- Erro no arquivo: a barra mantém o tema anterior e registra no log a linha e o motivo.

```toml
# resources/themes/obsidian.toml
name = "Obsidian"

[colors]
background     = "#08080A"
text           = "#FFFFFF"
text_secondary = "#9B9BA5"
border         = "#1E1E22"   # "none" = sem contorno
accent         = "auto"      # auto (capa da ilha) | system | "#RRGGBB"

[material]
opacity        = 0.96
depth          = true
shadow         = true
backdrop       = "none"      # none | acrylic
backdrop_fill  = 0.45

[shape]
style          = "pill"      # pill | w11
radius_max     = 44
panel_radius   = 24

[font]
family         = "Segoe UI Variable"
scale          = 1.0

[animation]
spring         = "default"   # smooth | default | bouncy | snappy
speed          = 1.0
```

### 8.2 Configuração da barra

`%APPDATA%\zefiro\config.toml`, editado pelo menu sem perder comentários (`toml_edit`) e recarregado ao salvar:

```toml
theme = "obsidian"

[bar]
height = "completa"   # completa | compacta
format = "faixa"      # faixa | pilulas

[bar.sections]
start = ["zefiro"]
end   = ["clock"]
# o centro é a ilha (monitor principal)

[island]
enabled = true
exe     = ""          # vazio = ao lado do zefiro.exe

[clock]
format  = ""          # vazio = formato do Windows
seconds = false
```

---

## 9. Arquitetura

Workspace Rust novo na raiz do repositório, seguindo o PLANNING §3.1–3.2. Só entra o que a 1a usa.

| Crate | Responsabilidade na 1a | Windows? |
|---|---|---|
| `zefiro-core` | Configuração (ler, observar, gravar com `toml_edit`), barramento de eventos, temporizadores | não |
| `zefiro-resources` | Temas: leitura, validação, os 8 embutidos, tokens derivados (§4.2), correção de contraste | não |
| `zefiro-ui` | Modelo de mola (k/c → ω/ζ, estilos, velocidade), **layout da barra** (seções → retângulos para as 4 combinações), layout dos painéis, hit-test | não |
| `zefiro-platform` | Traits: monitores, janela da barra, janela de painel, superfície de desenho, processo filho | não |
| `zefiro-platform-windows` | AppBar por monitor, DPI por monitor, `DispatcherQueue` (uma por thread), Windows.UI.Composition, dispositivo D2D **WARP** único, DirectWrite, `HostBackdropBrush`, sombras, janela de painel, foco, processo filho | sim |
| `zefiro-bar` (módulo) | Barra, chips, menu do Zefiro, sistema pílula → painel | não |
| `zefiro-island-host` (módulo) | Inicia, vigia e encerra o `zefiro-island.exe`; grava o `island.ini`; ordem das janelas (via trait) | não |
| `zefiro-integration-clock` | Hora, formatação pelo Windows (pt-BR), dados do calendário | só em `windows.rs` |
| `zefiro-app` | Binário `zefiro.exe`: instância única, logs em `%LOCALAPPDATA%\zefiro\logs\zefiro-AAAAMMDD.log`, relatório de crash | sim |

- **Fundação:** as tarefas F2 (workspace, logs, crash) e a parte de molas do F3 do plano da Fase 0 entram aqui, reescritas para este escopo. O supervisor de integrações e o `Secret<T>` ficam para a primeira integração que pode falhar ou que tem credenciais (1d/1e).
- **Threads:** uma thread de UI para todas as janelas (com a `DispatcherQueue`), uma para os observadores de arquivo e uma para vigiar a ilha. Mensagens para a UI chegam pela fila.
- **Regra de ouro:** a crate `windows` só em `zefiro-platform-windows`, nos `windows.rs` das integrações e no `zefiro-app`. Verificado no CI.
- **Desenho:** conteúdo desenhado só quando muda. Animação só no compositor. Nenhum laço de quadros na barra.

## 10. Erros e degradação

- **Explorer reiniciou** (`TaskbarCreated`): AppBars registradas de novo, ilha reposta acima.
- **Monitor ligado, desligado ou com resolução trocada:** as barras são recriadas com *debounce* (vários `WM_DISPLAYCHANGE` por evento, ADR-0001).
- **DPI mudou** (`WM_DPICHANGED`): tudo é recalculado e redesenhado no novo DPI.
- **Animações do Windows desligadas:** sem molas (instantâneo).
- **Transparência do Windows desligada:** `backdrop` vira `none`.
- **Tema ou configuração inválidos:** valores anteriores mantidos e erro no log.
- **Ilha ausente ou morrendo:** §7.2. A barra nunca espera a ilha.
- **Pânico numa thread:** relatório de crash no log, e a barra tenta continuar. Pânico na thread de UI encerra o processo com o relatório.

## 11. Testes e medição

**Testes automáticos** (rodam em qualquer sistema, sem Windows onde a crate não usa Windows):
- Temas: os 8 embutidos carregam e geram exatamente os tokens da ilha (tabela §4.2); um tema inválido gera erro com a linha.
- Layout: retângulos das seções e chips para as 4 combinações em 100 %, 125 % e 150 % de DPI; centro livre para a ilha.
- Molas: conversão k/c → ω/ζ, estilos e velocidade.
- Configuração: ler, gravar sem perder comentários, recarregar.
- Relógio: formatação pt-BR, virada de minuto, grade do mês (início no domingo e na segunda, meses de 4, 5 e 6 semanas).
- Ilha: o `island.ini` é gravado só com as chaves da §7.3; a política de reinício (3 em 60 s).

**No Windows** (manual + scripts):
- Capturas das 4 combinações com cada tema, para validação visual com o usuário.
- `scripts/perf.ps1 -Process zefiro` (60 s + 180 s), em repouso e abrindo e fechando painéis, com o YASB fechado.
- Primeiro quadro (log interno, 5 aberturas).
- `FrameWatch` + `scripts/frames.ps1` em 50 ciclos de abrir e fechar o calendário.
- Win+P, reinício do Explorer, troca de tema e de altura no meio de uma animação.

**Metas da 1a** (processo `zefiro`, sem contar a ilha, que tem as metas do ADR-0006):

| Métrica | Meta |
|---|---|
| Memória privada em repouso | ≤ 15 MB (o S1 com WARP deu 10,7 MB) |
| CPU em repouso | ~0 % (≤ 0,05 %) |
| CPU abrindo e fechando painéis | só no clique (nenhum custo por quadro) |
| Primeiro quadro | ≤ 250 ms |
| Clique → primeiro quadro do painel | ≤ 16 ms |
| Quadros perdidos abrindo e fechando | 0 na taxa do monitor |

## 12. Pronto quando

1. `zefiro.exe` abre as barras nos dois monitores e reserva o espaço.
2. As 4 combinações (completa/compacta × faixa/pílulas) funcionam e são trocadas ao vivo pelo menu do Zefiro, animando.
3. Os 8 temas trocam ao vivo; um tema do usuário editado e salvo é aplicado na hora.
4. O relógio e o calendário funcionam, e o painel abre e fecha como a ilha, sem quadros perdidos.
5. A ilha aparece no centro, combinando com a altura, o formato e o tema; é reiniciada se morrer e fechada junto com a barra.
6. As metas da §11 estão medidas e registradas no **ADR-0007**; as que não forem cumpridas estão explicadas.
7. O usuário valida visualmente as 4 combinações.

## 13. Riscos

| Risco | Plano |
|---|---|
| A transformação pílula → painel com recorte animado no compositor engasga ou mostra a "caixa" do desfoque | O S1 já animou um painel com mola no caminho B sem perder quadros; a 1a começa por esse componente e mede antes de fazer os widgets. |
| A mudança da pílula recolhida na ilha (§7.4) fica invasiva | Plano B da §7.4. |
| Barra e ilha disputam o topo da ordem das janelas | §7.5; testar com reinício do Explorer e Win+P. |
| Os chips "sem fundo" da faixa ficam pouco visíveis em temas claros | Validar com o Porcelain; se precisar, a faixa ganha `elevado` permanente nos chips em temas claros. |
| DPI 125 % não testável neste PC (ADR-0001) | Testes de layout cobrem 125 % e 150 %; validação real fica em aberto. |
| Fundação ainda inexistente atrasa a barra | A fundação desta spec é o mínimo; o resto do F3/F4 entra com as integrações que precisam dele. |

## 14. Mudanças no PLANNING

Feitas junto com a implementação:
- §2 e §12: **komorebi fora até a 1.0**; a 1c vira "porte da ilha para Rust"; a 1a é esta spec.
- §3.5: molas e transformação pílula → painel como descritas aqui.
- §4.1: tema padrão **Obsidian** com os 8 temas da ilha; formato do arquivo de tema da §8.1; `bar.height` passa a ser `completa | compacta` e entra `bar.format = faixa | pilulas`.
- §6: o komorebi sai da Fase 1.
