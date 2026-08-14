import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, '..');

test('task-state hot paths import TASK_STATE instead of hard-coding UI task states', () => {
  const positiveFiles = [
    'src/content/commentImageTask.js',
    'src/content/douyinBatchMessageHandlers.js',
    'src/content/xhsPageController.js',
    'src/content/components/TaskControlBar.jsx',
    'src/shared/taskUi.js',
  ];

  for (const relativePath of positiveFiles) {
    const source = fs.readFileSync(path.join(projectRoot, relativePath), 'utf8');
    assert.match(
      source,
      /TASK_STATE/,
      `${relativePath} should reference TASK_STATE constants`,
    );
  }

  const noLegacyDoneComparisons = [
    'src/content/commentImageTask.js',
    'src/content/douyinBatchMessageHandlers.js',
    'src/content/xhsPageController.js',
  ];

  for (const relativePath of noLegacyDoneComparisons) {
    const source = fs.readFileSync(path.join(projectRoot, relativePath), 'utf8');
    assert.doesNotMatch(
      source,
      /progress\.status === 'done'|p\.status === 'done'|progress\.taskState === 'done'|progress\.taskState === 'idle'/,
      `${relativePath} should route task-state checks through TASK_STATE helpers`,
    );
  }
});
