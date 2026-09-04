export const INSTALLATION_CREDENTIAL_KEY =
  'linggan.localTrusted.installationCredential.v1';

function normalizeCredential(value) {
  const credentialRef = String(value?.credentialRef || '').trim();
  const credential = String(value?.credential || '').trim();
  return credentialRef && credential ? { credentialRef, credential } : null;
}

function normalizeActiveCredential(value) {
  const credential = String(value?.credential || '').trim();
  return credential
    ? { credentialRef: String(value?.credentialRef || '').trim(), credential }
    : null;
}

export async function readInstallationCredentialRecord(storageArea, installKey) {
  const stored = await storageArea.get(INSTALLATION_CREDENTIAL_KEY);
  const record = stored?.[INSTALLATION_CREDENTIAL_KEY];
  if (String(record?.installKey || '') !== String(installKey || '')) {
    if (record && typeof storageArea.remove === 'function') {
      await storageArea.remove(INSTALLATION_CREDENTIAL_KEY);
    }
    return null;
  }
  // Read the old single-secret shape only as an active local credential so an update does not
  // strand a producer. New writes always use the explicit active/pending representation.
  const legacy = String(record?.credential || '').trim();
  return {
    installKey: String(installKey || ''),
    installationRef: String(record?.installationRef || '').trim(),
    active: normalizeActiveCredential(record?.active)
      || (legacy ? { credentialRef: '', credential: legacy } : null),
    pending: normalizeCredential(record?.pending),
  };
}

export async function readInstallationCredential(storageArea, installKey) {
  const record = await readInstallationCredentialRecord(storageArea, installKey);
  return String(record?.active?.credential || '').trim();
}

export async function persistInstallationCredential(
  storageArea,
  installKey,
  rawCredential,
) {
  const credential = String(rawCredential || '').trim();
  if (!credential) return readInstallationCredential(storageArea, installKey);
  await storageArea.set({
    [INSTALLATION_CREDENTIAL_KEY]: {
      installKey: String(installKey),
      installationRef: '',
      active: { credentialRef: '', credential },
      pending: null,
    },
  });
  return credential;
}

export async function persistPendingInstallationCredential(
  storageArea,
  { installKey, installationRef, credentialRef, credential },
) {
  const nextPending = normalizeCredential({ credentialRef, credential });
  if (!nextPending || !String(installationRef || '').trim()) return false;
  const current = await readInstallationCredentialRecord(storageArea, installKey);
  await storageArea.set({
    [INSTALLATION_CREDENTIAL_KEY]: {
      installKey: String(installKey),
      installationRef: String(installationRef),
      active: current?.active || null,
      pending: nextPending,
    },
  });
  return true;
}

export async function activatePendingInstallationCredential(
  storageArea,
  installKey,
  credentialRef,
) {
  const current = await readInstallationCredentialRecord(storageArea, installKey);
  if (!current?.pending || current.pending.credentialRef !== String(credentialRef || '')) {
    return false;
  }
  await storageArea.set({
    [INSTALLATION_CREDENTIAL_KEY]: {
      ...current,
      active: current.pending,
      pending: null,
    },
  });
  return true;
}
