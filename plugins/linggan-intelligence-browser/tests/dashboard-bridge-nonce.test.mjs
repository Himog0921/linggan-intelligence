import assert from 'node:assert/strict';
import test from 'node:test';

// DASHBOARD-NONCE-001。
//
// 现场报错两行：
//   [DashboardBridge] Failed to store nonce: Error: Access to storage is not allowed from this context.
//   [DashboardBridge] Rejected message: invalid or missing nonce
//
// 两个独立缺陷叠在一起：
//   1. `chrome.storage.session` 在内容脚本里默认不可读（需 service worker 调 setAccessLevel，
//      本仓库此前从未调用）；
//   2. 双写用了 `Promise.all`，session 那一路必然抛错，把本来能成功的 local 也带崩。
// 结果 nonce 一个字节都没存下去，面板随后每条指令都被自己拒掉。

const originalChrome = globalThis.chrome;
const SESSION_DENIED = 'Access to storage is not allowed from this context.';

function storageSurface({ sessionFails = false, localFails = false } = {}) {
  const writes = { session: [], local: [] };
  return {
    writes,
    chrome: {
      storage: {
        session: {
          set: async (payload) => {
            if (sessionFails) throw new Error(SESSION_DENIED);
            writes.session.push(payload);
          },
          remove: async () => {},
        },
        local: {
          set: async (payload) => {
            if (localFails) throw new Error('local unavailable');
            writes.local.push(payload);
          },
          remove: async () => {},
        },
      },
    },
  };
}

globalThis.chrome = storageSurface().chrome;
const { storeDashboardNonce } = await import('../src/content/dashboardBridge.js');
globalThis.chrome = originalChrome;

async function withStorage(surface, run) {
  globalThis.chrome = surface.chrome;
  const errors = [];
  const originalError = console.error;
  console.error = (...args) => errors.push(args.join(' '));
  try {
    return { result: await run(), errors };
  } finally {
    console.error = originalError;
    globalThis.chrome = originalChrome;
  }
}

// 修复前的现场：session 抛错把整个 Promise.all 带崩，local 一个字节都没写进去。
test('session 被拒时，local 仍然把 nonce 写进去了', async () => {
  const surface = storageSurface({ sessionFails: true });
  const { result, errors } = await withStorage(surface, () => storeDashboardNonce('nonce-1'));
  assert.equal(result, true, 'local 成功就算存住了');
  assert.equal(surface.writes.local.length, 1);
  assert.equal(surface.writes.local[0].dashboardNonce, 'nonce-1');
  assert.deepEqual(errors, [], '还有一路成功时不该报错');
});

test('两路都可用时两路都写', async () => {
  const surface = storageSurface();
  await withStorage(surface, () => storeDashboardNonce('nonce-2'));
  assert.equal(surface.writes.session.length, 1);
  assert.equal(surface.writes.local.length, 1);
});

test('两路都失败才算真失败，并且说出原因', async () => {
  const surface = storageSurface({ sessionFails: true, localFails: true });
  const { result, errors } = await withStorage(surface, () => storeDashboardNonce('nonce-3'));
  assert.equal(result, false);
  assert.equal(errors.length, 1);
  assert.ok(errors[0].includes('Failed to store nonce'));
});

// 源码级回归守卫：不能再退回「一路失败带崩另一路」的写法。
test('nonce 双写不能再用 Promise.all', async () => {
  const { readFile } = await import('node:fs/promises');
  const source = await readFile(new URL('../src/content/dashboardBridge.js', import.meta.url), 'utf8');
  const region = source.slice(0, source.indexOf('export function createDashboardBridge'));
  assert.ok(!/Promise\.all\s*\(/.test(region), 'nonce 写入必须各写各的，不能让一路失败牵连另一路');
  assert.ok(region.includes('allSettled'), 'nonce 写入应使用 allSettled 逐路收敛结果');
});

// 扫源码前必须先剥掉注释：这两个词在上面那段 JSDoc 里也出现，不剥的话
// 「把调用整行删掉、只留注释」也能骗过这条断言——那就是一条永远为真的假绿。
function sourceWithoutComments(source) {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .split('\n')
    .map((line) => line.replace(/(^|[^:])\/\/.*$/, '$1'))
    .join('\n');
}

test('service worker 必须真的调用 setAccessLevel，而不只是注释里提到', async () => {
  const { readFile } = await import('node:fs/promises');
  const raw = await readFile(new URL('../src/linggan/background.js', import.meta.url), 'utf8');
  const code = sourceWithoutComments(raw);
  assert.match(code, /session\.setAccessLevel\(\s*\{/, '必须存在真实的 setAccessLevel 调用');
  assert.match(code, /TRUSTED_AND_UNTRUSTED_CONTEXTS/, '访问级别必须放开到非受信上下文');
});

// 行为验证，不是扫文本：把 setAccessLevel mock 成**同步抛错**，真的去 import 一次
// background.js。这一行跑在模块顶层，同步抛错会让整个模块加载失败——闹钟监听器、
// onInstalled/onStartup、启动补位全部注册不上，插件彻底变砖。
// `Promise.resolve(f())` 接不住 `f()` 的同步异常（参数先求值，那时 promise 还不存在），
// 所以这里必须是 try/catch。
test('放开 session 时同步抛错，不会拖垮整个 background 模块', async () => {
  const surface = storageSurface();
  surface.chrome.storage.session.setAccessLevel = () => {
    throw new Error('模拟 Chrome 策略同步拒绝');
  };
  Object.assign(surface.chrome, {
    runtime: {
      onInstalled: { addListener() {} },
      onStartup: { addListener() {} },
      onMessage: { addListener() {} },
      getManifest: () => ({ version: '0.0.0-test' }),
    },
    alarms: { create: async () => {}, get: async () => null, onAlarm: { addListener() {} } },
    permissions: { contains: async () => false },
    tabs: { query: async () => [], sendMessage: async () => null, create: async () => {} },
    windows: { create: async () => ({}), remove: async () => {} },
  });
  surface.chrome.storage.local.get = async () => ({});

  globalThis.chrome = surface.chrome;
  try {
    await assert.doesNotReject(
      () => import('../src/linggan/background.js?syncthrow=' + Math.random()),
      'setAccessLevel 同步抛错时，background 模块仍必须完成加载',
    );
  } finally {
    globalThis.chrome = originalChrome;
  }
});
