import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import JSZip from 'jszip';

function readJson(file) {
  return JSON.parse(readFileSync(file, 'utf8'));
}

function readArg(name, fallback = '') {
  const index = process.argv.indexOf(name);
  if (index === -1) return fallback;
  return process.argv[index + 1] || fallback;
}

function assertCondition(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function assertIncludesAll(values = [], required = [], label = '') {
  for (const value of required) {
    assertCondition(
      values.includes(value),
      `${label} is missing ${value}`,
    );
  }
}

function assertPlatformManifestContract(manifest = {}, label = 'manifest') {
  const xhsContentScript = (manifest.content_scripts || []).find((entry) =>
    (entry.matches || []).some((value) => value === 'https://xiaohongshu.com/*' || value === 'https://*.xiaohongshu.com/*')
  );
  assertCondition(xhsContentScript, `${label} is missing XHS content script`);
  assertIncludesAll(
    xhsContentScript.js || [],
    ['vendor.js', 'content.js'],
    `${label} XHS content script js`,
  );
  assertIncludesAll(
    xhsContentScript.css || [],
    ['content.css'],
    `${label} XHS content script css`,
  );
  assertCondition(
    !(manifest.content_scripts || []).some((entry) => (entry.matches || []).includes('https://www.douyin.com/*')),
    `${label} must not activate a Douyin content script in this XHS-only release`,
  );
  for (const forbiddenHost of ['douyin', 'xhscdn', 'xiaohongshu.com/*']) {
    assertCondition(
      !(manifest.host_permissions || []).some((value) => value.includes(forbiddenHost)
        && value !== 'https://xiaohongshu.com/*'
        && value !== 'https://www.xiaohongshu.com/*'
        && value !== 'https://*.xiaohongshu.com/*'),
      `${label} retains an unapproved host permission containing ${forbiddenHost}`,
    );
  }
}

const packageJson = readJson('package.json');
const expectedVersion = readArg('--version', packageJson.version);
const zipPath = readArg('--zip', path.join('releases', `linggan-intelligence-browser-v${expectedVersion}.zip`));
const releaseManifest = readJson(path.join('releases', 'release-manifest.json'));

const manifestJson = readJson('manifest.json');
const packageLockJson = readJson('package-lock.json');
const distManifestPath = path.join('dist', 'manifest.json');

assertCondition(packageJson.version === expectedVersion, `package.json version is ${packageJson.version}, expected ${expectedVersion}`);
assertCondition(packageLockJson.version === expectedVersion, `package-lock.json version is ${packageLockJson.version}, expected ${expectedVersion}`);
assertCondition(packageLockJson.packages?.['']?.version === expectedVersion, `package-lock root package version is ${packageLockJson.packages?.['']?.version}, expected ${expectedVersion}`);
assertCondition(manifestJson.version === expectedVersion, `manifest.json version is ${manifestJson.version}, expected ${expectedVersion}`);
assertPlatformManifestContract(manifestJson, 'manifest.json');
assertCondition(existsSync(distManifestPath), 'dist/manifest.json is missing; run npm run build first');

const distManifestJson = readJson(distManifestPath);
assertCondition(distManifestJson.version === expectedVersion, `dist/manifest.json version is ${distManifestJson.version}, expected ${expectedVersion}`);
assertPlatformManifestContract(distManifestJson, 'dist/manifest.json');

const requiredDistFiles = [
  'manifest.json',
  'background.js',
  'content.js',
  'content.css',
  'vendor.js',
  'popup.js',
  'dashboard.js',
  'popup.html',
  'dashboard.html',
];

for (const file of requiredDistFiles) {
  assertCondition(existsSync(path.join('dist', file)), `dist/${file} is missing`);
}

assertCondition(existsSync(zipPath), `${zipPath} is missing`);
assertCondition(releaseManifest.package === packageJson.name, 'release manifest package does not match package.json');
assertCondition(releaseManifest.version === expectedVersion, 'release manifest version does not match package.json');
assertCondition(releaseManifest.zip === zipPath, 'release manifest ZIP path does not match verified ZIP');
assertCondition(
  releaseManifest.sha256 === createHash('sha256').update(readFileSync(zipPath)).digest('hex'),
  'release manifest SHA-256 does not match the committed ZIP',
);

const zip = await JSZip.loadAsync(readFileSync(zipPath));
for (const file of requiredDistFiles) {
  const zippedFile = zip.file(file);
  assertCondition(zippedFile, `${zipPath} is missing ${file}`);

  const distBytes = readFileSync(path.join('dist', file));
  const zippedBytes = await zippedFile.async('nodebuffer');
  assertCondition(
    Buffer.compare(distBytes, zippedBytes) === 0,
    `${zipPath} ${file} does not match dist/${file}; rebuild the release zip after npm run build`,
  );
}

const zipManifest = JSON.parse(await zip.file('manifest.json').async('string'));
assertCondition(zipManifest.version === expectedVersion, `${zipPath} manifest version is ${zipManifest.version}, expected ${expectedVersion}`);
assertPlatformManifestContract(zipManifest, `${zipPath} manifest.json`);

console.log(`release package verified: v${expectedVersion} -> ${zipPath}`);
