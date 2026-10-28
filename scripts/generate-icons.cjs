// Run with Node.js and sharp available locally or through NODE_PATH.
const fs = require('node:fs');
const path = require('node:path');
const sharp = require('sharp');

const root = path.resolve(__dirname, '..');
const master = fs.readFileSync(path.join(root, 'assets/branding/logo.svg'), 'utf8');
const out = path.join(root, 'assets/icons');
const sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256];

async function generate(name, source) {
  fs.writeFileSync(path.join(out, `${name}.svg`), source);
  await sharp(Buffer.from(source)).resize(256, 256).png().toFile(path.join(out, `${name}.png`));
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
  fs.writeFileSync(path.join(out, `${name}.ico`), Buffer.concat([header, ...frames]));
}

async function main() {
  fs.mkdirSync(out, { recursive: true });
  await generate('app', master);
  await generate('tray-dark', master.replaceAll('#087F85', '#18242D'));
  await generate('tray-light', master.replaceAll('#087F85', '#FFFFFF'));
  console.log('Generated official Zefiro icons from assets/branding/logo.svg');
}
main().catch(error => { console.error(error); process.exitCode = 1; });
