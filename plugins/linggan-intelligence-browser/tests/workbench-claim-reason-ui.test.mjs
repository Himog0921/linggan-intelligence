import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { formatTaskLeaseIdleNotice } from '../src/workbench/runtime/taskLeaseClient.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, '..');

test('idle claim notice helper formats a visible status message', () => {
  assert.deepEqual(
    formatTaskLeaseIdleNotice({
      idleReasonCode: 'no_available_account',
      idleReasonMessage: '没有可用账号',
      nextPollAfterMs: 45000,
    }),
    {
      message: '最近一次不接单原因：没有可用账号（no_available_account），约 45 秒后重试',
      type: 'warning',
      visible: true,
    },
  );
});

test('idle claim notice keeps legacy purpose mismatch wording generic', () => {
  assert.deepEqual(
    formatTaskLeaseIdleNotice({
      idleReasonCode: 'ACCOUNT_PURPOSE_MISMATCH',
      idleReasonMessage: '账号用途与当前任务不匹配',
      nextPollAfterMs: 15000,
    }),
    {
      message: '最近一次不接单原因：账号用途与当前任务不匹配（ACCOUNT_PURPOSE_MISMATCH），约 15 秒后重试',
      type: 'warning',
      visible: true,
    },
  );
});

test('popup app wires idle claim notices into its existing notice area', () => {
  const popupSource = fs.readFileSync(path.join(projectRoot, 'src/popup/App.jsx'), 'utf8');

  assert.match(popupSource, /formatTaskLeaseIdleNotice/);
  assert.match(popupSource, /TASK_LEASE_STORAGE_KEY/);
  assert.match(popupSource, /displayNotice/);
  assert.match(popupSource, /<Notice \{\.\.\.displayNotice\}/);
});

test('dashboard app wires idle claim notices into its existing notice area', () => {
  const dashboardSource = fs.readFileSync(path.join(projectRoot, 'src/dashboard/App.jsx'), 'utf8');

  assert.match(dashboardSource, /formatTaskLeaseIdleNotice/);
  assert.match(dashboardSource, /TASK_LEASE_STORAGE_KEY/);
  assert.match(dashboardSource, /displayNotice/);
  assert.match(dashboardSource, /dashboard-notice \${displayNotice\.type}/);
});
