# ADR-0024: Chip Spotify pela Web API

- **Status:** aceito (falta a medição de RAM e CPU e o roteiro na tela com o usuário)
- **Data:** 2026-10-27
- **Spec:** [Chip Spotify](../superpowers/specs/2026-10-27-chip-spotify-design.md)
- **Plano:** [2026-10-27-chip-spotify](../superpowers/plans/2026-10-27-chip-spotify.md)
- **Commits:**
  - `0cd69fa`: login;
  - `442e2a2`: pedidos;
  - `44982a4`: leitura;
  - `7362fa1`: sessão;
  - `3a7cc6c`: config;
  - `79cb273`: painel;
  - `c0b59a7`: plataforma;
  - `526e0c4`: serviço;
  - `f39b752`: ligação no app;
  - `3dc7485`: sonda.

## Contexto

No meio do trabalho, o usuário quer achar uma música ou abrir uma playlist e mandar tocar sem trazer
o Spotify para a frente. A referência é a extensão Spotify Player do Raycast. A ilha e o chip
`player` ficam como estão: eles vêm do SMTC e só mostram e controlam a mídia atual. O SMTC não busca,
não lista a biblioteca e não enxerga outros aparelhos. Para isso é preciso a Web API do Spotify.

A API mudou em fevereiro de 2026:
- **Modo de desenvolvimento:** o dono do app precisa de Premium, há no máximo 5 usuários por app e
  o modo ampliado é só para empresas.
- **Busca:** traz 10 resultados por página.
- **Playlists de outro dono:** não mostram as músicas.
- **Endpoints que saíram:** as buscas em lote, as faixas mais tocadas do artista e `browse/*`. As
  recomendações já tinham saído em 2024.
- **Player:** os comandos exigem Premium.

## Opções consideradas

1. **Web API com PKCE e o Client ID do próprio usuário** (escolhida).
   - **Prós:** sem segredo no binário, cada pessoa fica dentro do limite de 5 usuários do próprio app
     e é a API oficial.
   - **Contra:** um passo a mais na primeira vez (criar o app no painel do desenvolvedor do Spotify).
2. **Client ID do Zefiro, distribuído no instalador.**
   - **Contra:** com o limite de 5 usuários, não serve para distribuir. O modo ampliado não está
     aberto para pessoas físicas.
3. **Controlar o app do Spotify por fora** (SMTC, atalhos, `spotify:` URIs).
   - **Contra:** não busca, não lista a biblioteca, e os URIs trazem a janela do Spotify para a
     frente, que é o contrário do pedido.

## Números medidos

| Métrica | Sem o chip | Chip na barra, painel fechado | Painel aberto e rolado (1 min) | Meta |
|---|---|---|---|---|
| Memória privada | a medir | a medir | a medir | +1 MB fechado, +6 MB aberto |
| CPU | a medir | a medir | a medir | 0 % fechado |

A medição é com `perf.ps1`, o YASB fechado e `[island] enabled = false`, num perfil isolado. Falta
fazer com o usuário presente.

## Decisão

Fica um chip `spotify` com painel no estilo Raycast. A arquitetura:
- **Crate portável `zefiro-integration-spotify`:** login por PKCE, os pedidos, a leitura das
  respostas, a sessão e o cache de capas.
- **`zefiro-bar`:** o estado e o desenho do painel.
- **`platform-windows`:** a thread `zefiro-spotify`, o HTTP com corpo, o DPAPI e o listener do
  callback.

E as regras de funcionamento:
- **Login:** Authorization Code com PKCE e Redirect URI fixo `http://127.0.0.1:47823/callback`. O
  Spotify compara a URI exata, então a porta não muda.
- **Tokens:**
  - o de renovação fica cifrado pelo DPAPI em `%LOCALAPPDATA%\zefiro\spotify\token.bin`, junto do
    `client_id` dono dele;
  - o de acesso só existe na memória da thread;
  - nenhum dos dois vai para log, sonda ou diagnóstico;
  - quando o Spotify manda um token de renovação novo, o arquivo é regravado.
- **Rede só com o painel aberto:**
  - **fechado:** nenhuma consulta, só a thread parada;
  - **aberto:** o player é relido a cada 5 s, e a barra de progresso anda pelo tique de 1 s, como a
    linha do tempo da ilha;
  - **10 min fechado:** as capas e as listas em memória vão embora.
- **Prioridade na fila:** os comandos do usuário passam na frente das leituras, e as capas vão por
  último. Só a busca mais nova roda; ela dispara 300 ms depois da última tecla. Um 429 respeita o
  `Retry-After`, até 60 s.
- **Fora do escopo:** o ✓ de "já está nesta playlist" ficou de fora por decisão do usuário: exigiria
  ler todas as playlists e arriscaria o limite da API. O atalho global também ficou de fora, por
  enquanto.

## Consequências

- **Configuração:**
  - o primeiro uso pede o Client ID, e o painel mostra o passo a passo e o Redirect URI para copiar;
  - o `[spotify] client_id` fica no `config.toml` e não é segredo;
  - trocar o `client_id` apaga o token gravado.
- **Comportamento:**
  - **Músicas curtidas:** tocar uma faixa toca a lista carregada a partir dela. A API não tem um
    contexto simples para a coleção.
  - **Playlists de outro dono:** só tocam inteiras.
  - **"Tocadas há pouco":** mostra playlists, álbuns e artistas.
- **Riscos:** o Spotify mudar de novo as regras do modo de desenvolvimento, e respostas reais
  diferentes dos exemplos da documentação. A sonda `spotify_probe --dump` serve para trocar as
  fixtures pelas respostas reais.
- **Plano B:** se o modo de desenvolvimento fechar para pessoas físicas, o chip continua compilado e
  mostra a tela de login; a ilha e o `player` não dependem dele.
