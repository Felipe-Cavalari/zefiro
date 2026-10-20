# Chips da ilha: player, clima e monitor do PC — design

Data: 2026-10-20. Aprovado em conversa ("pode implementar").

## Intenção

O usuário quer ter partes da ilha no monitor secundário, sem levar a ilha inteira (que continua fixa
e obrigatória no centro do principal). Em vez de uma segunda ilha, três **chips comuns do catálogo**,
que entram em qualquer seção (Início, Centro, Fim) de qualquer monitor, inclusive o principal.

## Os três chips

| id | Nome | Na barra | Clique |
| --- | --- | --- | --- |
| `player` | Player | Idêntico à pílula da ilha: capa redonda e visualizador; espiando, "título — artista". Mesma mola de morph. Parado (sem mídia): pílula pequena com uma nota musical apagada. | Com mídia: abre o painel de mídia da ilha (controles, linha do tempo, barra de fontes) nascendo do chip. Sem mídia: nada. |
| `weather` | Clima | `☁️ 17° Guarulhos`: emoji da condição, temperatura em destaque, cidade apagada. Sem leitura: `--°`. | Com leitura: abre a aba de clima da ilha (sensação, umidade, vento, 3 dias). Sem leitura: nada. |
| `hardware` | Monitor do PC | Só o ícone (chip, `E950`). | Abre a aba de hardware da ilha (CPU, GPU, RAM, disco, rede). |

## Decisões

- **O player não herda os avisos da ilha** (volume, brilho, notificações, Bluetooth, área de
  transferência, Caps Lock, timer de foco, pontos de privacidade). Só mídia: parado, mídia, espiando.
- **Espiada:** a troca de faixa espia em todos os players por ~3 s (o mesmo estado da ilha). O mouse
  em cima espia só aquele chip (hover por barra), sem fazer a ilha do principal espiar.
- **Largura do player segue a pílula** (parado < mídia < espiando). Quando ela muda, a barra refaz o
  layout com mola: os vizinhos deslizam, como a ilha cresce. O teto de 160 DIP dos chips não vale para
  o player. Com o painel do player aberto, o hover não espia (o painel não anda sob o cursor).
- **Estilo:** a pílula do player usa o estilo da faixa (sem contorno nem sombra), como a ilha na
  faixa; nas pílulas ela se funde ao grupo. O player não tem o fundo de hover nem o de ativo.
- **Painéis:** os três reaproveitam o painel com abas da ilha, cada um com o seu conjunto de abas
  (player = mídia; clima = clima; monitor = hardware). Com uma aba só, não há pontos nem troca. Se a
  mídia some com o painel do player aberto, ele fecha.
- **Clima com largura estável:** a temperatura reserva a largura de `-00°` (ADR-0010: o painel nasce
  do chip e não pode andar). A cidade é cortada com "…" para o chip caber em 160 DIP.
- **Serviços seguem a barra** (`BarConfig::uses`): a mídia roda com a ilha embutida **ou** com um
  `player` em algum layout; o clima, com a ilha embutida **ou** um `weather`. O hardware continua só
  amostrando com o painel aberto. A captura de áudio roda com mídia tocando e alguma ilha ou player
  visível.
- **Independentes da ilha:** os chips funcionam com `[island] enabled = false` e com
  `engine = "externa"`.

## Não objetivos

- Uma segunda ilha completa no secundário (descartada na conversa).
- Aba de letras no painel do player.
- Menu do botão direito no player (fontes, saída de som): fica só na ilha.

## O que fica com quem constrói

Medidas exatas da pílula parada, espaçamentos do chip de clima e a ordem no catálogo.
