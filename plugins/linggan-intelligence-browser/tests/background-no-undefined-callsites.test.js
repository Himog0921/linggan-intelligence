import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const backgroundSource = readFileSync(new URL('../src/background/index.js', import.meta.url), 'utf8');
const taskPollerSource = readFileSync(new URL('../src/workbench/runtime/taskPoller.js', import.meta.url), 'utf8');

test('background lifecycle persistence callsites have definitions', () => {
  assert.match(
    backgroundSource,
    /async function persistNavigatedTaskTabsSnapshot\(\)/,
    'background must define persistNavigatedTaskTabsSnapshot before calling it',
  );
  assert.match(
    backgroundSource,
    /taskPoller\.persistActiveTaskContext\(\)/,
    'background must call persistActiveTaskContext through the taskPoller instance',
  );
  assert.match(
    taskPollerSource,
    /return\s*{[\s\S]*persistActiveTaskContext,[\s\S]*}/,
    'taskPoller must expose persistActiveTaskContext in its returned API',
  );
  assert.doesNotMatch(
    backgroundSource,
    /[^\w.]persistActiveTaskContext\(\)/,
    'background must not call the taskPoller closure helper as a bare global function',
  );
});
