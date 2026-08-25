import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, '..');

test('injected user script can run repeatedly on the same page', () => {
  const source = fs.readFileSync(path.join(projectRoot, 'src/injected/user.js'), 'utf8');
  const messages = [];
  const pageWindow = {
    __INITIAL_STATE__: {
      user: {
        userPageData: {
          _rawValue: {
            basicInfo: { nickname: 'Test Author' },
            interactions: [{ type: 'fans', count: '10k' }],
          },
        },
        userInfo: {
          _rawValue: {
            userId: 'xhs-user-1',
          },
        },
      },
    },
    postMessage(payload) {
      messages.push(payload);
    },
  };
  const context = vm.createContext({ window: pageWindow });
  const script = new vm.Script(source, { filename: 'injected/user.js' });

  assert.doesNotThrow(() => {
    script.runInContext(context);
    script.runInContext(context);
  });

  assert.equal(messages.length, 2);
  assert.equal(messages[0].type, 'user');
  assert.deepEqual(messages[0].data.userPageData.basicInfo, { nickname: 'Test Author' });
  assert.deepEqual(messages[0].data.userInfo, { userId: 'xhs-user-1' });
});
