# Fase 1g — tema Cobalto, HUD de debug e exportar diagnóstico — design

Data: 2026-10-20. Aprovado em conversa ("pode implementar então o 1g").

## Intenção

Fechar a Fase 1 para as 2 semanas de uso diário: um tema com o acrílico do Windows no lugar do
`cavalaridev.toml` (descartado pelo usuário), e as ferramentas de observabilidade da §7 do PLANNING
que ajudam a achar problemas durante o dogfooding.

## Tema Cobalto

- 9º tema embutido, `cobalto.toml`, escolhível no menu do Z.
- `backdrop = "acrylic"`: o desfoque do Windows aparece através da faixa, das pílulas e dos painéis.
  Sem "Efeitos de transparência" no Windows, cai sozinho para o fundo sólido (já é assim no motor).
- Fundo azul-marinho `#0F1A2E` com `backdrop_fill` ~0,55; texto `#E8EEF9`, secundário `#93A4C3`,
  contorno `#1E2B45`.
- **Destaque fixo `#3B82F6`** (não segue a capa do álbum).

## HUD de debug

- **`Ctrl+Alt+Shift+D`** (atalho global) mostra e esconde. Também há um item no menu do Z.
- Uma janela pequena no canto superior direito do monitor principal, logo abaixo da barra, que não
  recebe cliques (o mouse atravessa) e não rouba o foco.
- Atualiza **uma vez por segundo, só enquanto está aberto**. Fechado, não custa nada.
- Mostra:
  - **Quadros:** quadros por segundo que o DWM compôs no último segundo, a taxa do monitor e os
    quadros perdidos (`DwmGetCompositionTimingInfo`). As animações do Zefiro rodam no compositor, então
    é o número que importa; parado, o DWM compõe pouco, e isso é normal.
  - **Memória:** privada e working set do processo (`GetProcessMemoryInfo`).
  - **Integrações:** quando cada uma mandou dado novo pela última vez ("há 3 s"), para as que já
    mandaram algum.
  - **Últimos avisos:** os 4 avisos ou erros mais recentes do log (hora, módulo, mensagem curta).

## Exportar diagnóstico

- Item **"Exportar diagnóstico"** no menu do Z.
- Gera `zefiro-diagnostico-AAAAMMDD-HHMMSS.zip` em `Downloads` e abre o Explorer com ele selecionado.
- Conteúdo: os logs dos últimos 7 dias, os relatórios de crash, o `config.toml` e um `info.txt`
  (versão, Windows, monitores e DPI, tema, motor da ilha, memória, tempo ligado).
- **Removido antes de entrar no zip:** a pasta do usuário (vira `%USERPROFILE%`), o nome de usuário,
  e-mails e qualquer coisa com cara de token (`sk-…`, `Bearer …`, sequências longas de base64 ou hex).
- O zip é escrito sem compressão por um gravador mínimo próprio (sem dependência nova).

## Não objetivos

- Telemetria remota (§7: nada sai do PC).
- Monitor de vazamento (RAM no log a cada 10 min) e supervisor de integração: ficam para depois.
- Tema `cavalaridev.toml`.

## Com quem constrói

Layout exato do HUD, nomes das integrações na lista e as cores finas do Cobalto.
