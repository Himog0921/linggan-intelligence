import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

function run(command, args) {
  execFileSync(command, args, { cwd: process.cwd(), stdio: 'inherit' });
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

const packageJson = JSON.parse(await readFile('package.json', 'utf8'));
const releaseManifest = JSON.parse(await readFile(path.join('releases', 'release-manifest.json'), 'utf8'));
const committedZip = path.join('releases', `linggan-intelligence-browser-v${packageJson.version}.zip`);
const temporaryDirectory = await mkdtemp(path.join(os.tmpdir(), 'linggan-browser-release-'));
const freshZip = path.join(temporaryDirectory, path.basename(committedZip));

try {
  // This intentionally rebuilds from a clean dependency installation. The
  // generated ZIP goes to a temporary path, so verification never overwrites
  // the committed release artifact it is meant to check.
  run('npm', ['ci', '--ignore-scripts']);
  run('npm', ['run', 'build']);
  run('node', ['scripts/package-release.mjs', '--output', freshZip]);

  const [committedHash, freshHash] = await Promise.all([
    readFile(committedZip).then(sha256),
    readFile(freshZip).then(sha256),
  ]);
  if (releaseManifest.package !== packageJson.name || releaseManifest.version !== packageJson.version || releaseManifest.zip !== committedZip) {
    throw new Error('release manifest identity does not match package.json');
  }
  if (releaseManifest.sha256 !== committedHash) {
    throw new Error(`release manifest SHA-256 does not match committed ZIP: ${committedHash}`);
  }
  if (freshHash !== committedHash) {
    throw new Error(`fresh npm ci/build/package SHA-256 differs: expected ${committedHash}, got ${freshHash}`);
  }
  console.log(`release reproducibility verified: ${committedHash}`);
} finally {
  await rm(temporaryDirectory, { recursive: true, force: true });
}
