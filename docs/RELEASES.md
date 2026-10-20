# Como soltar uma release do Zefiro

O Zefiro instalado procura versão nova no GitHub Releases uma vez por dia (ADR-0014). Quando acha,
aparece **"Atualizar para a x.y.z"** no menu do Z. Para isso funcionar, cada release precisa:

- de uma tag `vX.Y.Z` igual à versão do `Cargo.toml` (`[workspace.package] version`);
- de um asset `Zefiro-X.Y.Z-setup.exe` anexado;
- não ser rascunho nem pré-release (essas o Zefiro ignora).

O workflow `.github/workflows/release.yml` cuida do instalador e do anexo. Você só sobe a versão e
cria a tag.

## Passo a passo

1. **Escolha o número.** `X.Y.Z`: correção → Z (1.1.0 → 1.1.1); recurso novo → Y (1.1.1 → 1.2.0);
   mudança grande → X.
2. **Suba a versão** no `Cargo.toml`, na seção `[workspace.package]`:

   ```toml
   version = "1.2.0"
   ```

3. **Faça o commit** (o `Cargo.lock` muda junto):

   ```powershell
   cargo build -p zefiro-app
   git commit -am "chore: release 1.2.0"
   ```

4. **Crie a tag e envie** (o push da `main` e da tag):

   ```powershell
   git tag v1.2.0
   git push origin main v1.2.0
   ```

5. **Acompanhe** em github.com/Felipe-Cavalari/zefiro → **Actions** → "release". Leva uns 10 min:
   confere a versão, roda o clippy e os testes, instala o Inno Setup, gera o instalador e cria a
   release com notas automáticas (a lista de commits desde a anterior).
6. **Pronto.** Em até 24 h (ou ao reabrir o Zefiro, depois de 60 s), quem tem a versão anterior vê
   "Atualizar para a 1.2.0" no menu do Z.

## Pelo site, sem tag no terminal

Em **Releases → Draft a new release**: digite a tag nova (`v1.2.0`), escolha a `main`, escreva as
notas e clique em **Publish release**. A tag nova dispara o mesmo workflow, que só anexa o
instalador. A versão do `Cargo.toml` precisa já estar commitada na `main`.

## Se algo der errado

- **"A tag não bate com a versão"**: o `Cargo.toml` não foi atualizado. Apague a tag
  (`git tag -d v1.2.0` e `git push origin :refs/tags/v1.2.0`), suba a versão, faça o commit e crie a
  tag de novo.
- **Workflow falhou nos testes**: conserte na `main`, apague e recrie a tag como acima.
- **Release sem o `setup.exe`**: o Zefiro não oferece a atualização. Rode o workflow de novo (Actions
  → release → Re-run) ou anexe o arquivo à mão (`pwsh -File scripts/package.ps1` e arraste o
  `dist\Zefiro-X.Y.Z-setup.exe` para a release).
- **Release local, sem GitHub Actions**: `pwsh -File scripts/package.ps1` gera o instalador, e
  `gh release create v1.2.0 dist\Zefiro-1.2.0-setup.exe --generate-notes` publica.

## Testar a atualização antes de publicar

O build de desenvolvimento (`target\release`) não checa atualização. Para testar:

```powershell
$env:ZEFIRO_UPDATE_CHECK = "1"; ./target/release/zefiro.exe
```

Ele consulta a release mais nova 60 s depois de abrir. Com a versão local menor que a publicada, a
linha aparece no menu do Z. Para desligar a checagem de vez: `[update] check = false` no
`config.toml`.
