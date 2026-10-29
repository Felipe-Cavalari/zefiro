# Assinatura do Zefiro pela SignPath Foundation — design

Data: 2026-10-29. Estado: rascunho para aprovação.

## Por quê

Quem baixa o `Zefiro-x.y.z-setup.exe` da release vê a tela azul do SmartScreen ("O Windows protegeu o
computador", fornecedor desconhecido). Isso acontece com todo mundo, porque nem o instalador nem o
`zefiro.exe` são assinados. A meta é que a instalação pelo download direto do GitHub pare de mostrar o
aviso, sem mudar o canal de distribuição nem a atualização automática (ADR-0014).

Caminhos descartados:

- **Azure Artifact Signing:** não aceita pessoa física fora dos EUA e do Canadá.
- **Certificado próprio (Certum etc.):** é pago, e a chave em token ou na nuvem complica o CI.
- **Microsoft Store:** o download direto continuaria com o aviso.

O escolhido é a **SignPath Foundation**: é grátis para open source, assina no GitHub Actions e o
certificado sai em nome de "SignPath Foundation".

**Expectativa honesta:** a assinatura não zera o aviso no primeiro dia. O SmartScreen decide pela
reputação de quem assina. A aposta (palpite, não medido) é que o certificado da SignPath Foundation,
que já assina muitos projetos, tire o aviso bem antes do que um certificado novo tiraria.

## Objetivos

1. Toda release criada pela tag `vX.Y.Z` publica um instalador assinado, com o `zefiro.exe` de dentro
   também assinado. A assinatura é Authenticode com carimbo de tempo.
2. O atualizador continua igual. O SHA-256 que ele confere vem do `digest` que o GitHub calcula sobre o
   arquivo enviado (`crates/integrations/update/src/lib.rs`), e o arquivo enviado já é o assinado.
3. O repositório cumpre os [termos da SignPath Foundation](https://signpath.org/terms). Ver "Requisitos
   da SignPath" abaixo.

## Não objetivos

- Assinar os builds locais (`scripts/package.ps1` na máquina continua gerando instalador sem
  assinatura).
- Assinar o desinstalador `unins000.exe`. Assinar esse arquivo exige que o Inno Setup chame a
  assinatura no meio da compilação (`SignTool`). Ele roda do disco, sem a marca de "baixado da
  internet", então o SmartScreen não age nele. *(decisão minha)*
- Microsoft Store, winget e certificado próprio.

## Anti-objetivos (falha mesmo que "funcione")

- Uma release sair **sem assinatura sem ninguém perceber** depois que a SignPath estiver ligada.
- O token da SignPath aparecer em log, no repositório ou em artefato.
- O release publicar o instalador antes de a assinatura terminar, ou publicar o arquivo sem assinatura
  por cima do assinado.
- A atualização quebrar porque o arquivo publicado não é o mesmo cujo `digest` o GitHub informa.

## Requisitos da SignPath e como o Zefiro cumpre

| Exigência | Como fica |
| --- | --- |
| Licença OSI, sem dual-licensing comercial | MIT OR Apache-2.0. As duas são OSI e nenhuma é comercial, então deve passar (o "dual" proibido é o comercial). |
| Repositório público, mantido pela equipe | `github.com/Felipe-Cavalari/zefiro`, mantido por Felipe Cavalari. |
| Build automático e verificável | Os jobs até a assinatura rodam só em runner hospedado do GitHub (`windows-latest`). |
| Já lançado na forma que será assinada | Há releases com o instalador desde a v1.0.0. |
| Reputação verificável | **Risco:** o repositório tem 10 dias, 0 estrelas e cerca de 30 downloads. A SignPath pode recusar ou pedir para tentar mais tarde. |
| Papéis e MFA | Felipe Cavalari é autor, revisor e aprovador, com MFA no GitHub e na SignPath. |
| "Code signing policy" na página inicial e na de download | Seção no `README.md` (página inicial do repositório). As notas de cada release ganham o link. |
| Privacidade declarada, mostrada na instalação e com como desligar | Seção de privacidade no README e uma página no instalador com o mesmo resumo. Ver o texto abaixo. |
| Desinstalação | O Inno Setup já instala o desinstalador. |
| Nome e versão do produto impostos | A configuração de artefato da SignPath exige `product-name="Zefiro"` e `product-version="${version}"`. |
| Aprovação manual por release | Cada pedido de assinatura espera a aprovação do Felipe no site da SignPath. |

## Fluxo do `release.yml`

Os passos atuais ("Conferir a versão" e "Testes") continuam iguais. Depois deles:

1. **Compilar:** `cargo build --release -p zefiro-app`.
2. **Enviar o exe sem assinatura:** `actions/upload-artifact@v4` (ou mais novo) com
   `target/release/zefiro.exe` e `id: unsigned-exe`. O GitHub guarda como zip, então a raiz da
   configuração de artefato é `<zip-file>`.
3. **Assinar o exe:** `signpath/github-action-submit-signing-request@v3` com:
   - `artifact-configuration-slug: exe`;
   - `github-artifact-id` do passo 2;
   - `wait-for-completion: true` e `wait-for-completion-timeout-in-seconds: 3600`, para o Felipe ter
     1 h para aprovar *(decisão minha)*;
   - `output-artifact-directory: signed/exe`;
   - `parameters: version: "<versão do Cargo.toml>"`.
4. **Pôr o exe assinado no lugar:** copiar `signed/exe/zefiro.exe` por cima de
   `target/release/zefiro.exe` e conferir com `Get-AuthenticodeSignature`: `Status = Valid` e o
   assinante contendo "SignPath Foundation". Se não, o job falha.
5. **Empacotar sem recompilar:** `scripts/package.ps1 -SkipBuild` (opção nova: pula o `cargo build` e
   o rename do exe em uso e só roda o ISCC). Sem `-SkipBuild`, o script faz o mesmo de hoje.
6. **Enviar o instalador sem assinatura:** `upload-artifact` com `dist/Zefiro-<versão>-setup.exe`.
7. **Assinar o instalador:** a mesma ação, com `artifact-configuration-slug: installer` e saída em
   `signed/setup`.
8. **Conferir:** `Get-AuthenticodeSignature` no instalador assinado, como no passo 4.
9. **Publicar:** o passo "Publicar na release" de hoje, só que enviando o arquivo de `signed/setup`.
   Na release criada pelo git, `--generate-notes` junto com `--notes` levando a linha da política:
   "Instalador assinado. Code signing policy: <link para a seção do README>". Na release criada no
   site, acrescentar essa linha às notas com `gh release edit` se ela ainda não estiver lá.

Cada release pede **duas aprovações** na SignPath. O instalador embute o exe, então o exe precisa
estar assinado antes de o instalador ser gerado.

**Antes da aprovação da SignPath:** os passos 2–4, 6–8 e a linha da política rodam só se a variável
de repositório `SIGNPATH_ORGANIZATION_ID` existir (`if: vars.SIGNPATH_ORGANIZATION_ID != ''`). Sem ela,
o release sai sem assinatura, como hoje, e o log do job diz isso numa linha. Com ela, qualquer falha
na assinatura derruba o job e nada é publicado. Ligar a variável é o "liga" explícito, e por isso
não conta como o anti-objetivo de sair sem assinatura sem ninguém perceber. *(decisão minha)*

Segredos e variáveis do repositório:

- `secrets.SIGNPATH_API_TOKEN`: token de um usuário com permissão de submitter.
- `vars.SIGNPATH_ORGANIZATION_ID`: id da organização.
- Os slugs `project-slug: zefiro` e `signing-policy-slug: release-signing` ficam fixos no workflow.
  Se a SignPath criar com outros nomes, o construtor ajusta.

## Versão nos executáveis

- **`zefiro.exe`:** o `winresource` já põe `ProductName = Zefiro` e a versão do `Cargo.toml`.
- **Instalador:** acrescentar `VersionInfoProductTextVersion={#AppVersion}` ao `[Setup]` do
  `installer/zefiro.iss`. Sem isso, o Inno grava "1.3.1.0", e a SignPath compara com "1.3.1".
  `VersionInfoProductName` já sai "Zefiro" do `AppName`. *(decisão minha; conferir na primeira
  assinatura)*

## Configurações de artefato (coladas no site da SignPath)

Ficam também no repositório em `.signpath/artifact-configurations/exe.xml` e `installer.xml`, como
referência. A SignPath lê a cópia do site, não a do repositório.

```xml
<!-- exe -->
<artifact-configuration xmlns="http://signpath.io/artifact-configuration/v1">
  <parameters>
    <parameter name="version" required="true" />
  </parameters>
  <zip-file>
    <pe-file path="zefiro.exe" product-name="Zefiro" product-version="${version}">
      <authenticode-sign/>
    </pe-file>
  </zip-file>
</artifact-configuration>
```

```xml
<!-- installer -->
<artifact-configuration xmlns="http://signpath.io/artifact-configuration/v1">
  <parameters>
    <parameter name="version" required="true" />
  </parameters>
  <zip-file>
    <pe-file path="Zefiro-${version}-setup.exe" product-name="Zefiro" product-version="${version}">
      <authenticode-sign/>
    </pe-file>
  </zip-file>
</artifact-configuration>
```

## Texto da política (README)

Seção nova `## Política de assinatura de código (Code signing policy)`, antes de "Créditos":

- Linha obrigatória, em inglês como a SignPath pede: "Free code signing provided by
  [SignPath.io](https://signpath.io), certificate by [SignPath Foundation](https://signpath.org)."
- Papéis: autores e revisores: [Felipe-Cavalari](https://github.com/Felipe-Cavalari). Aprovadores:
  [Felipe-Cavalari](https://github.com/Felipe-Cavalari).
- Só são assinados os arquivos gerados pelo `release.yml` a partir de uma tag deste repositório.

Seção `### Privacidade`, com a lista do que sai da máquina e como desligar:

| Serviço | Quando | O que vai | Como desligar |
| --- | --- | --- | --- |
| GitHub (`api.github.com`) | A cada 24 h, só na cópia instalada | Pedido da última release | `[update] check = false` |
| wttr.in | Com a ilha ou o chip `weather` | A cidade da config, ou nada (o wttr.in usa o IP) | Tirar o chip / `[island] enabled = false` |
| lrclib.net | Com a aba de letras aberta | Artista e música tocando | Não abrir a aba |
| Spotify | Só depois do login no chip `spotify` | Pedidos da Web API na conta do usuário | Tirar o chip |
| Claude, Codex e Grok | Com os chips de uso ou o chat | Pelos CLIs do próprio usuário, nas contas dele | Tirar os chips |

O texto também diz que o Zefiro não tem telemetria e que o "Exportar diagnóstico" grava um zip local e
não envia nada.

## Página de privacidade no instalador (decidido pelo usuário)

O instalador mostra o resumo de privacidade numa página própria, depois da licença:
`InfoBeforeFile=privacidade.txt` no `[Setup]` do `installer/zefiro.iss`, com o arquivo
`installer/privacidade.txt` em português. O conteúdo é o mesmo da tabela acima, em texto corrido curto
(uma linha por serviço com "quando" e "como desligar"), mais o link para a seção do README. O README e
o `privacidade.txt` precisam dizer a mesma coisa. Quem mudar um muda o outro, e o ADR registra isso.
O instalador é só em português (como hoje), e o arquivo vai em UTF-8 com BOM para o Inno mostrar os
acentos.

## Passos do usuário (fora do código)

1. Ligar o MFA no GitHub, se ainda não estiver ligado.
2. Mandar o pedido em [signpath.org](https://signpath.org) ("Apply"), apontando para o repositório e
   para a seção da política (já publicada).
3. Aprovado o projeto: criar a conta na SignPath com MFA, ligar o "trusted build system" do GitHub.com
   ao projeto, instalar o SignPath GitHub App no repositório e colar as duas configurações de artefato.
4. Criar o token de API e pôr em `SIGNPATH_API_TOKEN`; pôr o id da organização em
   `SIGNPATH_ORGANIZATION_ID`.
5. Na próxima tag, aprovar os dois pedidos no site da SignPath.

A ordem importa. A política precisa estar no README antes do pedido, então o código entra em duas
partes:

- **Parte A**, antes do pedido: README e privacidade, a página de privacidade no instalador,
  `-SkipBuild`, `VersionInfoProductTextVersion`, as XMLs e o workflow com os passos condicionais. Essa parte é toda inerte enquanto
  `SIGNPATH_ORGANIZATION_ID` não existir.
- **Parte B:** ligar as variáveis, depois da aprovação.

## Documentos

- **ADR-0029** "Assinatura pela SignPath Foundation": registra a escolha, os caminhos descartados, as
  duas aprovações por release e o desinstalador sem assinatura.
- **`docs/RELEASES.md`:** as aprovações na SignPath e o que fazer se o pedido expirar (rodar o job de
  novo; a SignPath aceita até 3 reexecuções).
- **`AGENTS.md`:** uma linha no bloco de releases.

## Verificação

- Local: `scripts/package.ps1` sem opção gera o mesmo instalador de hoje, e `-SkipBuild` usa o exe que
  já está em `target\release`.
- `actionlint` (se houver) ou revisão do YAML. Com a variável desligada, a próxima release segue
  igual à de hoje.
- Primeira release assinada: baixar o instalador num perfil limpo e conferir
  `Get-AuthenticodeSignature` nos dois arquivos, a atualização pelo menu do Z de uma versão anterior e
  o que o SmartScreen mostra. O resultado vai para o ADR.

## Fica com o construtor

- A forma exata do YAML: nomes dos passos, onde ler a versão (o passo "Conferir a versão" já calcula)
  e como passar o caminho do artefato.
- Se a SignPath recusar `required="true"` sem `default-value`, ou criar slugs diferentes, ajustar.
