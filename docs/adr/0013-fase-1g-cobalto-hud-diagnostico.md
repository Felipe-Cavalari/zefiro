# ADR-0013: Fase 1g — tema Cobalto, HUD de debug e exportar diagnóstico

- **Status:** aceito
- **Data:** 2026-10-20
- **Spike/branch:** main (commits d64275f … c7ebb7b)

## Contexto

A 1g fecha a Fase 1 para as 2 semanas de uso diário. O usuário trocou o `cavalaridev.toml` por um
tema com o acrílico do Windows puxando para o azul `#3B82F6`, e manteve o HUD e o diagnóstico da §7
do PLANNING. [Spec](../superpowers/specs/2026-10-20-fase-1g-design.md).

## Opções consideradas

1. **Zip:** a crate `zip` (com deflate) ou um gravador próprio, só com arquivos guardados. Os logs têm
   poucos MB; um gravador de ~80 linhas com CRC-32 evita uma dependência nova. Escolhido o próprio.
2. **Quadros do HUD:** contar quadros do Zefiro (não há laço por quadro: as animações rodam no
   compositor) ou ler os contadores do DWM (`DwmGetCompositionTimingInfo`). Escolhido o DWM.
3. **Avisos do HUD:** instrumentar cada integração ou capturar os `WARN`/`ERROR` do `tracing` numa
   camada (`RecentLayer`). Escolhida a camada: cobre tudo o que já loga, sem tocar nas integrações.

## Decisão

- **Cobalto** é o 9º tema embutido: `backdrop = "acrylic"`, fundo `#0F1A2E`, destaque fixo
  `#3B82F6` (passa pelo `ensure_contrast`, como todo destaque). A ilha C++ só tem 8 paletas
  (`ColorThemeV2` 0–7, 8 = personalizado): temas embutidos depois delas vão como personalizados.
- **HUD:** `Ctrl+Alt+Shift+D` (`RegisterHotKey` na janela de mensagens) ou o item no menu do Z. Uma
  janela que o mouse atravessa, no canto superior direito do principal, atualizada a cada 1 s **só
  enquanto aberta**. Cada `USER_*` carimba a integração que mandou dado. Os avisos mostram "há N s"
  em vez da hora.
- **Exportar diagnóstico:** item no menu do Z; `zefiro-diagnostico-AAAAMMDD-HHMMSS.zip` em Downloads,
  com os logs, os crashes, o `config.toml` e um `info.txt`, todos limpos por `sanitize` (pasta do
  usuário, nome de usuário, e-mails, tokens). O Explorer abre com o arquivo selecionado.

## Consequências

- O HUD fechado não custa nada além do carimbo de um `Instant` por aviso das integrações.
- `sanitize` corta qualquer sequência longa de letras e números (≥ 32, misturadas) como token: alguns
  ids e hashes dos logs saem como `<token>`.
- Os títulos de janela que o log grava ao detectar tela cheia vão para o zip como estão.
- O Cobalto é o primeiro tema embutido com acrílico; o desfoque precisa da "Transparência" do Windows
  ligada, e sem ela o tema fica com o fundo sólido.
