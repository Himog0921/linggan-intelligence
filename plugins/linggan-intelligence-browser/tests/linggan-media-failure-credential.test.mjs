import assert from 'node:assert/strict';
import test from 'node:test';
import { recordMediaDownloadFailure } from '../src/linggan/mediaTransferRuntime.js';

// MEDIA-FAILURE-CREDENTIAL-001。
//
// 服务端 `record_media_download_failure_route` 有一支是：拿到了 workRef 与 claimGeneration、
// 却没拿到 installKey 或凭据 → `installation_credential_required` → 401。
// 插件此前调用 `recordFailure(upload, error)` 只传两个参数，第四个 `installationCredential`
// 一直是默认空串，于是每次都精确落进那一支。后果不是噪音：下载尝试虽然记下了，但
// **这张工单失败这件事从来没有被服务端登记过**，401 又被上层 `.catch` 吞掉。

function uploadWithWork() {
  return {
    candidateUris: ['https://sns-img.xhscdn.com/a.jpg'],
    mediaObservationRef: '11111111-1111-4111-8111-111111111111',
    serverWorkRef: '22222222-2222-4222-8222-222222222222',
    claimGeneration: 7,
    installKey: 'install-abc',
  };
}

function captureFetch(seen) {
  return async (url, init) => {
    seen.push({ url, body: JSON.parse(init.body) });
    return { ok: true, json: async () => ({ delivery: 'acknowledged' }) };
  };
}

test('带着工单引用上报时，凭据必须一起送出去', async () => {
  const seen = [];
  await recordMediaDownloadFailure(uploadWithWork(), new Error('network_error'), captureFetch(seen), 'cred-xyz');
  assert.equal(seen.length, 1);
  assert.equal(seen[0].body.workRef, '22222222-2222-4222-8222-222222222222');
  assert.equal(seen[0].body.installKey, 'install-abc');
  assert.equal(seen[0].body.installationCredential, 'cred-xyz');
});

// 这一条复现的就是修复前的现场：凭据缺席 → 服务端那一支只会回 401。
test('缺凭据时送出的正是服务端会判 401 的那种请求', async () => {
  const seen = [];
  await recordMediaDownloadFailure(uploadWithWork(), new Error('network_error'), captureFetch(seen), '');
  const body = seen[0].body;
  assert.ok(body.workRef && body.claimGeneration && body.installKey);
  assert.equal(body.installationCredential, undefined);
});

test('服务端拒绝时抛出它给的原因码，不静默成功', async () => {
  const failing = async () => ({
    ok: false,
    json: async () => ({ code: 'installation_credential_required' }),
  });
  await assert.rejects(
    () => recordMediaDownloadFailure(uploadWithWork(), new Error('boom'), failing, ''),
    /installation_credential_required/,
  );
});

// 上面三条测的是 `recordMediaDownloadFailure` 本身——它一直是对的，它接受凭据。
// **出 bug 的是调用点**：background.js 里原来写的是 `recordFailure: recordMediaDownloadFailure`，
// 少传两个参数，于是凭据永远是默认空串。这一条钉住的就是那一行，不是函数本身。
test('background 把凭据真的绑进了失败上报的调用点', async () => {
  const { readFile } = await import('node:fs/promises');
  const source = await readFile(new URL('../src/linggan/background.js', import.meta.url), 'utf8');
  const binding = source.slice(
    source.indexOf('recordFailure:'),
    source.indexOf('scheduleRecovery:', source.indexOf('recordFailure:')),
  );
  assert.ok(
    binding.includes('installationCredential'),
    '失败上报的调用点必须把 installationCredential 传下去；\n实际绑定：\n' + binding,
  );
  assert.ok(
    !/recordFailure:\s*recordMediaDownloadFailure\s*,/.test(source),
    '不能退回「直接把函数名当回调」的写法——那样第四个参数会是默认空串',
  );
});
