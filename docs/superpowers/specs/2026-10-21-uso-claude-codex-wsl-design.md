# Uso do Claude e do Codex: contas do Windows e do WSL, e chips que atualizam (spec)

> **Data:** 2026-10-21
> **Status:** aprovada pelo usuário em 2026-10-21.
> **Base:** [ADR-0011](../../adr/0011-widgets-do-yasb-fase-1e.md) (chips `claude` e `codex`), [spec da 1e](2026-10-19-fase-1e-widgets-do-yasb-design.md) §4.1–4.2, `crates/integrations/usage`, `crates/platform-windows/src/{claude,http}.rs`, `crates/app/src/usage_loop.rs`.

## 1. Intenção

O usuário usa os chips `claude` e `codex` o dia todo e percebeu dois problemas:

1. **Os chips não atualizam.** O número só muda quando ele fecha e abre a barra. No print de 2026-10-21, o painel do Zefiro dizia 5 h = 9 % e "Atualizado agora", enquanto o Claude Code no VS Code (mesma conta do Windows) dizia 17 %.
2. **Ele tem duas contas do Claude:** uma no PowerShell (Windows) e outra no WSL. Alguns testers do Zefiro **só** usam o Claude Code no WSL. Hoje o Zefiro só lê `%USERPROFILE%\.claude\.credentials.json`, então a conta do WSL não aparece e, para esses testers, o chip fica em "Faça login no Claude Code".

O usuário pediu que o Codex também identifique o uso feito no WSL e no PowerShell.

Sucesso: o chip mostra o mesmo número que o Claude Code (com até 2 min de atraso) sem reiniciar a barra; quem só usa o WSL vê o uso sem configurar nada; quem usa as duas contas vê as duas no painel, cada uma identificada.

## 2. O que foi medido (2026-10-21)

- **Causa do chip parado:** `http::get_with` usa o `Windows.Web.Http.HttpClient` com o filtro padrão, que guarda a resposta no cache e a devolve nas consultas seguintes **mesmo com `Cache-Control: no-cache` no pedido**. Sonda temporária (`examples/claude_cache_probe.rs`), três consultas seguidas no mesmo processo: a 1ª veio da rede (`HttpResponseMessage.Source` = Network) e a 2ª e a 3ª do cache (`Source` = Cache). Com um `HttpBaseProtocolFilter` com `CacheControl.ReadBehavior = NoCache` e `WriteBehavior = NoCache`, a consulta foi à rede e trouxe 17 %, o número do Claude Code. Reiniciar a barra "resolve" porque a 1ª consulta de cada processo vai à rede.
- O cabeçalho `Date` das respostas da rede vem até ~1 min atrasado em relação ao relógio: o servidor também segura o dado por um tempo. Isso é aceitável (o Claude Code vê o mesmo).
- **Distros:** `HKCU\Software\Microsoft\Windows\CurrentVersion\Lxss` lista `Ubuntu-24.04` e `docker-desktop`. `wsl.exe --list --running --quiet` leva ~67 ms e não liga a VM; a função `system::wsl_running_distros()` já existe e faz isso.
- **Ler `\\wsl.localhost\<distro>` com a distro parada a liga.** Por isso o Zefiro só lê distros que já estão rodando.
- **Claude no WSL:** `\\wsl.localhost\Ubuntu-24.04\home\felip\.claude\.credentials.json` existe, com o mesmo formato do Windows. `~/.claude.json` (Windows e WSL) tem `oauthAccount` com `accountUuid` e `emailAddress`. As duas contas do usuário têm `accountUuid` diferentes. `/root` dá "permissão negada" pelo `\\wsl.localhost`.
- **Codex no WSL:** `~/.codex/sessions` existe nos dois lados (8 sessões no Windows, do Codex Desktop; 22 no WSL, do `codex-tui`). A 1ª linha de cada sessão (`session_meta`) tem `payload.creator_account_id`; no usuário, é o **mesmo** nos dois lados (uma conta só). As sessões não têm e-mail; ele só existe no `~/.codex/auth.json`, que o Zefiro não abre (ADR-0011).
- **Codex reiniciado:** quando a janela de 5 h passa do `resets_at`, `effective_percent` já dá 0 %, mas o chip só é redesenhado quando o arquivo de sessão muda ou outra coisa refaz a barra. No print, o chip estava certo (0 %) por acaso.

## 3. Objetivos

1. Os chips `claude` e `codex` atualizam sozinhos, sem reiniciar a barra.
2. O Zefiro encontra sozinho as contas do Claude e do Codex no Windows e nas distros do WSL que estão rodando.
3. O painel mostra cada conta identificada; contas iguais aparecem uma vez.
4. Testers que só usam o WSL veem o uso sem configurar nada.

## 4. Fora do escopo

- **Chat com o Claude pela barra.** O usuário quer isso (um painel com campo de texto, usando o Claude Code sem janela), mas é outro projeto, com spec própria, depois deste.
- Renovar o token, rodar o Claude Code sozinho ou gravar qualquer arquivo do Claude ou do Codex (ADR-0011 continua valendo).
- Abrir `~/.codex/auth.json`.
- Ligar uma distro parada para ler.
- Configuração nova no `config.toml` (nenhuma chave nova; ver §9).
- Mudar o visual do chip (ícone, largura fixa "100%", cor de alerta a partir de 80 %).

## 5. Anti-objetivos (falhou mesmo se "funcionar")

- O Zefiro liga o WSL de quem deixou ele desligado.
- O token, o e-mail ou o caminho com o nome do usuário vão para o log, para o diagnóstico exportado ou para um teste. (O e-mail aparece **só** no painel.)
- O chip anda de largura ou o painel anda sob o cursor.
- CPU em repouso sai de 0,0 % ou a memória privada sobe mais que ~1 MB.
- Um tester sem WSL passa a ver erro, demora ou janela piscando (o `wsl.exe` roda sem janela).

## 6. Desenho

### 6.1 HTTP sem cache (correção)

`http::client()` passa a criar o `HttpClient` com um `HttpBaseProtocolFilter` com `CacheControl.ReadBehavior = NoCache` e `WriteBehavior = NoCache`. Vale para todas as chamadas de `http` (Claude, clima, letras, checagem e download da atualização): nenhuma delas quer dado velho. Precisa da feature `Web_Http_Filters` da crate `windows` (já adicionada no `Cargo.toml` do workspace durante a investigação).

A sonda `claude_cache_probe` vira uma sonda permanente `http_cache_probe`, que não usa o Claude: faz duas consultas seguidas a uma URL pública (a do clima) com `http::get` e imprime `Source` de cada uma. Antes da correção a 2ª dá "cache"; depois, "rede". Para isso, `http` ganha uma função interna de teste ou a sonda monta o pedido com `http::client()` exposto como `pub(crate)` + função `pub fn response_source_probe(url)`; a forma fica com quem implementa.

### 6.2 Origens

Uma **origem** é um lugar onde o Claude Code ou o Codex guarda dados:

| Origem | Rótulo no painel | Raiz |
|---|---|---|
| Windows | "Windows" | `%USERPROFILE%` |
| WSL | "WSL" (ou "WSL · <distro>" se houver mais de uma distro com conta) | `\\wsl.localhost\<distro>\home\<usuário>` |

Descoberta das origens do WSL, a cada ciclo do serviço (120 s) e ao abrir o painel:

1. `system::wsl_running_distros()`; sem WSL ou sem distro rodando, nenhuma origem do WSL.
2. Ignora distros cujo nome começa com `docker-desktop`.
3. Em cada distro rodando, lista `\\wsl.localhost\<distro>\home\*` e fica com as pastas que têm `.claude\.credentials.json` (Claude) ou `.codex\sessions` (Codex). `/root` fica de fora (sem permissão pelo `\\wsl.localhost`).

Um único `wsl.exe` por ciclo serve aos dois serviços (*chamada minha*: um pequeno `WslWatch` em `platform-windows`, com a lista das distros rodando e o instante da leitura, consultado pelos dois; se for mais simples, cada serviço chama o seu, desde que o total fique em no máximo dois `wsl.exe` a cada 120 s).

A lógica de "dada uma raiz, quais caminhos olhar" e de juntar contas (§6.3, §6.4) fica em `crates/integrations/usage`, portável e testada com pastas temporárias; `platform-windows` só fornece a lista de distros rodando e o HTTP.

**Distro que parou de rodar:** a origem continua na lista com o último resultado, marcada como desligada (§6.5), até a barra fechar. Ela não é lida enquanto estiver parada. Nada disso é gravado em disco (o e-mail não sai do processo).

**WSL instalado mas parado, sem nenhuma conta lida** (o tester que só usa o WSL, logo depois de ligar o PC): as distros instaladas vêm do registro `HKCU\...\Lxss` (`DistributionName`, sem `docker-desktop*`), que não liga o WSL. Se há distro instalada parada e nenhuma conta foi achada, o painel diz "WSL desligado. Abra o WSL para ver o uso de lá." no lugar de "Faça login no Claude Code" (e o mesmo no Codex no lugar de "Nenhum uso do Codex encontrado"). O chip fica "—" apagado. *Chamada minha.*

**Ociosidade do WSL (medir antes de implementar a descoberta):** a primeira tarefa do plano mede se ler `\\wsl.localhost` a cada 120 s impede a distro de desligar sozinha. Se impedir, a leitura continua a cada 120 s (decisão do usuário, §10) e o número medido vai para o ADR.

### 6.3 Claude: contas

Por origem com `.credentials.json`:

- **Identidade:** `oauthAccount.accountUuid` do `.claude.json` da mesma raiz. Sem o arquivo ou sem o campo, a identidade é a própria origem (não junta com ninguém).
- **E-mail:** `oauthAccount.emailAddress` do mesmo arquivo; sem ele, o título usa só o rótulo da origem.
- **Plano:** `claudeAiOauth.subscriptionType` do `.credentials.json` ("pro" → "Pro", "max" → "Max"); sem ele, sem plano no título.
- **Consulta:** igual à de hoje, por conta: relê o `.credentials.json` a cada consulta, `token_state`, `GET /api/oauth/usage`. Nunca grava nem renova.

Origens com a mesma identidade viram **uma conta**. A consulta usa o token válido de qualquer uma delas (primeiro o do Windows, depois as do WSL em ordem alfabética de distro); o rótulo mostra as origens juntas ("Windows · WSL").

Uma origem cujo `.credentials.json` não existe não vira conta. Se nenhuma origem tem credenciais, o painel continua com "Faça login no Claude Code".

O serviço `zefiro-claude` continua sendo **uma** thread: a cada ciclo de 120 s (e no `refresh` do painel, se o dado tiver mais de 60 s) descobre as origens e consulta cada conta em sequência. O estado compartilhado passa a ser uma lista de contas, cada uma com rótulo, e-mail, plano, origem(ns), instante e `Result<ClaudeUsage, UsageError>`, mais o último valor bom (para o chip em falha passageira, como `claude_last_ok` hoje).

### 6.4 Codex: contas

Por origem com `.codex\sessions`:

- **Leitura:** como a de hoje (`read_codex`), mas agrupada por conta: dos `MAX_FILES` `.jsonl` mais novos da origem, guarda o `rate_limits` mais recente **de cada conta**, não só o mais recente da origem. Assim, quem trocou de login numa origem não perde a conta anterior enquanto ela estiver entre os arquivos recentes. *Chamada minha.*
- **Identidade:** `payload.creator_account_id` da 1ª linha (`session_meta`) de cada arquivo. Arquivo sem o campo (Codex antigo) entra na conta do arquivo mais recente **com** o campo na mesma origem; se nenhum tiver, a identidade é a origem. *Chamada minha.*
- **Mesma identidade em várias origens:** vira uma conta só, com o `rate_limits` do evento mais recente entre elas (pelo `timestamp` do evento). É o caso do usuário.
- **Contas diferentes:** uma por identidade.
- **Título:** "Codex · <Plano>" como hoje; com mais de uma conta, cada bloco leva também o rótulo da origem ("Codex · Plus · WSL").

Desempenho: o percurso recursivo da árvore de sessões pelo `\\wsl.localhost` é mais lento que no disco local. *Chamada minha:* o percurso desce primeiro nas pastas de ano, mês e dia mais recentes (`sessions/AAAA/MM/DD`) e para quando já juntou `MAX_FILES` arquivos; pastas fora desse padrão continuam sendo percorridas como hoje. O `stat` do arquivo mais novo continua decidindo se relê.

**Janela que reinicia:** a thread `zefiro-codex` espera no máximo até o menor `resets_at` futuro entre as janelas lidas (ou 120 s, o que vier antes) e avisa a UI quando ele passa, mesmo sem arquivo novo. A UI refaz o chip, que passa a mostrar 0 %. Nada de temporizador na thread da UI.

### 6.5 Chip

- **Claude:** a maior % de 5 h entre as contas que têm dado (resultado bom ou, numa falha passageira, o último bom), contando 0 % na janela já reiniciada (`resets_at` no passado). Sem nenhuma conta com dado, "—" apagado, como hoje.
- **Codex:** a maior % da janela primária entre as contas, com `effective_percent` como hoje.
- Origem desligada (distro parada) entra no máximo com o último valor, porque o limite continua valendo; quando a janela dela reinicia, conta 0 %.
- Cor e largura do chip: sem mudança.

### 6.6 Painel

Um bloco por conta, empilhados na ordem: Windows primeiro, depois WSL por distro. Com uma conta só, o painel fica **igual ao de hoje**, só com o e-mail num subtítulo.

Cada bloco do Claude:

- **Título do bloco:** "Claude · Pro" e, abaixo, apagado: "felipe@… · Windows" (e-mail cortado com reticências se não couber na largura do painel).
- As linhas de hoje: 5 horas, 7 dias e os limites por modelo.
- **Mensagem no lugar das linhas,** por conta:
  - login expirado: "Login expirado no <origem>. Abra o Claude Code no <origem> para renovar.";
  - sem rede, HTTP e formato desconhecido: como hoje.
- **Origem desligada:** as linhas do último valor, e no subtítulo "· WSL desligado".

O rodapé "Atualizado há …" fica um só, do fim da última consulta.

Cada bloco do Codex: título como §6.4, as linhas de hoje, e o rodapé "Último uso do Codex: …" passa a ser por bloco (vira o subtítulo do bloco).

O painel cresce na altura com os blocos. A largura não muda (340 DIP).

### 6.7 Log e privacidade

- O log registra quantas contas e origens foram achadas quando o número muda ("uso do Claude: 2 contas (Windows, WSL)"), nunca e-mail, token, `accountUuid` ou caminho com nome de usuário.
- Erros por conta são registrados como hoje (só quando mudam), com o rótulo da origem.
- O diagnóstico exportado e o HUD não ganham campos novos com e-mail.

## 7. Onde fica cada coisa

| Camada | O que muda |
|---|---|
| `crates/integrations/usage` | Tipos `Origin` (rótulo, raiz, desligada?), `ClaudeAccount`/`CodexAccount`; descoberta de caminhos por raiz; leitura de `accountUuid`/e-mail/plano; `creator_account_id` do `session_meta`; junção por identidade; % do chip com várias contas; percurso das sessões pelas pastas mais recentes; `CodexService` com várias origens e o aviso no `resets_at`. |
| `crates/platform-windows` | `http::client()` sem cache; `claude.rs` com várias contas; o `WslWatch` (ou o uso direto de `wsl_running_distros`). |
| `crates/modules/bar` | `usage_panel` com blocos (título, subtítulo, linhas ou mensagem). |
| `crates/app` | `usage_loop.rs`: estado com listas de contas, chip e painel a partir delas; raízes das origens. |

Regra de ouro da crate `windows` mantida: nada de `windows` em `integrations/usage` nem em `modules/bar`.

## 8. Testes e verificação

- **TDD nas partes portáveis** (`integrations/usage`), com pastas temporárias simulando raízes do Windows e do WSL: descoberta; identidade do Claude com e sem `.claude.json`; junção de duas origens com o mesmo `accountUuid`; Codex com o mesmo `creator_account_id` em duas origens ficando com o evento mais recente; contas diferentes; chip com o máximo e com janela reiniciada; percurso pelas pastas mais recentes; `CodexService` avisando no `resets_at` sem arquivo novo.
- **`usage_panel`** com uma conta (igual ao de hoje, mais o subtítulo) e com duas.
- **Sonda `http_cache_probe`:** a 2ª consulta sai "cache" antes da correção e "rede" depois.
- **Sonda de contas** (`claude_accounts_probe`): lista as origens e contas achadas, com rótulo, plano e se o token está válido, **sem** token, e-mail ou `accountUuid` (o e-mail aparece como "tem e-mail").
- **Na tela, com o usuário** (perfil isolado com `ZEFIRO_CONFIG_DIR` e depois o dele):
  - o chip do Claude acompanha o Claude Code por 10 min sem reiniciar a barra;
  - com o WSL rodando, o painel mostra as duas contas do Claude, com e-mail e origem;
  - o Codex mostra um bloco só (mesma conta) com o evento mais recente entre Windows e WSL;
  - `wsl --shutdown`: o bloco do WSL fica "WSL desligado", o WSL **não** volta a ligar sozinho em 5 min (`wsl -l --running` vazio);
  - sem nada do Windows (raiz do Windows apontada para uma pasta vazia por variável de teste, *chamada minha*: `ZEFIRO_USAGE_WINDOWS_HOME`), só a conta do WSL aparece, como num tester.
- **Medição** (`perf.ps1`, YASB fechado): CPU em repouso 0,0 % e memória privada ≤ +1 MB em relação à 1.1.1.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `scripts/check-windows-imports.ps1`.

## 9. Chamadas minhas (para o usuário conferir)

- Nenhuma chave nova no `config.toml`: o WSL é sempre lido quando está rodando e o chip está na barra. Se alguém quiser desligar, vira pedido novo.
- Distros `docker-desktop*` ignoradas.
- `/root` do WSL fica de fora.
- Um `wsl.exe` por ciclo, compartilhado pelos dois serviços.
- Com contas iguais, o Claude consulta com o token do Windows primeiro.
- O chip mostra o máximo entre as contas, inclusive as de distros desligadas, até a janela delas reiniciar.
- O percurso das sessões do Codex pelas pastas de data mais recentes.
- A variável `ZEFIRO_USAGE_WINDOWS_HOME` só para testes.
- O HTTP sem cache vale para todas as chamadas, não só para o Claude.

## 10. Decisões do usuário

- **Ociosidade do WSL (2026-10-21):** aceito manter a leitura a cada 120 s. O usuário deixa o WSL rodando o tempo todo. A medição continua no plano e o resultado vai para o ADR, para os testers saberem.

## 11. Documentação

- ADR novo (`0019-uso-em-varias-origens.md`): HTTP sem cache (com o número medido) e contas do Windows e do WSL.
- `AGENTS.md`: em "Coisas não óbvias", o cache do `HttpClient` e a regra de não ler distro parada.
- `docs/PLANNING.md`: sem mudança de roteiro.
