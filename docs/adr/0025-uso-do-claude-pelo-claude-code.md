# ADR-0025: Uso do Claude pelo `/usage` do Claude Code

- **Status:** aceito
- **Data:** 2026-10-28
- **Substitui:** a fonte do uso do Claude da ADR-0011 (token do `.credentials.json` → `GET /api/oauth/usage`) e a consulta por token da ADR-0019. A descoberta das contas e das origens da ADR-0019 continua valendo.

## Contexto

O chip `claude` lia o `accessToken` de `~/.claude/.credentials.json` e chamava `https://api.anthropic.com/api/oauth/usage` por conta própria. Desde fevereiro de 2026, a Anthropic diz que o token OAuth das assinaturas Free, Pro e Max só pode ser usado no Claude Code e no Claude.ai. O chip era exatamente isso: o token da assinatura usado fora do Claude Code, num endpoint não documentado. Com o Zefiro indo para outras pessoas, o risco (bloqueio do endpoint ou da conta) caía nelas.

## Opções consideradas

1. **Manter o endpoint com o token:** dá a % exata, mas contraria a regra da Anthropic.
2. **Estimar pelos `.jsonl` locais (como o ccusage):** não usa token, mas não sabe os limites do plano, então não dá a % de verdade.
3. **Rodar `claude -p /usage`:** quem lê o token, renova o login e fala com a Anthropic é o próprio Claude Code. Custa um processo por consulta.

## Números medidos

Claude Code 2.1.312, Windows 11 e Ubuntu-24.04 no WSL, 2026-10-28.

| Medida | Resultado |
|---|---|
| Tempo do `claude -p /usage` | ~2,4 s no Windows; a sonda com Windows e WSL levou 6,2 s |
| Modelo chamado | nenhum: `num_turns: 0`, `total_cost_usd: 0`, `local_command: "usage"` |
| Formato | com `--output-format stream-json --verbose`, a mensagem do comando traz `usage_report.rate_limits.limits[]` com `kind` (`session`, `weekly_all`, `weekly_*`), `percent` e `resets_at` em ISO com fuso |
| Texto do `result` | só para pessoas: as datas vêm no fuso local sem ano, e o `TZ=UTC` é ignorado no Windows (no WSL vale). Por isso o parser lê o `usage_report` e não o texto |
| Sem login (`CLAUDE_CONFIG_DIR` vazio) | `is_error: true`, `result: "Not logged in · Please run /login"` |
| Sessão gravada | sem `--no-session-persistence`, cada consulta grava um `.jsonl` em `~/.claude/projects`; com ele, nada |

## Decisão

O chip roda `claude -p /usage --output-format stream-json --verbose --setting-sources "" --no-session-persistence --strict-mcp-config` em cada origem da conta (o `claude.exe` do Windows, ou `wsl.exe -d <distro> -u <dono da pasta> -- bash -lc "cd ~ && DISABLE_AUTOUPDATER=1 exec claude …"`) e lê o `usage_report`. O Zefiro não lê mais o `.credentials.json`: só confere se ele existe (para saber que a origem tem login). Conta e plano saem do `.claude.json` (`oauthAccount.accountUuid`, `emailAddress` e `organizationType`, em que "claude_pro" vira "Pro").

- **Origens:** a primeira origem que responder vale (Windows, depois as distros). Antes do `wsl.exe`, a distro é conferida de novo na lista das que rodam, porque `wsl.exe -d` numa distro que parou a ligaria.
- **Erros:** "Not logged in" é `Missing`; o `claude` não achado (ou o bash saindo com 127) é `NotInstalled` ("Claude Code não encontrado no …"); `rate_limits` nulo, outro erro do Claude Code ou mais de 30 s é `Network` (o chip fica com o último valor); um stream sem `usage_report` é `UnknownFormat`.
- **Processo:** sem janela, stdout lido numa thread (o stream passa do buffer do pipe) e encerrado depois de 30 s. O `DISABLE_AUTOUPDATER=1` evita que a atualização automática fique rodando depois do comando.
- **Intervalos:** os mesmos de antes (5 min, 2 min ao abrir o painel, 60 s no botão), porque o Claude Code consulta o mesmo serviço.

## Consequências

- O "Login expirado" deixa de aparecer na prática: o Claude Code renova o token sozinho ao rodar o `/usage`. O texto do README sobre manter o Claude Code rodando no WSL para renovar o token não vale mais.
- O `/usage` é um comando de pessoa, não uma API: o `usage_report` pode mudar numa versão nova do Claude Code. Nesse caso o chip mostra "Formato de resposta desconhecido" em vez de um número errado; a sonda `claude_usage_probe` confere tudo de ponta a ponta.
- Cada consulta abre um processo do Claude Code (~2,5 s de CPU baixa a cada 5 min, por conta).
- Minors: `UsageError::Expired` e o caminho do 429 (`rate_limit_wait`, `retry_at`) continuam no código e na interface, mas o Claude Code não informa mais nenhum dos dois; o `http::get_with_retry_after` ficou sem uso no Claude.
