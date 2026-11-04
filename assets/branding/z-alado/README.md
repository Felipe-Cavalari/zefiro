# Zefiro — Z + asas

Evolução das propostas iniciais, combinando o monograma Z com a asa do oeste. A cor foi mantida igual nas quatro opções para facilitar a comparação das formas.

1. **Z pluma:** as penas fazem parte da barra superior da letra. Símbolo compacto, com uma silhueta contínua.
2. **Z com asa:** mantém o Z explícito e acrescenta duas penas à esquerda.
3. **Z em voo:** curvas e pontas alongadas fundem a letra ao movimento de uma asa.
4. **Z de duas asas:** interpretação simétrica, próxima de um emblema mitológico.

Cada pasta contém SVG, PNG transparente e ICO nas versões colorida, escura e branca. Os ICOs incluem 16, 20, 24, 32, 40, 48, 64, 128 e 256 px. As versões monocromáticas servem como base para o system tray; a integração no Windows ainda não foi feita.

Regenerar: `node scripts/generate-logo-concepts.cjs --winged` (requer `sharp`). A geometria está em `scripts/logo-winged-concepts.json`. As propostas anteriores foram preservadas em `assets/branding/concepts`.
