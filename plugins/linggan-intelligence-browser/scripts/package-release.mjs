import { readdir, readFile, writeFile, mkdir } from 'node:fs/promises';
import path from 'node:path';
import JSZip from 'jszip';

const ZIP_EPOCH = new Date('2026-08-25T00:00:00.000Z');

function readArg(name, fallback = '') {
  const index = process.argv.indexOf(name);
  return index === -1 ? fallback : (process.argv[index + 1] || fallback);
}

async function addDirectory(zip, root, relative = '') {
  const entries = (await readdir(path.join(root, relative), { withFileTypes: true }))
    .sort((left, right) => left.name.localeCompare(right.name));
  for (const entry of entries) {
    const entryRelative = path.join(relative, entry.name);
    if (entry.isDirectory()) {
      await addDirectory(zip, root, entryRelative);
    } else if (entry.isFile()) {
      zip.file(entryRelative, await readFile(path.join(root, entryRelative)), {
        date: ZIP_EPOCH,
        createFolders: false,
      });
    }
  }
}

const packageJson = JSON.parse(await readFile('package.json', 'utf8'));
const defaultOutput = path.join('releases', `linggan-intelligence-browser-v${packageJson.version}.zip`);
const output = path.resolve(readArg('--output', defaultOutput));
const zip = new JSZip();
await addDirectory(zip, 'dist');
await mkdir(path.dirname(output), { recursive: true });
await writeFile(output, await zip.generateAsync({
  type: 'nodebuffer',
  compression: 'DEFLATE',
  platform: 'UNIX',
}));
console.log(`created ${path.relative(process.cwd(), output) || output}`);
