import { readdir, readFile, writeFile, mkdir } from 'node:fs/promises';
import path from 'node:path';
import JSZip from 'jszip';

const ZIP_EPOCH = new Date('2026-08-25T00:00:00.000Z');

async function addDirectory(zip, root, relative = '') {
  const entries = (await readdir(path.join(root, relative), { withFileTypes: true }))
    .sort((left, right) => left.name.localeCompare(right.name));
  for (const entry of entries) {
    const entryRelative = path.join(relative, entry.name);
    if (entry.isDirectory()) {
      await addDirectory(zip, root, entryRelative);
    } else if (entry.isFile()) {
      zip.file(entryRelative, await readFile(path.join(root, entryRelative)), { date: ZIP_EPOCH });
    }
  }
}

const packageJson = JSON.parse(await readFile('package.json', 'utf8'));
const output = path.join('releases', `linggan-intelligence-browser-v${packageJson.version}.zip`);
const zip = new JSZip();
await addDirectory(zip, 'dist');
await mkdir('releases', { recursive: true });
await writeFile(output, await zip.generateAsync({
  type: 'nodebuffer',
  compression: 'DEFLATE',
  platform: 'UNIX',
}));
console.log(`created ${output}`);
