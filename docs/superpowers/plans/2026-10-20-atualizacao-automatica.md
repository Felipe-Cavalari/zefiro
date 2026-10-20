# Atualização automática — plano

Spec: `docs/superpowers/specs/2026-10-20-atualizacao-automatica-design.md`.

1. **`zefiro-integration-update`** (crate nova, portável): `Version`, `parse_latest`, `update_for`,
   `trusted_url`, `sha256_hex`, `verify`. Testes com o JSON real da v1.0.1, vetores do SHA-256.
2. **Config:** `[update] check` (padrão `true`). Testes no `zefiro-core`.
3. **Menu do Z:** linha de atualização (`MenuAction::InstallUpdate`) com texto variável. Testes no
   `zefiro-bar`.
4. **Plataforma:** `http::get_bytes_with`, serviço `update` (thread, estados, download, rodar o setup).
   Teste de integração só do que não precisa de rede.
5. **App:** `update_loop` (liga com `refresh_services`, `USER_UPDATE`, menu, instalar e sair).
6. **Instalador:** reabrir na instalação silenciosa.
7. **Release:** `.github/workflows/release.yml`, `docs/RELEASES.md`, versão 1.1.0, ADR-0014, AGENTS.
