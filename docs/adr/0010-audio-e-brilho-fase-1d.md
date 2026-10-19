# ADR-0010: Áudio e brilho na barra (fase 1d)

- **Status:** aceito
- **Data:** 2026-10-19
- **Branch:** `main` (commits `8b78f71`, `44acb96`; correções da validação `02384fc`, `6bd881b`, `7848180`)

## Contexto

A 1d pedia os widgets de **alto-falante**, **microfone** e **brilho** do [PLANNING §2](../PLANNING.md#widgets-da-fase-1): porcentagem no chip, roda do mouse ajusta, painel com slider e troca de dispositivo (áudio) ou um slider por monitor (brilho). A ilha embutida (ADR-0009) já tinha o observador de volume da saída e a troca de dispositivo pelo menu; faltavam a entrada, a escrita de volume/mudo e o brilho por DDC/CI, cuja latência o spike S3 mediu em dezenas de ms por chamada.

## Opções consideradas

1. **Brilho síncrono na thread da UI.** Simples, mas cada leitura/escrita trava a barra ~60 ms por monitor e o arraste do slider enfileira dezenas de escritas.
2. **Brilho numa thread própria, interface otimista e escrita agrupada.** A barra mostra o valor pedido na hora; a thread grava só o último valor de cada monitor, uma escrita por vez, com 50 ms de folga entre elas. Escolhida.
3. **WMI `WmiMonitorBrightness`.** Só funciona em tela interna; os dois monitores daqui são externos. Fica para quando houver notebook.

## Números medidos

Ambiente: Windows 11 Pro 26200, dois monitores 1920×1080 a 100 % (27G2WG3 principal, LG ULTRAGEAR), `compacta`/`faixa`, Obsidian, build release, ilha embutida, YASB fechado, 2026-10-19. `scripts/perf.ps1 -Process zefiro -WarmupSeconds 20 -SampleSeconds 60`.

| Métrica | Barra sem os chips (perfil do usuário) | Com som, microfone e brilho, recém-aberta | Com os chips, depois de usar os três painéis |
|---|---|---|---|
| Memória privada | **25,9 MB** | **29,1 MB** | 36,9 MB |
| Working set | 70,3 MB | 72,2 MB | 90,1 MB |
| CPU em repouso | 0,0 % | 0,0 % | 0,0 % |

| Medida | Resultado |
|---|---|
| Leitura DDC/CI (VCP 0x10) por monitor | 54–64 ms |
| Escrita pelo slider (70 %) lida de volta no monitor | confirmada pela sonda `brightness_probe` |

Os três serviços custam cerca de 3 MB privados. Os 7,8 MB a mais depois de usar os painéis não voltaram em 60 s de repouso; a causa não foi investigada (ver Consequências).

## Decisão

Som, microfone e brilho são três chips do catálogo (`audio`, `mic`, `brightness`), cada um com painel próprio que nasce do chip.

### Arquitetura

| Camada | O que tem |
|---|---|
| `crates/integrations/devices` (`zefiro-integration-devices`, portável) | `Coalescer` (só o último valor, uma escrita por vez, intervalo mínimo) e `level` (passo da roda, posição no slider, valor bruto ↔ %). Testado no Linux pelo CI. |
| `crates/modules/bar/src/devices.rs` | Chips de nível (`level_chip`, `volume_level`), ícones e o painel (`device_panel`): título, linhas de slider com mudo opcional, lista de dispositivos e as áreas de clique. |
| `crates/platform-windows` | `brightness.rs`: thread `zefiro-brightness` sobre `dxva2` (reabre os handles em `WM_DISPLAYCHANGE`). `volume.rs`: `VolumeWatcher` para saída **e** entrada padrão, por evento. `audio_devices.rs`: volume e mudo de qualquer fluxo e troca do padrão (`IPolicyConfig`). |
| `crates/app/src/devices_loop.rs` | Serviços, roda nos chips, arraste com captura do mouse, mudo, troca de dispositivo e os avisos de volume e brilho na ilha embutida. |

### Decisões de comportamento

- **Roda:** 2 % por clique no volume e 5 % no brilho, arredondando para a grade (sempre chega exatamente a 0 e 100); meio clique de touchpad ainda anda um passo.
- **Brilho por barra:** o chip mostra e ajusta o monitor da própria barra; o painel lista todos, e um monitor sem DDC/CI aparece como "Indisponível".
- **Otimista:** o slider e a roda mudam o estado na hora; o aviso do sistema confirma depois. Mexer no volume pela barra também mostra o aviso de volume na ilha, e o brilho ganhou aviso próprio na ilha embutida (na C++ ele não funcionava em monitor externo).
- **Lista de dispositivos lida ao abrir o painel** (não a cada quadro do arraste); a troca do padrão, venha de onde vier, relê a lista e o volume do dispositivo novo.
- **Largura estável dos chips de nível** (correção da validação): o chip reserva a largura de "100%" e o microfone mudo mantém o % apagado. O painel aberto acompanha o chip; quando o chip encolhia (mudo, 100 % → 9 %), o painel andava ~14 px sob o cursor e o clique seguinte errava o botão de mudo, deixando o microfone mudo.
- **Mover "na hora" também é animação** (correção da validação): no `move_offset`, `StopAnimation` + `InsertScalar` logo depois de uma mola perdia para a parada, que o compositor aplica depois e que grava por cima o valor do meio do caminho. Na partida, os chips nascem só com o ícone, crescem quando o estado chega e o conteúdo ficava parado fora do chip. Agora é um quadro-chave de 1 ms, que substitui a mola na ordem certa. Vale também para o conteúdo da ilha.

## Validação na tela

Perfil isolado (`ZEFIRO_CONFIG_DIR`) com `end = ["mic", "audio", "brightness", "clock"]`, no monitor principal:

1. Roda no chip de som: 68 → 72 → 68 %.
2. Painel de som: arraste do slider 68 → 50 → 68 %, mudo liga e desliga, troca da saída para "Saída Digital" e de volta para os fones (o painel seguiu o volume do dispositivo novo).
3. Painel do microfone: mudo liga e desliga no mesmo ponto, conferido pela sonda `audio_probe` (antes da correção, o segundo clique errava o botão).
4. Brilho: roda no chip do principal 50 → 55 → 50 %; slider do secundário 51 → 70 → 51 %, com o 70 % lido de volta no monitor.
5. Chips na partida: o conteúdo de cada chip fica dentro do retângulo dele (antes da correção, ficava encavalado no vizinho).

Volume, mudo, saída padrão e brilho voltaram aos valores de antes.

## Consequências

- O roteiro avança para a **1e** (itens declarativos v1).
- **Pendências de validação:** roda no chip de brilho da barra do secundário, troca do microfone padrão pelo painel (mesmo caminho da saída), monitor sem DDC/CI, desconectar/reconectar monitor, DPI 125/150 % e PresentMon elevado.
- **Minor adiados:**
  - Os monitores aparecem como "Generic PnP Monitor"; o nome do EDID (27G2WG3, LG ULTRAGEAR) ainda não é lido.
  - Na partida, a barra roda `apply` sem animação ~16 vezes em 40 ms; é desperdício, não erro.
  - Investigar os ~8 MB privados que ficam depois de usar os painéis de som, microfone e brilho.
- **Plano B do brilho:** se um monitor perder comandos mesmo com a folga de 50 ms, aumentar o intervalo por monitor; para tela interna, WMI.
