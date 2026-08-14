import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

test('release script keeps version files and ignored artifacts aligned', () => {
  const script = readFileSync(new URL('../scripts/version.sh', import.meta.url), 'utf8');

  assert.match(
    script,
    /package-lock\.json/,
    'release script must update package-lock.json when bumping package.json version',
  );

  assert.doesNotMatch(
    script,
    /git add[^\n]*\bdist\//,
    'release script must not git add dist/ because dist is ignored and only used for local packaging',
  );

  assert.match(
    script,
    /mkdir -p releases/,
    'release script must create releases/ before writing the local zip package',
  );
});

test('release package verifier rejects stale zip contents', () => {
  const verifier = readFileSync(new URL('../scripts/verify-release-package.mjs', import.meta.url), 'utf8');

  assert.match(
    verifier,
    /Buffer\.compare\(distBytes,\s*zippedBytes\)/,
    'release verifier must compare zip files with current dist files',
  );
  assert.match(
    verifier,
    /injected\/douyinApiCapture\.js/,
    'release verifier must include douyin injected capture script in the package contract',
  );
  assert.match(
    verifier,
    /does not match dist\//,
    'release verifier should explain stale release zips clearly',
  );
});
