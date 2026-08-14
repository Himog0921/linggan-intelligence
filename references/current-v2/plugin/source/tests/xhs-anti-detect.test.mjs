import assert from 'node:assert/strict';
import test from 'node:test';

import { showCaptchaPauseOverlay } from '../src/platforms/xhs/antiDetect.js';

function installFakeDom() {
  const previousDocument = globalThis.document;
  const previousWindow = globalThis.window;

  class FakeElement {
    constructor(className = '') {
      this.className = className;
      this.style = {};
      this.listeners = new Map();
      this.children = new Map();
      this.removed = false;
    }

    set innerHTML(value) {
      this._innerHTML = value;
      [
        'lgbbb-captcha-dialog',
        'lgbbb-captcha-icon',
        'lgbbb-captcha-title',
        'lgbbb-captcha-desc',
        'lgbbb-captcha-resume-btn',
        'lgbbb-captcha-stop-btn',
      ].forEach((className) => {
        this.children.set(`.${className}`, new FakeElement(className));
      });
    }

    querySelector(selector) {
      return this.children.get(selector) || null;
    }

    addEventListener(name, handler) {
      this.listeners.set(name, handler);
    }

    click() {
      this.listeners.get('click')?.();
    }

    remove() {
      this.removed = true;
      if (documentState.current === this) documentState.current = null;
    }
  }

  const documentState = {
    current: null,
  };

  globalThis.document = {
    querySelector(selector) {
      if (selector === '.lgbbb-captcha-overlay') return documentState.current;
      return null;
    },
    createElement() {
      return new FakeElement('lgbbb-captcha-overlay');
    },
    body: {
      appendChild(element) {
        documentState.current = element;
      },
    },
  };
  globalThis.window = {
    setTimeout,
    clearTimeout,
  };

  return {
    get overlay() {
      return documentState.current;
    },
    restore() {
      globalThis.document = previousDocument;
      globalThis.window = previousWindow;
    },
  };
}

test('captcha overlay resolves timeout and removes itself', async () => {
  const env = installFakeDom();
  try {
    const result = await showCaptchaPauseOverlay({ timeoutMs: 5 });
    assert.equal(result, 'timeout');
    assert.equal(env.overlay, null);
  } finally {
    env.restore();
  }
});
