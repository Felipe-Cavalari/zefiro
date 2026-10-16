# Passagem da Fase 1b — modo de edição da barra

## Estado da entrega

Implementação na branch **`codex/phase-1b-bar-editor`**, no checkout `C:\Users\felip\Projects\zefiro`. Base: `c224d3c`. A spec e o plano foram aprovados pelo usuário nesta conversa; os testes de mouse/teclado/reinício da barra também foram autorizados.

O código e os checks descritos abaixo estão entregues. A aceitação completa de ambiente ainda depende dos testes manuais e da captura elevada do PresentMon listados no fim. Não iniciar a 1c supondo que esses testes já foram feitos.

- [Spec](../specs/2026-10-15-fase-1b-modo-de-edicao-design.md)
- [Plano](../plans/2026-10-15-fase-1b-modo-de-edicao.md)
- [ADR-0008](../../adr/0008-modo-de-edicao-da-barra.md)
- Ledger local: `.superpowers/sdd/2026-10-15-fase-1b-modo-de-edicao/progress.md`.

## O que funciona

**Editar barra** no menu, inclusive pelo botão direito na faixa vazia. Uma sessão oferece prévia real, seleção de monitor, layout compartilhado/próprio, três seções, galeria local, remoção, movimento por mouse ou teclado, opções globais do relógio, Cancelar e Concluir.

O arquivo da 1a permanece compatível. A edição preserva comentários, campos desconhecidos e layouts de monitores desconectados. IDs desconhecidos aparecem como indisponíveis, sem virar relógio. As opções de layout do monitor usam um caminho de dispositivo, não um índice ou nome `DISPLAY1/2`.

O editor detecta mudanças externas, permite recarregar ou reaplicar seus campos e pede escolha quando o mesmo campo mudou dos dois lados. Arquivo inválido e falhas de gravação mantêm o rascunho; a gravação roda em worker e substitui um temporário completo.

A ilha C++ continua fixa no centro do primário. Os itens comuns do centro ficam ocultos ali enquanto `[island] enabled = true`, preservados no arquivo e visíveis nos monitores sem ilha. A reserva visual é publicada pelo fork; atualizar também o exe da ilha ao testar a barra nova.

## Verificação executada

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pwsh -NoProfile -File scripts/check-windows-imports.ps1
cargo check --target x86_64-unknown-linux-gnu -p zefiro-core -p zefiro-platform -p zefiro-resources -p zefiro-ui -p zefiro-integration-clock -p zefiro-bar -p zefiro-island-host
cargo build --release -p zefiro-app
pwsh -NoProfile -File island/build.ps1
```

**Resultado:** 141 testes passaram; Clippy, imports, check Linux e builds passaram. O sandbox impedia algumas substituições de temporários, o acesso ao cursor, o Clippy e o gerenciador de PDB do MSVC; esses comandos passaram quando executados com a permissão apropriada. Isso não foi tratado como falha funcional do produto.

Sondas versionadas:

- `scripts/probes/display-contract.ps1 -Verify`: duas telas reais com IDs distintos.
- `cargo run -p zefiro-platform-windows --example empty_bar_input -- --without-hook`: falhou como esperado; direito chegou ao outro processo e não recuperou o menu.
- O mesmo exemplo com o hook: passou; esquerdo chegou ao outro processo, direito recuperou o menu.
- `cargo run -p zefiro-platform-windows --example island_geometry`: passou com a ilha compilada; reserva publicada de 880 px a 100%.
- `scripts/probes/editor-ui.ps1`: perfil de teste, abertura, captura, cliques com verificação de dono, teclado com verificação de primeiro plano, arraste, ciclos e notificação de display.

Verificado na tela e nos arquivos isolados:

1. Galeria adicionando Zefiro e relógio a uma barra vazia; relógio colocado no centro.
2. Arraste centro → fim e persistência da seção correta.
3. Movimento por Espaço/setas/Enter; Esc cancelando só o movimento e preservando a posição original ao concluir.
4. Campo EDIT nativo visível, texto e segundos; Concluir gravou e Cancelar preservou o arquivo anterior.
5. Configuração alterada externamente: aviso, confirmação bloqueada, reaplicação e preservação de tema/comentário externos.
6. Recuperação com todas as seções vazias no formato pílulas.
7. Oito combinações de alturas/formatos × Obsidian/Porcelain inspecionadas. A galeria encontrou o fundo sobrepondo chips após troca para pílulas; corrigido com regressão e nova conferência visual.
8. Rascunho preservado após três notificações `WM_DISPLAYCHANGE`, uma recriação e gravação correta ao concluir. Sem alterar a topologia física.
9. Cinquenta ciclos em release, com **8 → 8 janelas do Zefiro**. As janelas IME do Windows são discriminadas pela sonda.

## Medições

Barra release, dois monitores a 100%, Obsidian/faixa completa, ilha e YASB desligados:

| Métrica | Resultado |
|---|---|
| Comando → editor pronto para composição, 50 aberturas | mediana **4,74 ms**, p95 **5,37 ms**, máximo **5,50 ms** |
| Primeiro quadro, duas aberturas release | **36 e 38 ms** |
| Movimento de arraste em release | p95 **2,63 ms**, amostra curta de um gesto real |
| Memória privada após ciclos e 5 s de espera | **14,87 MB**, mediana de 13 amostras de 1 s |
| Working set no mesmo teste | **41,54 MB** |
| CPU média em repouso | **0,0%** |
| Memória privada imediatamente ao fim dos ciclos, antes da limpeza ociosa | **17,98 MB**; não é o repouso nem uma medição do pico máximo |

Os tempos de abertura são de preparação para composição, não de apresentação física. O pico máximo durante todos os gestos/opções não foi amostrado continuamente. O orçamento global de 35 MB do PLANNING inclui widgets/ilha e não é declarado cumprido pela 1b.

Artefatos ignorados pelo Git: `perf-final.json`, `open-final.json`, `runtime/editor-cycles.json`, `final-tests.log`, capturas e sondas auxiliares no ledger. A galeria inicial registra também a regressão antes da correção de camadas; `editor-layer-fixed.png` é a evidência visual da correção.

## Rulings e ajustes registrados

1. **Concorrência otimista aprovada pelo usuário:** comparação de revisão imediatamente antes de substituir; pequena corrida com escritor externo não cooperante permanece possível. Não há promessa de exclusão absoluta. A spec foi atualizada para essa decisão.
2. **Reserva conservadora da ilha:** usar o limite comprovado do palco (880 × escala), não tentar adivinhar largura da pílula ou consultar geometria por quadro. Sem publicação, reservar toda a largura útil e oferecer recuperação/excedentes.
3. **Entrada de texto nativa em host próprio:** o EDIT filho da janela sem bitmap de redirecionamento ficava invisível. Host GDI pequeno resolve isso sem WebView; testes cobrem Unicode, notificações e fechamento.
4. **Recursos ociosos:** grupos criados sob demanda, superfície grande liberada após fechar, limpeza Direct2D/DXGI adiada para 1 s sem painel/editor. Não redefinir memória de repouso como memória durante interação.
5. **Menu → editor mantém foco:** a restauração intermediária ao app anterior custava cerca de 9 ms; essa transição é continuação da edição no mesmo app.
6. **Ordem de execução:** configuração e testes portáveis avançaram enquanto os contratos de plataforma eram sondados; nenhum layout da ilha foi considerado validado por largura estimada.

## Minor adiados

- O campo nativo de formato usa a fonte padrão de UI do Windows, sem aplicar integralmente família/escala de fonte do tema.
- A representação arrastada é recortada pela janela da barra se o ponteiro sair verticalmente; a soltura fora cancela corretamente.

## Validações de ambiente pendentes

- PresentMon elevado: a ferramenta existe, mas recusou iniciar ETW por falta de privilégio. Quadros perdidos **não foram medidos**.
- Desconectar/reconectar monitor, Win+P e dois monitores do mesmo modelo fisicamente conectados.
- DPI 125%/150% na sessão real; os cálculos dessas escalas estão cobertos por testes portáveis.
- Reiniciar Explorer de verdade, bloquear/desbloquear e desligar animações/transparência nas preferências do Windows.
- Amostrar pico máximo de memória em uma sessão longa de edição e ampliar a medição curta do gesto.

## Executáveis e estado da máquina

`target/release/zefiro.exe` e `target/release/zefiro-island.exe` estão prontos no checkout desta branch. Para usar a versão nova, encerrar a barra anterior normalmente antes de iniciar esse exe. O processo de instância única impede ter as duas simultaneamente.

Ao terminar os testes, a barra anterior foi **reaberta do worktree da 1a**, junto com sua ilha. A configuração pessoal e `codex-backup` foram preservados. O YASB não estava em execução antes das medições. Alterações preexistentes de README/assets/scripts de logo não pertencem aos commits desta fase.
