import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

const packageJson = JSON.parse(await readFile('package.json', 'utf8'));
const zip = path.join('releases', `linggan-intelligence-browser-v${packageJson.version}.zip`);
const bytes = await readFile(zip);
const manifest = {
  package: packageJson.name,
  version: packageJson.version,
  zip,
  sha256: createHash('sha256').update(bytes).digest('hex'),
};

await writeFile(path.join('releases', 'release-manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`recorded ${manifest.sha256} for ${zip}`);
