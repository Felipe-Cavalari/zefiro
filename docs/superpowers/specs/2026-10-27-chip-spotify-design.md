# Chip Spotify: buscar, navegar e tocar sem abrir o Spotify — design

Data: 2026-10-27. Desenho "B · Busca primeiro (estilo Raycast)" escolhido com os mockups
(`.superpowers/brainstorm/…/spotify-layout.html` e `spotify-raycast.html`). O usuário aprovou a
descrição em conversa ("acho que tá certo"). Depois da revisão, ele decidiu que não haverá ✓ de
"já está nesta playlist" nem atalho global.

## Intenção

No meio do trabalho, o usuário quer achar uma música ou abrir uma playlist e mandar tocar **sem
trazer a janela do Spotify para a frente**. A referência é a extensão
[Spotify Player do Raycast](https://www.raycast.com/mattisssa/spotify-player): busca, biblioteca,
fila, aparelhos e ações sobre a música atual, tudo por teclado.

Um chip novo, `spotify`, abre um painel no estilo Raycast. **A ilha e o chip `player` ficam como
estão**: eles continuam vindo do SMTC e não usam nada deste chip.

O usuário tem Spotify **Premium**, que a API exige para tocar, pausar, pôr na fila e trocar de
aparelho.

## Objetivos

1. Buscar músicas, artistas, álbuns, playlists e podcasts (episódios entram junto dos podcasts) e
   tocar ou pôr na fila sem sair do que se está fazendo.
2. Navegar pela biblioteca: músicas curtidas, playlists, álbuns, artistas e podcasts salvos.
3. Ver a fila e os aparelhos e trocar o aparelho que toca (Spotify Connect).
4. Agir sobre a música atual:
   - tocar, pausar, próxima e anterior;
   - curtir e descurtir;
   - adicionar a uma playlist, ou tirar da playlist aberta no painel;
   - aleatório e repetir;
   - volume do Spotify;
   - ±15 s em episódios;
   - copiar o link, o "artista – título" ou o código de incorporação.
5. Custo baixo: com o painel fechado, nenhuma consulta à rede.

## Não objetivos

- Tudo o que não existe na API pública: rádio da música, DJ, letras e gerar playlist com IA. O
  usuário pediu para tirar.
- Mudar a ilha, o chip `player` ou o serviço do SMTC.
- Criar, renomear, apagar ou reordenar playlists; seguir artistas; salvar álbuns. Só curtir e
  descurtir músicas, e adicionar ou tirar de playlists.
- Ver as músicas de playlists que não são do usuário (Daily Mix, Descobertas da Semana, de outras
  pessoas). A API de 2026 devolve só o nome e a capa. Elas tocam inteiras, e o usuário aceitou.
- Tocar sem Premium.
- Distribuir um Client ID do Zefiro: cada pessoa usa o próprio (ver "Login").
- Mostrar em quais playlists a música já está (o ✓ da extensão do Raycast). O usuário recusou: seria
  preciso ler as faixas de todas as playlists, o que gasta consultas e arrisca o limite da API.
- Atalho de teclado global para abrir o painel. Por enquanto, só o clique no chip (decisão do
  usuário; pode entrar depois).

## Anti-objetivos (mesmo funcionando, seria falha)

- A janela do Spotify vir para a frente ou roubar o foco.
- Qualquer consulta de rede com o painel fechado, ou a RAM do Zefiro subir de forma visível. O
  orçamento está em "Custo".
- O token (de acesso ou de renovação) ir para log, diagnóstico, teste ou sonda.
- O painel travar enquanto a rede responde: nada de HTTP nem de decodificar capa na thread de UI.
- A ilha ou o `player` mudarem de comportamento.

## Restrições da API (conferidas em 2026-10-27)

Base: [guia de migração de fevereiro de 2026](https://developer.spotify.com/documentation/web-api/tutorials/february-2026-migration-guide).

- **Modo de desenvolvimento:** o dono do app precisa de Premium. O app aceita até 5 usuários, e o
  painel do desenvolvedor do Spotify exige Client ID próprio.
- **`GET /search`:** no máximo 10 por página (o padrão é 5). Para mais, use `offset`.
- **Playlists:** `GET /playlists/{id}/items` só funciona para playlists do usuário ou colaborativas.
  Nas outras, a resposta não tem `items`. Para adicionar ou tirar, use `POST` ou `DELETE
  /playlists/{id}/items`. Os campos `tracks.*` passaram a se chamar `items.*`.
- **Biblioteca:** para curtir e descurtir, use `PUT` e `DELETE /me/library` com URIs. Para saber se
  já está salvo, use `GET /me/library/contains`.
- **Fora da API:** as buscas em lote (`GET /tracks?ids=`), `GET /artists/{id}/top-tracks`,
  `browse/*` e `/users/{id}` saíram. Recomendações saíram em 2024.
- **Player** (`/me/player`, `/play`, `/pause`, `/next`, `/previous`, `/seek`, `/shuffle`, `/repeat`,
  `/volume`, `/queue`, `/devices`, `/recently-played`): sem mudança; exige Premium.
- O guia não cita `GET /artists/{id}/albums`, então ele continua valendo. É o que o artista mostra.

## O chip

| id | Nome | Na barra | Clique |
| --- | --- | --- | --- |
| `spotify` | Spotify | O logo oficial do Spotify no verde da marca (#1ED760), largura fixa como os chips de VS Code e Terminal. *(Mudou em 2026-10-27, a pedido do usuário; antes era uma nota musical.)* | Abre o painel "Spotify". |

Descrição no catálogo: "Busque e toque no Spotify sem abrir o app (Premium)". Ele entra em qualquer
seção de qualquer monitor, como os outros chips. O chip não mostra a música tocando: isso já é
papel do `player`.

## O painel

Nasce do chip, como o do VS Code. Largura de cerca de 400 DIP, linhas de cerca de 44 DIP com capa de
36 DIP, e altura que segue o conteúdo até um máximo. As medidas exatas ficam com quem constrói.

### Campo de busca

- Fica sempre no topo. O cursor já está nele ao abrir: o `WM_CHAR` chega pelo mesmo caminho do
  VS Code e do chat.
- O texto de apoio muda com a tela: "Buscar no Spotify…" na raiz e "Filtrar <nome>…" dentro de uma
  lista.
- Digitar na raiz busca no Spotify. Digitar dentro de uma lista filtra a lista aberta no próprio
  painel, sem rede, sem diferenciar maiúsculas nem acentos (como o `fold` do VS Code).
- **Busca na rede:** dispara 300 ms depois da última tecla *(chamada minha)*. Se uma resposta velha
  chegar depois de uma nova, ela é descartada.
- Ctrl+V cola, Backspace apaga, Ctrl+Backspace limpa.

### Raiz (campo vazio)

1. **Cartão da música atual:** capa, título, artista e o nome do aparelho, uma barra de progresso e
   os botões anterior, tocar/pausar, próxima e coração. Sem nada tocando: "Nada tocando". Os dados
   vêm de `GET /me/player`, e não do SMTC, para enxergar outro aparelho.
2. Três entradas, cada uma com `›`:
   - **Sua biblioteca:** "Curtidas, playlists, álbuns, artistas, podcasts";
   - **Fila:** "A seguir: <próxima>";
   - **Aparelhos:** "Tocando no <aparelho>".
3. **"Tocadas há pouco"**: até 5 contextos (playlist, álbum, artista) distintos, de
   `GET /me/player/recently-played`. *(Chamada minha, não estava no pedido: dá para tirar sem mexer
   no resto.)*

### Resultados da busca

- Filtros em pílulas: Tudo, Músicas, Artistas, Álbuns, Playlists e Podcasts.
- **Tudo:** uma consulta com todos os tipos e `limit=5`, agrupada por tipo, com um rótulo por
  grupo. Músicas e episódios mostram a duração à direita.
- **Um tipo só:** 10 por página. Mais páginas carregam ao rolar perto do fim, com `offset`.
- Ctrl+Tab e Ctrl+Shift+Tab trocam o filtro *(chamada minha)*. O clique na pílula também troca.

### Navegação em pilha

Entrar num item empilha uma camada nova, e Esc volta uma camada (com o campo vazio). Na raiz, Esc
fecha o painel. Uma trilha no topo ("‹ Biblioteca / Foco no código") mostra onde se está, e clicar
no "‹" volta.

| Camada | Conteúdo |
| --- | --- |
| Sua biblioteca | Músicas curtidas, Playlists, Álbuns, Artistas, Podcasts |
| Músicas curtidas | `GET /me/tracks`, paginado |
| Playlists | `GET /me/playlists`, paginado. "sua" ou "Spotify/<dono>" no subtítulo |
| Playlist sua ou colaborativa | 1ª linha "Tocar a playlist" (Shift+Enter: no aleatório) e as músicas |
| Playlist de outro dono | Só "Tocar a playlist" e "Tocar no aleatório", com a nota "O Spotify não mostra as músicas desta playlist" |
| Álbuns / Álbum | `GET /me/albums` / as faixas do álbum, com "Tocar o álbum" na 1ª linha |
| Artistas / Artista | `GET /me/following?type=artist` / os álbuns do artista (`/artists/{id}/albums`), com "Tocar o artista" na 1ª linha |
| Podcasts / Podcast | `GET /me/shows` / os episódios, mais novos primeiro |
| Fila | `GET /me/player/queue`: a atual e as próximas |
| Aparelhos | `GET /me/player/devices`: nome, tipo (ícone) e o ativo marcado |

As listas carregam ao entrar e mostram "Carregando…" até chegar. Ao voltar, a camada de baixo
continua como estava, com a mesma seleção e a mesma rolagem.

### Teclado e mouse

- ↑ e ↓ escolhem a linha, e a roda rola.
- **Enter faz a ação principal:**
  - em música ou episódio, toca;
  - dentro de uma playlist ou álbum, toca a partir dessa faixa, no contexto;
  - em lista, playlist, álbum, artista ou podcast, abre;
  - em aparelho, toca nele.
- Ctrl+Enter põe na fila. Shift+Enter toca no aleatório, onde fizer sentido.
- **Tab ou o botão direito** abrem o menu de ações da linha. Nesse menu, ↑ e ↓ escolhem, Enter
  confirma e Esc fecha só o menu.
- Atalhos diretos *(chamadas minhas, mostrados no menu)*:
  - Ctrl+L curte ou descurte;
  - Ctrl+P adiciona a uma playlist;
  - Ctrl+C copia o link;
  - Espaço com o campo vazio toca ou pausa.
- Clique numa linha faz a ação principal, e o mouse por cima destaca a linha, como no VS Code.
- Uma linha fixa no rodapé mostra os atalhos da tela atual ("↵ tocar · Ctrl+↵ fila · Tab ações ·
  Esc voltar").
- **Depois de tocar, o painel continua aberto** e o cartão da música atual se atualiza *(chamada
  minha: o Raycast fecha; aqui, fechar é um Esc)*.

### Menu de ações (por tipo)

| Linha | Ações |
| --- | --- |
| Música | Tocar · Pôr na fila · Curtir/Descurtir · Adicionar a playlist… · Tirar desta playlist (só dentro de uma playlist sua) · Ir para o álbum · Ir para o artista · Copiar link · Copiar "artista – título" · Copiar código de incorporação |
| Episódio | Tocar · Pôr na fila · Ir para o podcast · Copiar link |
| Álbum, playlist, artista ou podcast | Abrir · Tocar · Tocar no aleatório · Copiar link |
| Aparelho | Tocar aqui · Volume deste aparelho… |
| Cartão da música atual | Tocar/Pausar · Próxima · Anterior · Recomeçar · Curtir/Descurtir · Adicionar a playlist… · Aleatório (liga/desliga) · Repetir (desligado → contexto → faixa) · Volume… · −15 s / +15 s (só episódio) · Ir para o álbum · Ir para o artista · Copiar link · Copiar "artista – título" · Copiar código de incorporação |

- **Volume…** abre um submenu com 0, 25, 50, 75 e 100 %, além de −10 % e +10 %. Ele usa `PUT
  /me/player/volume` no aparelho ativo ou no escolhido. Esse volume é do Spotify, não o do Windows,
  que continua no chip `audio`.
- **Adicionar a playlist…** abre um submenu com as playlists que o usuário pode editar (as suas e
  as colaborativas), lidas de `GET /me/playlists`. Escolher uma adiciona a música no fim dela.
  - O submenu **não** mostra em quais playlists a música já está (decisão do usuário), então
    adicionar de novo duplica a música, como no próprio Spotify.
  - Para **tirar**, só existe "Tirar desta playlist", dentro de uma playlist sua aberta no painel.
  - O rodapé confirma por um instante: "Adicionada a <playlist>" ou "Tirada de <playlist>".
- **Código de incorporação** é o `<iframe>` do `open.spotify.com/embed/<tipo>/<id>`, nas medidas
  padrão do Spotify.
- Copiar usa a área de transferência do Windows e mostra "Copiado" por um instante no rodapé.

### Mensagens no lugar da lista

- "Carregando…"
- "Nenhum resultado"
- "Sem conexão com o Spotify". Fica na lista, e a raiz continua navegável com o que já carregou.
- "Nenhum aparelho ativo. Abra o Spotify em algum aparelho": aparece quando a API responde
  `NO_ACTIVE_DEVICE` ao tentar tocar. O painel vai sozinho para Aparelhos se houver algum na lista.
  *(Chamada minha.)*
- "Esta conta não é Premium": aparece num 403 com `PREMIUM_REQUIRED`.
- "Muitas consultas; tentando de novo em N s": aparece num 429, que respeita o `Retry-After`.

### Ao reabrir

- Reaberto em até 10 min, o painel volta à **raiz** com o campo vazio *(chamada minha)*. As listas
  já carregadas ficam em memória e não voltam a ser lidas da rede nesse intervalo.
- A raiz sempre relê `/me/player`, a fila (`GET /me/player/queue`, para a linha "A seguir") e
  `/recently-played`.

## Login

Fluxo de Authorization Code com PKCE: não há segredo no binário nem no disco, só o Client ID.

1. **Sem `client_id`:** o painel mostra uma tela de configuração:
   - um passo a passo curto, com o logo no título:
     - criar um app com a Web API em developer.spotify.com (o endereço é um botão que abre o painel
       do desenvolvedor no navegador);
     - cadastrar o Redirect URI `http://127.0.0.1:47823/callback` (o painel mostra esse texto com
       um botão "Copiar");
   - um campo "Client ID" onde a pessoa cola com Ctrl+V, e o botão "Salvar".

   O painel grava `[spotify] client_id` no `config.toml`.
2. **Com `client_id` e sem token:** botão "Entrar com o Spotify".
   - O botão abre o navegador padrão (`system::open_uri`) na página de autorização, com
     `code_challenge` (S256) e `state` aleatório.
   - Antes disso, uma thread abre um `TcpListener` em `127.0.0.1:47823`, que aceita **uma** conexão
     com o `state` certo, responde uma página "Pronto, pode voltar ao Zefiro" e fecha.
   - Se não chegar nada em 5 min, o listener fecha e o painel volta ao botão.
   - **Porta fixa:** o Spotify compara o Redirect URI exato. A porta 47823 é chamada minha.
   - Se a porta estiver ocupada, aparece o aviso "A porta 47823 está em uso".
3. O código vira um token de acesso e um token de renovação (`POST /api/token`).
   - O token de renovação é gravado **cifrado pelo Windows (DPAPI, `CryptProtectData`, escopo do
     usuário)** em `%LOCALAPPDATA%\zefiro\spotify\token.bin`.
   - O token de acesso só existe na memória da thread do serviço.
   - Quando o Spotify devolve um token de renovação novo, o gravado é trocado.
4. **Escopos:**
   - `user-read-playback-state`;
   - `user-modify-playback-state`;
   - `user-read-currently-playing`;
   - `user-read-recently-played`;
   - `user-library-read` e `user-library-modify`;
   - `playlist-read-private`, `playlist-read-collaborative`, `playlist-modify-private` e
     `playlist-modify-public`;
   - `user-follow-read`.
5. **401:** o serviço renova uma vez e repete a requisição. Se a renovação falhar (`invalid_grant`),
   apaga o `token.bin` e volta ao botão "Entrar com o Spotify".
6. **Sair:** fica no menu de ações do cartão da música atual, como "Sair da conta do Spotify". Apaga
   o `token.bin`. *(Chamada minha.)*

O `token.bin` só é apagado ao sair ou quando a renovação é recusada: é arquivo do Zefiro, não do
usuário, mas mesmo assim nada além disso apaga nada em `%APPDATA%\zefiro`.

## Quando roda e quanto custa

- **O serviço:** é uma thread `zefiro-spotify` que existe só com o chip em algum layout
  (`BarConfig::uses("spotify")`, `refresh_services()`). Ela dorme num canal e só acorda por pedido
  do painel.
- **Ao abrir o painel:** o serviço lê `/me/player` e `/recently-played`. Enquanto o painel está
  aberto, relê `/me/player` a cada 5 s, para o progresso e para música trocada em outro aparelho, e
  logo depois de cada ação. A barra de progresso anda sozinha entre leituras, a partir do
  `progress_ms` e da hora em que ele chegou. *(Chamada minha.)*
- **Painel fechado:** nenhuma consulta. Com o painel fechado por 10 min, as camadas e as capas em
  memória são descartadas. A thread continua, parada.
- **Capas:**
  - a imagem é a menor do `images` que tenha pelo menos 64 px, baixada com `http::get_bytes_with` e
    decodificada por `imaging::decode_bytes` no tamanho exato (36 DIP vezes a escala), na thread do
    serviço;
  - o cache em memória guarda no máximo 150 capas, cerca de 3 MB a 200 %, e as mais antigas saem
    primeiro;
  - nada vai para o disco;
  - enquanto a capa não chega, a linha mostra um quadrado com a cor do tema.
- **Orçamento:** com o painel fechado, no máximo +1 MB de memória privada. Com o painel aberto e
  rolado, no máximo +6 MB. Com o painel fechado, 0 % de CPU. Isso é medido com `perf.ps1` antes e
  depois, com o YASB fechado e a ilha desligada.
- **HTTP:** o `HttpClient` do Windows que já existe, sem cache (`http.rs`). Ele ganha uma função
  para `PUT`, `POST` e `DELETE` com corpo JSON ou de formulário e tempo limite. Não entra nenhuma
  dependência nova de HTTP.

## Logs e privacidade

- O token nunca vai para o log. O texto da busca também não *(chamada minha: é pessoal, como as
  conversas do chat)*.
- O log diz só o que aconteceu: "spotify: busca, 10 resultados em 230 ms", "spotify: tocar (faixa)",
  "spotify: 429, esperando 5 s", "spotify: renovação recusada, saindo".
- O "Exportar diagnóstico" não leva o `token.bin`.

## Configuração

```toml
[spotify]
client_id = ""   # do app criado em developer.spotify.com (Redirect URI http://127.0.0.1:47823/callback)
```

- O painel grava essa seção com `toml_edit`, como a `[terminal]`, passando pelo `config_worker`.
- Mudança só em `[spotify]` não fecha o painel: entra em `Config::same_except_panels`.
- Trocar o `client_id` invalida o token gravado: apaga o `token.bin` e pede para entrar de novo.
  Para saber de qual `client_id` o token é, o `token.bin` guarda os dois juntos, cifrados.

## Onde mora cada parte

| Parte | Crate |
| --- | --- |
| Endpoints e corpos, respostas → tipos (faixa, álbum, artista, playlist, podcast, episódio, aparelho, estado do player, fila), erros (`NO_ACTIVE_DEVICE`, `PREMIUM_REQUIRED`, 401, 429), PKCE (verificador, desafio S256 em base64url, `state`), URL de autorização, leitura do pedido do callback, URI → link e código de incorporação | `crates/integrations/spotify` (nova, portável, com testes) |
| Seção `[spotify]` no `Config` e a gravação | `crates/core` |
| Estado do painel (pilha de camadas, seleção, filtro local, menu de ações por tipo, atalhos, mensagens) e desenho em `DisplayList` | `crates/modules/bar/src/spotify.rs` (portável, com testes) |
| Thread do serviço, HTTP com método e corpo, DPAPI, listener do callback, capas por WIC | `crates/platform-windows` (`spotify.rs`, `http.rs`) |
| Catálogo, glifo, `PanelKind::Spotify`, teclas, cliques, roda, `refresh_services` e gravação do `client_id` | `crates/modules/bar/src/catalog.rs`, `controller.rs`, `crates/app` (`spotify_loop.rs`, como o `vscode_loop.rs`) |

A regra da crate `windows` continua valendo. O SHA-256 já existe em `zefiro-integration-update`
(`sha256_hex`), e o base64 em `zefiro-integration-chat`: reaproveite ou extraia em vez de duplicar,
se couber. O `TcpListener` é `std` e pode ficar na integração.

## Verificação

- **Testes da integração**, com respostas JSON reais copiadas como fixture, sem token nem e-mail:
  - busca com todos os tipos;
  - playlist sua, com `items`, e playlist do Spotify, sem `items`;
  - `/me/player` tocando, pausado e vazio (204);
  - fila;
  - aparelhos;
  - erro `NO_ACTIVE_DEVICE`;
  - 429 com `Retry-After`;
  - PKCE contra o exemplo do RFC 7636 (`dBjftJeZ4CVP-mJ92K9CQ`… → `E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM`);
  - leitura do callback com o `state` certo, errado ou com `error=access_denied`.
- **Testes do painel:**
  - Esc volta uma camada e, na raiz, fecha;
  - filtro local sem acento;
  - a resposta velha da busca é descartada;
  - o menu de ações certo para cada tipo;
  - "Tirar desta playlist" só aparece dentro de playlist sua;
  - a playlist de outro dono mostra só tocar;
  - Ctrl+Enter vira "pôr na fila".
- **Sonda `spotify_probe`:** entra com o `token.bin` existente e mostra o estado do player, os
  aparelhos e 3 resultados de uma busca passada por argumento, sem nunca imprimir o token.
- **Na tela:**
  1. configurar o Client ID;
  2. entrar;
  3. buscar e tocar uma música com o Spotify em segundo plano. A janela dele não pode vir para a
     frente;
  4. pôr na fila;
  5. abrir uma playlist sua e tocar a partir de uma faixa;
  6. abrir o Daily Mix e tocar;
  7. trocar para o celular em Aparelhos;
  8. curtir, adicionar a uma playlist e tirar dela;
  9. mudar o volume;
  10. sair da conta.
- **Medição:** `perf.ps1` com o chip na barra e o painel fechado, e depois com o painel aberto e
  rolado por 1 min. O resultado tem que ficar dentro do orçamento.
- **ADR novo:** `docs/adr/0024-chip-spotify.md`. Ele registra o login por PKCE com Client ID próprio,
  o DPAPI, a porta fixa, a API de 2026 e o painel no estilo Raycast. Também atualizar a tabela de
  estrutura do `AGENTS.md`.

## O que fica com quem constrói

- Medidas exatas, glifos (nota, aparelhos por tipo, coração, ✓) e a cor do destaque. O mockup usa
  verde só para mostrar o que está ativo, e o tema manda.
- A altura máxima do painel.
- O texto exato das mensagens.
- Como o progresso do cartão é animado: pelo compositor, nunca por `WM_TIMER` redesenhando.
- O tamanho das páginas fora da busca (a API permite 50).
- A ordem dos itens no catálogo.
