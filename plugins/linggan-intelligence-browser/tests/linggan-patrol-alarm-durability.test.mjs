import assert from 'node:assert/strict';
import test from 'node:test';

// PATROL-ALARM-RECOVERY-001。
//
// 2026-09-13 实测：两台工位各自睡了 2 小时 26 分，期间服务端健康、凭据有效、机器没休眠、
// Chrome 没重启，Service Worker 卡片显示「无效」。唯一的唤醒原语——`linggan-patrol`
// 闹钟——当时并不存在。下面这组断言钉住的就是「闹钟不能再丢」。

const originalChrome = globalThis.chrome;

function alarmSurface({ existing = null, getThrows = false } = {}) {
  const created = [];
  let current = existing;
  return {
    created,
    get current() { return current; },
    chrome: {
      runtime: {
        onInstalled: { addListener() {} },
        onStartup: { addListener() {} },
        onMessage: { addListener() {} },
        getManifest: () => ({ version: '0.0.0-test' }),
      },
      alarms: {
        get: async (name) => {
          if (getThrows) throw new Error('alarms unavailable');
          return current && current.name === name ? current : null;
        },
        create: async (name, schedule) => {
          created.push({ name, schedule });
          current = { name, ...schedule };
        },
        onAlarm: { addListener() {} },
      },
      permissions: { contains: async () => false },
      storage: {
        local: { get: async () => ({}), set: async () => {} },
        session: { setAccessLevel: async () => {} },
      },
      tabs: { query: async () => [], sendMessage: async () => null, create: async () => {} },
      windows: { create: async () => ({}), remove: async () => {} },
    },
  };
}

const bootSurface = alarmSurface();
globalThis.chrome = bootSurface.chrome;
const { ensurePatrolAlarm } = await import('../src/linggan/background.js');
globalThis.chrome = originalChrome;

async function withSurface(surface, run) {
  globalThis.chrome = surface.chrome;
  try {
    return await run();
  } finally {
    globalThis.chrome = originalChrome;
  }
}

test('没有闹钟时一定补一个', async () => {
  const surface = alarmSurface({ existing: null });
  await withSurface(surface, () => ensurePatrolAlarm(300));
  assert.deepEqual(surface.created, [{
    name: 'linggan-patrol',
    schedule: { delayInMinutes: 5, periodInMinutes: 5 },
  }]);
});

// 这一条是本次修复的核心：重建等于「先删再建」，掐在中间就永远醒不过来。节奏没变就别碰它。
test('节奏没变时绝不重建闹钟', async () => {
  const surface = alarmSurface({ existing: { name: 'linggan-patrol', periodInMinutes: 5 } });
  const result = await withSurface(surface, () => ensurePatrolAlarm(300));
  assert.equal(surface.created.length, 0);
  assert.equal(result.created, false);
});

test('节奏真的变了才写入新节奏', async () => {
  const surface = alarmSurface({ existing: { name: 'linggan-patrol', periodInMinutes: 5 } });
  await withSurface(surface, () => ensurePatrolAlarm(60));
  assert.deepEqual(surface.created, [{
    name: 'linggan-patrol',
    schedule: { delayInMinutes: 1, periodInMinutes: 1 },
  }]);
});

// worker 刚醒时只想确认「还有人能叫醒下一次」，不该把服务端刚给的 5 分钟改回 1 分钟。
test('兜底补位不会覆盖服务端已经给出的节奏', async () => {
  const surface = alarmSurface({ existing: { name: 'linggan-patrol', periodInMinutes: 5 } });
  const result = await withSurface(surface, () => ensurePatrolAlarm(60, { onlyIfMissing: true }));
  assert.equal(surface.created.length, 0);
  assert.equal(result.created, false);
});

test('兜底补位在闹钟真的不在时仍然会建', async () => {
  const surface = alarmSurface({ existing: null });
  await withSurface(surface, () => ensurePatrolAlarm(60, { onlyIfMissing: true }));
  assert.deepEqual(surface.created, [{
    name: 'linggan-patrol',
    schedule: { delayInMinutes: 1, periodInMinutes: 1 },
  }]);
});

// 读不到闹钟状态与「没有闹钟」必须走同一个结果：多建一个的代价，远小于永远醒不过来。
test('查不到闹钟状态时按没有处理', async () => {
  const surface = alarmSurface({ existing: { name: 'linggan-patrol', periodInMinutes: 5 }, getThrows: true });
  await withSurface(surface, () => ensurePatrolAlarm(300));
  assert.equal(surface.created.length, 1);
});

test('没有闹钟 API 时不抛错，只是什么都不做', async () => {
  const surface = alarmSurface();
  delete surface.chrome.alarms.create;
  const result = await withSurface(surface, () => ensurePatrolAlarm(300));
  assert.equal(result, null);
});
