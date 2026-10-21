# ADR-0019: Uso do Claude e do Codex no Windows e no WSL, e HTTP sem cache

- **Status:** aceito
- **Data:** 2026-10-21
- **Branch:** `main` (commits `4731672` … ver `git log --grep usage`); spec em [`2026-10-21-uso-claude-codex-wsl-design.md`](../superpowers/specs/2026-10-21-uso-claude-codex-wsl-design.md), plano em [`2026-10-21-uso-claude-codex-wsl.md`](../superpowers/plans/2026-10-21-uso-claude-codex-wsl.md)
- **Complementa:** [ADR-0011](0011-widgets-do-yasb-fase-1e.md) (chips `claude` e `codex`)

## Contexto

Dois problemas relatados pelo usuário nos chips `claude` e `codex` (ADR-0011):

1. **Os chips não atualizavam.** O número só mudava quando a barra era reiniciada. No print do usuário, o painel do Zefiro dizia 5 h = 9 % e "Atualizado agora", e o Claude Code (mesma conta) dizia 17 %.
2. **O WSL ficava de fora.** O usuário tem uma conta do Claude no PowerShell e outra no WSL, e testers usam só o WSL. O Zefiro só lia `%USERPROFILE%`. O Codex também roda nos dois lados.

## Opções consideradas

**Para o chip parado:**

1. Mandar `Cache-Control: no-cache` no pedido. Não resolve: medido, a 2ª consulta continua vindo do cache.
2. **`HttpBaseProtocolFilter` com `ReadBehavior` e `WriteBehavior` = `NoCache` em `http::client()`, para todas as chamadas.** Escolhida.

**Para mostrar várias contas:**

1. Um chip por origem (`claude`, `claude-wsl`). Exige configuração, e o tester só WSL precisaria trocar de chip.
2. **Um chip por serviço, que descobre sozinho as origens.** O chip mostra a maior %. O painel tem um bloco por conta, e contas iguais aparecem juntas. Escolhida.

## Números medidos

Ambiente: Windows 11 Pro 26200, dois monitores 1920×1080 a 100 %, perfil do usuário, build release, YASB fechado, 2026-10-21.

| Medida | Resultado |
|---|---|
| `HttpClient` padrão, duas consultas seguidas (`http_cache_probe`) | 1ª da rede, **2ª do cache** |
| Com o filtro `NoCache` | as duas da rede |
| `wsl.exe --list --running --quiet` | ~67 ms, não liga a VM |
| Chip × API (`claude_usage_probe`), sem reiniciar a barra | 18:23: chip 34 %, API 34 % · 18:33: chip 34 %, API 34 % (o uso não mudou nesse intervalo; a atualização dentro do mesmo processo fica provada pela `http_cache_probe`) |
| Memória privada (`perf.ps1`, 60 s) | 1.1.1: 34,0 MB · nova: 31,95 MB |
| CPU em repouso | 0,0 % nas duas |

**Não medido:** se ler `\\wsl.localhost` a cada 120 s impede a distro de desligar sozinha. O WSL do usuário fica sempre ligado (Docker Desktop), e montar uma distro ociosa exigiria derrubar o Docker. O usuário aceitou manter a leitura (spec §10).

## Decisão

**HTTP sem cache.** `http::client()` cria o `HttpClient` com o filtro `NoCache`. Isso vale para o Claude, o clima, as letras e a atualização.

**Origens.** Uma origem é o Windows (`%USERPROFILE%`) ou uma pasta em `/home` de uma distro rodando (`\\wsl.localhost\<distro>\home\<usuário>`).
- **Distros instaladas:** vêm do registro (`HKCU\...\Lxss`).
- **Distros rodando:** vêm do `wsl.exe --list --running --quiet`, sem janela, com tempo limite de 3 s e cache de 3 s, para o Claude e o Codex dividirem a chamada.
- **Ignoradas:** `docker-desktop*` e `/root`.
- **Distro parada:** nunca é lida, porque a leitura a liga. Para fechar essa janela, o Claude lê todas as credenciais logo depois de descobrir as origens, antes de qualquer HTTP.

**Contas do Claude.**
- **Identidade:** `oauthAccount.accountUuid` do `~/.claude.json`. Sem ele, cada origem é uma conta.
- **Consulta:** usa o primeiro token válido entre as origens da conta, com o Windows primeiro.
- **Painel:** o título mostra o plano (`subscriptionType`) e o subtítulo, o e-mail. O e-mail só aparece no painel; o log registra "uso do Claude: 2 contas (Windows, WSL)".

**Contas do Codex.**
- **Identidade:** `creator_account_id` do `session_meta`, a 1ª linha de cada sessão.
- **Mesma conta em várias origens:** vale o evento mais recente entre elas.
- **Leitura:** o percurso da árvore de sessões é completo. Um percurso curto pelas pastas de data foi tentado e desfeito, porque `codex resume` grava na pasta do dia em que a sessão começou.
- **Reinício da janela:** o serviço acorda no `resets_at` mais próximo e avisa a UI.

**Distro que para.**
- O último valor continua valendo, marcado "WSL desligado".
- No Codex, as leituras da distro parada continuam entrando na conta juntada.
- Sem nenhuma conta e com distro instalada parada, o painel diz "WSL desligado. Abra o WSL para ver o uso de lá."

**Chip.** A barra é refeita quando muda o número desenhado (`UsageState::set_claude`/`set_codex`), comparando com o último número mostrado. Recalcular o snapshot antigo não serve: depois do `resets_at`, ele também dá 0 %, e o chip ficaria com o número velho.

**Serviços.** O `Drop` do `ClaudeService` e do `CodexService` não espera a thread. Ela pode estar presa num `wsl.exe`, num `\\wsl.localhost` ou num HTTP, e quem solta o serviço é a thread da UI. De quebra, isso resolve o minor da ADR-0011 "Sair pode esperar até 15 s pelo HTTP do Claude".

## Validação na tela

1. **Duas contas do Claude:** o painel mostra o bloco do Windows (Pro, 28 %/20 %) e o do WSL (Pro, "Login expirado no WSL. Abra o Claude Code no WSL para renovar."), com e-mail e origem.
2. **Codex:** um bloco só ("Codex · Plus"), com o último uso mais recente entre Windows e WSL (20/10 15:18, do WSL).
3. **Tester só WSL:** com `ZEFIRO_CONFIG_DIR` e `ZEFIRO_USAGE_WINDOWS_HOME` apontando para pastas vazias, aparece só a conta do WSL.
4. **Chip acompanha a API sem reiniciar a barra:** ver a tabela.

## Consequências

- **Pendente de validação na tela:** o bloco "WSL desligado" e o WSL que não volta a ligar sozinho. Em 2026-10-21, `wsl --shutdown` às 19:41:28 e o Ubuntu rodava de novo às 19:41:33, ligado pelo `claude.exe` (o Claude Desktop mantém um servidor remoto no WSL: `wsl.exe -d Ubuntu-24.04 --exec … ~/.claude/remote/srv/…`), não pelo Zefiro. Com o Claude Desktop aberto, a distro não fica parada. A regra fica coberta pelos testes `running_cache_is_short`, `credentials_are_read_before_any_query` e `service_keeps_*_when_distro_stops`; a conferência na tela precisa do Claude Desktop fechado.
- **Minor adiados** (lista no ledger do plano):
  - `wsl_stopped` fica ligado com outra distro rodando;
  - falta a linha de log de contas do Codex;
  - um 401 não tenta o próximo token;
  - o registro é lido duas vezes por ciclo;
  - "Login expirado no Windows" aparece numa conta juntada com os dois tokens vencidos;
  - numa conta do Claude juntada, quando a distro com o único token válido para, a conta cai para "expirado".
- **Testers que usam `root` no WSL** não têm a conta achada. Reabrir se aparecer.
