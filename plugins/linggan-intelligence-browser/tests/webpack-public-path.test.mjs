import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const webpackConfig = require('../webpack.config.cjs');

test('webpack uses an explicit public path so content scripts do not auto-detect page scripts', () => {
  assert.equal(webpackConfig.output.publicPath, '');
});
