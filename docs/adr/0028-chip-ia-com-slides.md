# ADR-0028: Chip `ia`, o uso das IAs num chip só com slides

- **Status:** aceito
- **Data:** 2026-10-28
- **Spike/branch:** main

## Contexto

Com o Claude, o Codex e o Grok na barra (ADR-0011, ADR-0025, ADR-0027), os três chips de uso
ocupavam muito espaço. O usuário pediu um chip só que alterne entre as IAs como um slide, mostre se
cada conta é do Windows ou do WSL e deixe escolher quais contas entram no slide.

## Opções consideradas

1. **Redesenhar o chip a cada troca** com um `WM_TIMER` de quadro em quadro: rejeitado pela regra
   das animações no compositor (o carrossel das abas da ilha travava assim).
2. **Mola em `pos` com `Mod` na expressão**, como o `tab_strip.rs`: a posição cresce sem fim (um
   slide a cada 5 s) e o `f32` perde precisão depois de alguns dias.
3. **Dois quadros-chave por troca** (o slide que sai vai de 0 a −altura, o próximo de +altura a 0),
   num contêiner recortado no tamanho do chip. Escolhida: nada cresce, e o Zefiro só pede a troca.

## Decisão

- **Chip `ia` ("Uso das IAs")**, ao lado dos chips `claude`, `codex` e `grok`, que continuam no
  catálogo. Com ele na barra, os três serviços de uso rodam (`BarConfig::uses`).
- **Um slide por conta e origem** (`UsageState::ai_slides`): cada conta do Claude, do Codex e do
  Grok em cada origem dela. A mesma conta no Windows e no WSL (o Codex do usuário) vira dois slides
  com o mesmo número, para escolher qual aparece; juntos num slide só, sem marca, parecia que o WSL
  não era detectado. A chave é o agente, a origem e um código curto da conta (FNV-1a da chave dela,
  nunca o id): `codex:Ubuntu-24.04:1a2b3c4d`. Sem o código, duas contas do Codex no mesmo WSL viravam
  a mesma chave, e o painel mostrava a primeira duas vezes. Duas contas da mesma IA na mesma origem
  ganham o plano ou o e-mail no nome ("Codex · WSL · Plus"). O slide
  mostra o ícone da IA, a % da conta e "Win" ou "WSL" ao lado. Uma IA que ainda não respondeu não
  tem slide; sem nenhum, o robô com "—".
- **Largura fixa:** todos os slides têm a largura do maior ("100%" e a maior marca), e o chip não
  anda quando o slide troca.
- **Deslize no compositor** (`crates/app/src/ai_carousel.rs`): um visual por slide, desenhado uma vez
  e redesenhado só quando o número muda, num contêiner com `InsetClip` cuja posição é uma expressão
  sobre a mola do próprio chip. A troca são dois quadros-chave de 0,45 s com a curva das abas,
  divididos pela velocidade das animações do tema. O `TIMER_AI_SLIDE` pede a troca a cada
  `[ia] interval` (5 s; 2 a 60). Com o mouse em cima do chip, o painel dele aberto ou a barra
  escondida, o slide fica parado.
- **Painel:** com contas no Windows e no WSL, abas "Windows | WSL" no topo (o mesmo seletor do
  chat), e os blocos da aba escolhida; ao abrir, a primeira aba com conta. Um bloco por slide
  visível, na ordem do chip (o mesmo bloco dos painéis
  individuais, só com a origem do slide: a conta do Codex no Windows e no WSL são dois blocos, e um
  slide tirado pelo olho sai do painel também), com um botão de atualizar que consulta as três e
  um lápis. O lápis abre a lista dos slides: o olho tira do slide ou põe de volta
  (no fim) e as setas mudam a ordem. Cada clique grava `[ia] order` e `hidden` na hora
  (`config::set_ai`); mudar só o `[ia]` não fecha o painel (`Config::same_except_panels`).

## Consequências

- O `[ia] interval` só se muda no `config.toml`, sem tela.
- A ordem gravada é a dos slides visíveis; um slide oculto que volta vai para o fim.
- As chaves dependem do nome da distro: renomear a distro faz o slide voltar ao padrão (visível, no
  fim).
- A marca "Win/WSL" não diz qual distro; o nome completo está na lista do lápis e nos blocos do
  painel.
