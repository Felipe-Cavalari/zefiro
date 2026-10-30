# Chip Comandos: cola de comandos de dev na barra — design

Data: 2026-10-30. Desenho escolhido com o usuário em conversa e com mockups
(`.superpowers/brainstorm/774-1793384420/content/painel-comandos.html`): dropdown da ferramenta em
cima, busca embaixo, resultados em **lista compacta (A)**, e uma seção "Mais usados" quando não há
busca. O usuário aprovou isso em conversa ("A, sim a seção mais usados faz sentido"). O que está
marcado como *(chamada minha)* ainda não foi visto por ele.

## Intenção

No meio do trabalho, o usuário esquece um comando ("como apago o cache do Docker?", "como subo o
FastAPI com o uvicorn?") e hoje precisa abrir o navegador. O chip `comandos` abre um painel onde ele
escolhe a ferramenta ou linguagem, busca o que quer fazer e copia o comando, com uma linha dizendo o
que ele faz.

É uma consulta rápida de **comandos**, não um leitor de documentação: sem páginas longas, sem rede
para consultar.

## Objetivos

1. Escolher a ferramenta num dropdown (Todas, Docker, Linux, PowerShell, Python, Git…) e buscar
   dentro dela pelo que se quer fazer, em português, sem acento e sem diferenciar maiúsculas.
2. Cada resultado mostra o comando, o que ele faz em uma linha, e copia com Enter ou um clique.
3. Sem busca digitada, o painel mostra "Mais usados" da ferramenta escolhida.
4. Funciona offline e abre na hora: nenhuma consulta de rede para mostrar comandos.
5. O usuário pode ter os comandos dele (um comando do projeto, um atalho que ele sempre esquece).

## Não objetivos

- Documentação completa de linguagens (referência de API, tutoriais, páginas de manual).
- Executar o comando. O chip só copia; quem roda é o usuário, no terminal dele.
- Busca na internet, Stack Overflow ou IA. Para isso existe o chip `chat`.
- Importar o tldr-pages inteiro nesta fase (ver "Conteúdo" e "Perguntas em aberto").
- Um editor visual de comandos. Os comandos do usuário são arquivos de texto.

## Anti-objetivos (mesmo funcionando, seria falha)

- Qualquer rede, leitura de disco ou trabalho com o painel fechado.
- O painel demorar para abrir ou a RAM do Zefiro subir de forma visível (orçamento em "Custo").
- Resultado errado ou perigoso sem aviso: um comando que apaga dados precisa dizer isso na
  explicação (`docker system prune -a`, `rm -rf`, `git reset --hard`).
- Copiar com uma quebra de linha no fim, que executaria o comando ao colar no terminal.

## O chip

| id | Nome | Na barra | Clique |
| --- | --- | --- | --- |
| `commands` | Comandos | Ícone de terminal com `>_` *(chamada minha)*, largura fixa como os chips de VS Code e Terminal | Abre o painel "Comandos" |

Descrição no catálogo: "Cola de comandos: Docker, Linux, PowerShell, Python e mais". Entra em qualquer
seção de qualquer monitor, como os outros chips (`crates/modules/bar/src/catalog.rs`).

## O painel

Nasce do chip, como o do VS Code e o do Spotify, com largura de cerca de 360 a 400 DIP. As medidas
ficam com quem constrói.

- **Dropdown (topo):** ícone e nome da ferramenta escolhida. Clique abre a lista: Todas, e as
  ferramentas na ordem do pacote. Setas e Enter escolhem; Esc fecha só a lista. Lembra a última
  escolha *(chamada minha, gravada em `[commands] last`)*.
- **Busca (logo abaixo):** o cursor já está nela ao abrir (`WM_CHAR` pelo mesmo caminho do VS Code).
  O texto de apoio é "buscar em <ferramenta>…". Cada palavra digitada precisa aparecer em algum lugar
  do comando, da explicação ou das palavras-chave, no estilo do `matches` do VS Code (`fold`: sem
  acento, sem maiúsculas). Ctrl+V cola, Backspace apaga, Ctrl+Backspace limpa.
- **Resultados (versão A):** lista compacta. Cada linha tem o comando em fonte monoespaçada, a
  explicação embaixo em cinza (uma linha) e "copiar" à direita. Com "Todas" selecionada, a
  ferramenta aparece como selo pequeno na linha *(chamada minha)*.
- **Sem busca digitada:** o rótulo "Mais usados" e os comandos marcados como tal na ferramenta. Em
  "Todas", os "Mais usados" de cada uma, em ordem *(chamada minha)*.
- **Teclado:** ↑↓ escolhem a linha, Enter copia, Esc fecha. Rodapé fixo: "↵ copiar · ↑↓ navegar ·
  Esc fechar".
- **Copiar:** vai para a área de transferência **sem quebra de linha no fim** e o painel **fecha na
  hora** *(chamada minha: o uso é "copiar e colar no terminal"; sem aviso "Copiado", para não criar
  temporizador)*.
- **Sem resultado:** "Nada encontrado em <ferramenta>" e, em ferramenta específica, a dica "Tente em
  Todas".
- **Parâmetros:** os comandos têm trechos a trocar, como `docker rm <contêiner>`. O trecho aparece
  entre `<` e `>` e é copiado como está. O painel não pergunta o valor.

## Conteúdo

### Pacote embutido (curado, em português)

Os comandos vêm de um pacote **escrito para o Zefiro e embutido no executável**, em português do
Brasil, um arquivo TOML por ferramenta (`Docker`, `Linux`, `PowerShell`, `Python`, `Git` no início).
Isso é necessário porque a busca é por **intenção** ("limpar cache", "subir api"): as bases públicas
descrevem os comandos em inglês, e o usuário busca em português.

Formato de uma entrada *(chamada minha)*:

```toml
[tool]
id = "docker"
name = "Docker"

[[command]]
cmd = "docker builder prune"
what = "Apaga o cache de build (as camadas em cache)."
keywords = ["limpar cache", "liberar espaço", "buildkit"]
popular = true
```

- `cmd` é o texto copiado; `what` é a explicação de uma linha; `keywords` são sinônimos buscáveis;
  `popular` coloca em "Mais usados".
- Entram também **receitas** que não são um comando só do manual, como "subir o FastAPI:
  `uvicorn main:app --reload`", dentro da ferramenta Python.

### Comandos do usuário

O usuário põe arquivos `.toml` no mesmo formato em `%APPDATA%\zefiro\commands\`. Eles aparecem junto
do pacote, na mesma ferramenta (mesmo `id`) ou numa ferramenta nova, e **valem sobre** o pacote se o
`cmd` for igual. A pasta é lida ao abrir o painel (um erro numa linha mostra o aviso daquele arquivo
no log, sem derrubar o resto), no estilo dos temas do usuário. O Zefiro nunca grava nessa pasta.

### tldr-pages (fora desta fase)

O [tldr-pages](https://github.com/tldr-pages/tldr) (CC BY 4.0, ~50 idiomas, `pt_BR` parcial) é a
melhor base aberta para ampliar a cobertura. O arquivo para clientes é
`https://github.com/tldr-pages/tldr/releases/latest/download/tldr.zip`, e a licença exige atribuição.
Fica fora desta fase por duas razões: a busca em português não acharia o texto em inglês, e a
atribuição e o download pedem uma decisão do usuário.

## Arquitetura

Segue as regras do projeto: a crate `windows` só aparece em `platform-windows` e `app`.

| Peça | Onde | O que faz |
| --- | --- | --- |
| Dados e busca | `crates/integrations/commands` (nova, portável) | Lê o TOML (embutido e do usuário), `fold`, `matches`, ordem de "Mais usados", ferramentas. Testável no Linux. |
| Painel | `crates/modules/bar/src/commands.rs` | Estado do painel (ferramenta, busca, seleção, rolagem) e `DisplayList`, no padrão de `vscode.rs`. |
| Chip | `crates/modules/bar/src/catalog.rs`, `controller.rs`, `chips.rs` | Entrada no catálogo e abertura do painel. |
| Ligação | `crates/app` | Teclado e clique, copiar para a área de transferência, ler a pasta do usuário ao abrir. |

- **Tema e animação:** como os outros painéis. O painel é desenhado pela barra; nenhum
  `WM_TIMER` redesenha, e a abertura segue a mola do painel (ver as regras de animação no
  compositor).
- **Config:** `[commands] last` (ferramenta escolhida) *(chamada minha)*. Mudança só nessa seção não
  fecha o painel (`Config::same_except_panels`).
- **Serviços:** o chip não tem serviço, thread nem rede. Os dados são lidos ao abrir.

## Custo

- Painel fechado: zero. Nada é lido nem mantido além do pacote embutido (poucas dezenas de KB no
  executável).
- Abrir: parse do TOML embutido uma única vez no processo *(chamada minha)* e leitura da pasta do
  usuário a cada abertura. Meta: sem diferença perceptível no tempo do clique ao quadro
  (`ZEFIRO_CYCLES`).

## Testes

- `integrations/commands`: `fold` com acentos; busca por várias palavras; busca por `keywords`; ordem
  de "Mais usados"; o usuário sobrepõe o pacote; arquivo do usuário com erro não derruba os outros;
  o pacote embutido é válido (`cmd` e `what` não vazios, sem `cmd` com quebra de linha, sem ids
  repetidos).
- `modules/bar`: a lista filtra pela ferramenta; "Todas" mostra o selo; o rodapé; o texto copiado não
  termina em quebra de linha.
- Verificação na tela: sonda ou captura do painel aberto, com a busca "limpar cache" em Docker.

## Chamadas minhas depois da revisão do construtor

Um construtor independente leu esta spec e apontou os pontos abaixo. Nenhum muda o que foi
aprovado; resolvi todos e eles podem ser trocados.

- **Tamanho do pacote:** uma semente de cerca de 40 comandos por ferramenta (5 ferramentas), escrita
  por mim. O usuário amplia pelos arquivos dele. Um teste de lint exige que todo comando que apaga ou
  sobrescreve dados (`prune`, `rm -rf`, `reset --hard`, `-f`, `drop`…) diga isso em `what`.
- **Busca por intenção:** continua "toda palavra precisa casar", mas com duas ajudas: uma lista
  pequena de palavras vazias em português (como, eu, o, a, de, do, da, para, um, uma, com, que, no,
  na) que não contam, e casamento por prefixo de 4 letras ("apago" acha "apaga"). Ordem dos
  resultados: casou em `keywords` > casou no `cmd` > casou só em `what`; empate mantém a ordem do
  arquivo.
- **Ícones:** um glifo genérico (Segoe Fluent) para todas as ferramentas; o dropdown mostra ícone e
  nome. O `[tool]` aceita um campo `icon` opcional com um glifo. Nada de bitmap nem SVG.
- **Fonte monoespaçada:** entra `Family::Mono` em `zefiro_platform::text` (hoje só há Text, Display
  e Icons), com Cascadia Mono e Consolas de reserva, e o suporte correspondente na
  `platform-windows`. É o único trabalho de plataforma da fase.
- **Dropdown aberto:** é uma camada por cima da busca e dos resultados (não empurra o conteúdo, a
  altura do painel não muda). Com ela aberta, ↑↓ movem a lista, Enter escolhe, Esc fecha só a lista,
  e a digitação continua indo para a busca. O estado "lista aberta" é explícito em `commands.rs`.
- **Sem Ctrl+Tab:** tirei o atalho (precisaria ler modificadores). A ferramenta se troca pelo
  dropdown.
- **Mesclagem de arquivos do usuário:** o `cmd` igual só vale dentro da mesma ferramenta (`id`); se
  o `id` repete com outro `name`, vale o do pacote; ferramentas só do usuário vêm depois das
  embutidas, em ordem alfabética; "Mais usados" segue a ordem do arquivo.
- **`[commands] last`:** é o `id` da ferramenta; vazio ou ausente é "Todas". Se a ferramenta não
  existe mais, volta para "Todas". `commands` entra na exclusão de `Config::same_except_panels`.

## Perguntas em aberto

1. **tldr-pages:** entra numa fase 2 como "Mais comandos" (download na primeira vez, atribuição
   CC BY 4.0, em inglês)? Fica a critério do usuário.
2. **Quais ferramentas no início?** Proposta: Docker, Linux, PowerShell, Python (com FastAPI e
   uvicorn), Git. Outras que ele quiser (npm, Rust/Cargo, WSL, SQL…) entram depois.
