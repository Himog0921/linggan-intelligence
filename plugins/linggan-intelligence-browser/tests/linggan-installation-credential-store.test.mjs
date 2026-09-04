import assert from 'node:assert/strict';
import test from 'node:test';

import {
  INSTALLATION_CREDENTIAL_KEY,
  activatePendingInstallationCredential,
  persistPendingInstallationCredential,
  persistInstallationCredential,
  readInstallationCredential,
  readInstallationCredentialRecord,
} from '../src/linggan/installationCredentialStore.js';

function memoryStorage() {
  const values = {};
  return {
    values,
    async get(key) { return { [key]: values[key] }; },
    async set(next) { Object.assign(values, next); },
    async remove(key) { delete values[key]; },
  };
}

test('first issuance is local-only and heartbeat null preserves the credential', async () => {
  const storage = memoryStorage();
  await persistInstallationCredential(storage, 'install-1', 'credential-secret-1');
  assert.deepEqual(storage.values[INSTALLATION_CREDENTIAL_KEY], {
    installKey: 'install-1',
    installationRef: '',
    active: { credentialRef: '', credential: 'credential-secret-1' },
    pending: null,
  });
  assert.equal(
    await persistInstallationCredential(storage, 'install-1', ''),
    'credential-secret-1',
  );
});

test('pending issuance is retained across a lost activation response and becomes active on replay', async () => {
  const storage = memoryStorage();
  await persistPendingInstallationCredential(storage, {
    installKey: 'install-1',
    installationRef: 'installation-1',
    credentialRef: 'credential-ref-1',
    credential: 'credential-secret-1',
  });
  assert.equal(await readInstallationCredential(storage, 'install-1'), '');
  assert.deepEqual(
    (await readInstallationCredentialRecord(storage, 'install-1')).pending,
    { credentialRef: 'credential-ref-1', credential: 'credential-secret-1' },
  );

  assert.equal(
    await activatePendingInstallationCredential(storage, 'install-1', 'credential-ref-1'),
    true,
  );
  assert.equal(
    await activatePendingInstallationCredential(storage, 'install-1', 'credential-ref-1'),
    false,
  );
  assert.equal(await readInstallationCredential(storage, 'install-1'), 'credential-secret-1');
});

test('rotation pending storage preserves the prior active credential until activation', async () => {
  const storage = memoryStorage();
  await persistInstallationCredential(storage, 'install-1', 'credential-secret-old');
  await persistPendingInstallationCredential(storage, {
    installKey: 'install-1',
    installationRef: 'installation-1',
    credentialRef: 'credential-ref-new',
    credential: 'credential-secret-new',
  });
  assert.equal(await readInstallationCredential(storage, 'install-1'), 'credential-secret-old');
  await activatePendingInstallationCredential(storage, 'install-1', 'credential-ref-new');
  assert.equal(await readInstallationCredential(storage, 'install-1'), 'credential-secret-new');
});

test('rotation replaces the secret and an install-key change clears it', async () => {
  const storage = memoryStorage();
  await persistInstallationCredential(storage, 'install-1', 'credential-secret-1');
  await persistInstallationCredential(storage, 'install-1', 'credential-secret-2');
  assert.equal(await readInstallationCredential(storage, 'install-1'), 'credential-secret-2');
  assert.equal(await readInstallationCredential(storage, 'install-2'), '');
  assert.equal(storage.values[INSTALLATION_CREDENTIAL_KEY], undefined);
});

test('the stored secret is not projected into a status-shaped value', async () => {
  const storage = memoryStorage();
  await persistInstallationCredential(storage, 'install-1', 'credential-secret-1');
  const publicStatus = { registered: true, installationRef: 'installation-1' };
  assert.equal(JSON.stringify(publicStatus).includes('credential-secret-1'), false);
});
