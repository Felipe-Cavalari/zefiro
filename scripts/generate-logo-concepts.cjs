// Run with Node.js and the sharp package available through NODE_PATH.
const fs = require('node:fs');
const path = require('node:path');
const sharp = require('sharp');

const winged = process.argv.includes('--winged');
const refined = process.argv.includes('--refined');
const out = path.resolve(__dirname, refined ? '../assets/branding/zefiro-refinado' : winged ? '../assets/branding/z-alado' : '../assets/branding/concepts');
const concepts = refined ? [{
  id: 'zefiro', name: 'Zefiro', color: '#087F85', light: '#C7F4E8',
  shape: '<path d="M8 20C20 11 39 8 56 12C53 20 45 27 35 34L23 43C35 41 46 42 56 46C43 54 24 56 8 52C11 43 19 36 29 29L41 21C29 25 17 25 8 20Z"/>',
}] : winged ? require('./logo-winged-concepts.json') : [
  {
    id: '01-z-corrente', name: 'Z corrente', tag: 'A inicial desenhada pelo vento.',
    detail: 'Direto, ágil e reconhecível.', color: '#087F85', light: '#B8F0E4',
    shape: '<path d="M13 13H52C52 22 45 27 36 33L23 42H52L47 51H10C10 42 17 37 26 31L39 22H8Z"/>',
  },
  {
    id: '02-asa-oeste', name: 'Asa do oeste', tag: 'Uma asa levada pela brisa.',
    detail: 'Leveza, liberdade e movimento.', color: '#4660C7', light: '#CCD7FF',
    shape: '<path d="M8 41C18 18 33 10 57 12C48 18 40 23 29 26C22 28 15 34 8 41Z M10 46C23 32 37 28 52 28C44 36 35 40 24 41C18 42 14 44 10 46Z M13 52C24 45 34 44 43 44C36 52 26 55 13 52Z"/>',
  },
  {
    id: '03-sopro', name: 'Sopro contínuo', tag: 'O vento como um gesto único.',
    detail: 'Fluidez e energia em circulação.', color: '#B45D2B', light: '#FFD5AE',
    shape: '<path d="M9 25H38C45 25 49 21 49 16C49 11 45 8 41 8C37 8 34 10 33 13 M9 37H43C51 37 56 42 56 48C56 54 51 57 46 56 M9 49H27" fill="none" stroke="PAINT" stroke-width="7" stroke-linecap="round" stroke-linejoin="round"/>',
  },
  {
    id: '04-elmo-alado', name: 'Elmo alado', tag: 'Uma presença mitológica.',
    detail: 'Zéfiro como guardião do desktop.', color: '#7355AA', light: '#E5D0FF',
    shape: '<path d="M9 12C22 12 31 17 35 26C22 25 13 21 9 12Z M8 25C18 25 26 28 31 34C20 35 12 32 8 25Z M38 14C48 14 55 21 55 31V39L49 43V53H37V42H29V31C29 23 32 17 38 14Z"/>',
  },
];

function mark(c, paint) {
  return `<g fill="${paint}">${c.shape.replaceAll('PAINT', paint)}</g>`;
}
function svg(c, paint) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 64 64"><title>Zefiro — ${c.name}</title>${mark(c, paint)}</svg>\n`;
}
function mini(c, x, y, size, paint) {
  return `<g transform="translate(${x} ${y}) scale(${size / 64})">${mark(c, paint)}</g>`;
}

async function main() {
  fs.mkdirSync(out, { recursive: true });
  for (const c of concepts) {
    const dir = path.join(out, c.id);
    fs.mkdirSync(dir, { recursive: true });
    for (const [variant, paint] of [['color', c.color], ['tray-dark', '#18242D'], ['tray-light', '#FFFFFF']]) {
      const source = svg(c, paint);
      fs.writeFileSync(path.join(dir, `${variant}.svg`), source);
      await sharp(Buffer.from(source)).resize(256, 256).png().toFile(path.join(dir, `${variant}.png`));
      // ICO containers accept PNG frames. Each size is rasterized directly from SVG.
      const sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256];
      const frames = [];
      for (const size of sizes) frames.push(await sharp(Buffer.from(source)).resize(size, size).png().toBuffer());
      const header = Buffer.alloc(6 + sizes.length * 16);
      header.writeUInt16LE(1, 2);
      header.writeUInt16LE(sizes.length, 4);
      let offset = header.length;
      frames.forEach((frame, i) => {
        const pos = 6 + i * 16;
        header[pos] = sizes[i] === 256 ? 0 : sizes[i];
        header[pos + 1] = header[pos];
        header.writeUInt16LE(1, pos + 4);
        header.writeUInt16LE(32, pos + 6);
        header.writeUInt32LE(frame.length, pos + 8);
        header.writeUInt32LE(offset, pos + 12);
        offset += frame.length;
      });
      fs.writeFileSync(path.join(dir, `${variant}.ico`), Buffer.concat([header, ...frames]));
    }
  }

  const cards = concepts.map((c, i) => {
    const x = 48 + (i % 2) * 760, y = 196 + Math.floor(i / 2) * 420;
    return `<g transform="translate(${x} ${y})">
      <rect width="728" height="388" rx="24" fill="#FFFFFF"/>
      <text x="28" y="44" class="number">0${i + 1}</text>
      <text x="72" y="45" class="name">${c.name}</text>
      <rect x="28" y="74" width="244" height="244" rx="44" fill="${c.color}"/>
      ${mini(c, 66, 112, 168, c.light)}
      <text x="308" y="98" class="label">MONOCROMÁTICO</text>
      <rect x="308" y="116" width="150" height="100" rx="16" fill="#F0F2F3"/>
      <rect x="474" y="116" width="150" height="100" rx="16" fill="#18242D"/>
      ${mini(c, 351, 134, 64, '#18242D')}${mini(c, 517, 134, 64, '#FFFFFF')}
      <text x="308" y="249" class="label">TAMANHO REAL · 16 / 20 / 24 / 32 PX</text>
      <rect x="308" y="263" width="316" height="55" rx="12" fill="#18242D"/>
      ${[16, 20, 24, 32].map((size, j) => mini(c, 328 + j * 72, 274 + (32 - size) / 2, size, '#FFFFFF')).join('')}
      <text x="28" y="351" class="tag">${c.tag}</text>
      <text x="28" y="375" class="detail">${c.detail}</text>
    </g>`;
  }).join('');
  const sheet = refined ? `<svg xmlns="http://www.w3.org/2000/svg" width="1440" height="960" viewBox="0 0 1440 960">
    <style>text{font-family:Segoe UI,Arial,sans-serif}</style>
    <rect width="1440" height="960" fill="#F0F2EE"/>
    <text x="64" y="63" font-size="13" letter-spacing="4" fill="#697780">ZEFIRO / REFINAMENTO</text>
    <text x="64" y="128" font-size="44" font-weight="600" fill="#18242D">Uma letra. Um movimento.</text>
    <rect x="64" y="180" width="760" height="650" rx="28" fill="#FFFFFF"/>
    ${mini(concepts[0], 224, 224, 440, '#087F85')}
    <text x="444" y="742" text-anchor="middle" font-size="48" letter-spacing="3" font-weight="500" fill="#18242D">zefiro</text>
    <text x="444" y="785" text-anchor="middle" font-size="16" fill="#697780">O Z e a asa na mesma silhueta.</text>
    <rect x="856" y="180" width="520" height="300" rx="28" fill="#18242D"/>
    ${mini(concepts[0], 1020, 210, 192, '#FFFFFF')}
    <text x="1116" y="446" text-anchor="middle" font-size="14" letter-spacing="2" fill="#B6C2C7">MONOCROMÁTICO</text>
    <rect x="856" y="504" width="520" height="326" rx="28" fill="#FFFFFF"/>
    <text x="892" y="551" font-size="14" letter-spacing="2" fill="#697780">SYSTEM TRAY · TAMANHO REAL</text>
    <rect x="892" y="581" width="448" height="80" rx="14" fill="#18242D"/>
    ${[16,20,24,32].map((s,i) => mini(concepts[0], 930+i*104, 605+(32-s)/2, s, '#FFFFFF')).join('')}
    ${[16,20,24,32].map((s,i) => `<text x="${946+i*104}" y="698" text-anchor="middle" font-size="13" fill="#697780">${s} px</text>`).join('')}
    <text x="892" y="756" font-size="17" fill="#18242D">Curvas amplas. Sem penas anexadas.</text>
    <text x="892" y="787" font-size="17" fill="#697780">A asa aparece no gesto da letra.</text>
    <text x="64" y="900" font-size="15" fill="#697780">SVG transparente · Versões clara e escura · ICO de 16 a 256 px</text>
  </svg>` : `<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="1100" viewBox="0 0 1600 1100">
    <style>text{font-family:Segoe UI,Arial,sans-serif}.number{font-size:18px;fill:#7E8A90}.name{font-size:25px;font-weight:650;fill:#18242D}.label{font-size:12px;letter-spacing:1px;fill:#697780}.tag{font-size:19px;fill:#18242D}.detail{font-size:15px;fill:#697780}</style>
    <rect width="1600" height="1100" fill="#EBEEEA"/>
    <text x="48" y="58" font-size="14" letter-spacing="4" fill="#697780">ZEFIRO / ${winged ? 'EVOLUÇÃO DO SÍMBOLO' : 'EXPLORAÇÃO DE IDENTIDADE'}</text>
    <text x="48" y="124" font-size="52" font-weight="650" fill="#18242D">${winged ? 'O Z ganha asas.' : 'Quatro formas de sentir o vento.'}</text>
    <text x="50" y="161" font-size="20" fill="#697780">${winged ? 'Quatro variações da mesma ideia: inicial + vento + asa.' : 'Leveza no desktop. Presença em cada pixel.'}</text>
    ${cards}
    <text x="48" y="1063" font-size="14" fill="#697780">SVG vetorial · Fundo transparente · Variações para temas claro e escuro · ICO com 9 resoluções</text>
  </svg>`;
  fs.writeFileSync(path.join(out, 'comparativo.svg'), sheet);
  await sharp(Buffer.from(sheet)).png().toFile(path.join(out, 'comparativo.png'));
  console.log(`Generated ${concepts.length} logo concept(s) in ${out}`);
}
main().catch(error => { console.error(error); process.exitCode = 1; });
