import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

test('dashboard actions and notices now use busy-state and structured feedback hooks', () => {
  const appSource = fs.readFileSync(path.join(projectRoot, 'src/dashboard/App.jsx'), 'utf8');
  const styleSource = fs.readFileSync(path.join(projectRoot, 'src/dashboard/dashboard.css'), 'utf8');

  assert.match(appSource, /busyActions/);
  assert.match(appSource, /rowBusyActions/);
  assert.match(appSource, /withBusyAction/);
  assert.match(appSource, /dashboard-notice-icon/);
  assert.match(appSource, /dashboard-dialog-detail/);
  assert.match(appSource, /emptyState/);
  assert.match(appSource, /getMediaStatusMeta/);

  assert.match(styleSource, /\.dashboard-notice-icon/);
  assert.match(styleSource, /\.dashboard-notice-copy/);
  assert.match(styleSource, /\.dashboard-dialog-detail/);
  assert.match(styleSource, /\.table-status-pill/);
  assert.match(styleSource, /\.media-status\.tone-success/);
});

test('content toast and injected buttons share the new immediate feedback semantics', () => {
  const toastSource = fs.readFileSync(path.join(projectRoot, 'src/content/components/Toast.jsx'), 'utf8');
  const buttonGroupSource = fs.readFileSync(path.join(projectRoot, 'src/content/components/ButtonGroup.jsx'), 'utf8');
  const feedbackSource = fs.readFileSync(path.join(projectRoot, 'src/shared/feedback.js'), 'utf8');
  const iconSource = fs.readFileSync(path.join(projectRoot, 'src/shared/icons.js'), 'utf8');

  assert.match(toastSource, /getFeedbackMeta/);
  assert.match(toastSource, /meta\.title/);
  assert.match(toastSource, /icon\(meta\.icon/);

  assert.match(buttonGroupSource, /pendingAction/);
  assert.match(buttonGroupSource, /acknowledgeAction/);
  assert.match(buttonGroupSource, /disabled=\{isPending\}/);

  assert.match(feedbackSource, /getFeedbackMeta/);
  assert.match(iconSource, /infoCircle/);
  assert.match(iconSource, /alertTriangle/);
  assert.match(iconSource, /xCircle/);
  assert.match(iconSource, /loader/);
});
