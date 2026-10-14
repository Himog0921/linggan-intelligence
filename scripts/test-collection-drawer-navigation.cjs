// Pure event/transport contract tests: no browser, database, provider or platform.
const assert = require('node:assert/strict');
const { test } = require('node:test');
const fs = require('node:fs');
const vm = require('node:vm');
const source = fs.readFileSync('apps/api/src/local_web/collection_workspace.js', 'utf8').split('/* Target drawers reuse the displayed list, but read their facts afresh on every navigation. */')[1];
function fixture() {
  const events = {}, windowEvents = {}, requests = [], assigned = [], installed = [], focus = [];
  let drawer = null;
  const location = { href: 'http://127.0.0.1:3001/collection/targets?domain=all', origin: 'http://127.0.0.1:3001', pathname: '/collection/targets', assign: url => assigned.push(url), reload: () => assigned.push('reload') };
  const document = {
    body: { setAttribute() {}, removeAttribute() {}, appendChild: next => { drawer = next; installed.push(next.label); } },
    querySelector: selector => selector === '[data-row-opener]' ? { href: location.href + '&drawer=first' } : null,
    getElementById: id => id === 'c-drawer' ? drawer : { focus: () => focus.push(id) },
    addEventListener: (name, fn) => { (events[name] ||= []).push(fn); },
    dispatchEvent: event => (events[event.type] || []).forEach(fn => fn(event))
  };
  const window = {
    location, AbortController, fetch: (url, options) => new Promise((resolve, reject) => requests.push({ url, options, resolve, reject })),
    history: { pushState: (_a, _b, url) => { location.href = String(url); } },
    setTimeout, clearTimeout, addEventListener: (name, fn) => { (windowEvents[name] ||= []).push(fn); }
  };
  const context = { window, document, URL, AbortController, CustomEvent: class { constructor(type) { this.type = type; } }, DOMParser: class {
    parseFromString(label) { return { getElementById: () => label === 'missing' ? null : {
      label, querySelectorAll: () => [], remove: () => { drawer = null; },
      replaceWith: next => { drawer = next; installed.push(next.label); }
    } }; }
  } };
  vm.runInNewContext(source, context);
  function click(query, options = {}) {
    const href = 'http://127.0.0.1:3001/collection/targets?domain=all' + query;
    const link = { href, target: '', hasAttribute: () => false, classList: { contains: name => options.close && name === 'c-dw-close' } };
    const event = { button: 0, ...options, target: { closest: () => link }, preventDefault() { this.defaultPrevented = true; } };
    (events.click || []).forEach(fn => fn(event));
    return event;
  }
  const settle = () => new Promise(resolve => setImmediate(resolve));
  const deliver = async (index, label) => { requests[index].resolve({ ok: true, text: async () => label }); await settle(); };
  return { click, requests, deliver, settle, assigned, installed, focus, location, document, windowEvents };
}

test('open and close preserve the list document, canonical URL and focus', async () => {
  const f = fixture(), originalBody = f.document.body;
  assert.equal(f.click('&drawer=first').defaultPrevented, true);
  assert.equal(new URL(f.requests[0].url).searchParams.get('drawer_fragment'), 'true');
  assert.equal(f.requests[0].options.cache, 'no-store');
  await f.deliver(0, 'first');
  assert.equal(f.document.body, originalBody);
  assert.equal(new URL(f.location.href).searchParams.has('drawer_fragment'), false);
  f.click('#target-first', { close: true });
  assert.equal(f.document.getElementById('c-drawer'), null);
  assert.deepEqual(f.focus, ['target-first']);
});

test('superseded responses cannot install a stale drawer or force full navigation', async () => {
  const f = fixture();
  f.click('&drawer=first'); f.click('&drawer=second');
  assert.equal(f.requests[0].options.signal.aborted, true);
  await f.deliver(1, 'second'); await f.deliver(0, 'first');
  assert.deepEqual(f.installed, ['second']);
  assert.deepEqual(f.assigned, []);
  assert.equal(new URL(f.location.href).searchParams.get('drawer'), 'second');
});

test('current read failure uses the canonical full-page link', async () => {
  const f = fixture(); f.click('&drawer=first');
  f.requests[0].reject(new Error('synthetic transport failure')); await f.settle();
  assert.equal(f.assigned.length, 1);
  assert.equal(new URL(f.assigned[0]).searchParams.has('drawer_fragment'), false);
});

test('modified clicks, commands and different list scopes keep ordinary navigation', () => {
  const f = fixture();
  assert.equal(f.click('&drawer=first', { ctrlKey: true }).defaultPrevented, undefined);
  assert.equal(f.click('&drawer=first&rule=first').defaultPrevented, undefined);
  assert.equal(f.click('&drawer=first&error=failed').defaultPrevented, undefined);
  assert.equal(f.click('&drawer=first&filter=creator').defaultPrevented, undefined);
  assert.equal(f.requests.length, 0);
});

test('Back re-reads drawer facts rather than restoring cached permission or Lease facts', async () => {
  const f = fixture(); f.click('&drawer=first'); await f.deliver(0, 'first');
  f.windowEvents.popstate[0]();
  assert.equal(f.requests.length, 2);
  await f.deliver(1, 'fresh');
  assert.deepEqual(f.installed, ['first', 'fresh']);
});

test('fragment mounting enhances native work filters once and preserves their change value', () => {
  const whole = fs.readFileSync('apps/api/src/local_web/collection_workspace.js', 'utf8');
  const drawn = whole.slice(whole.indexOf('// 一套自绘 select'), whole.indexOf('// 执行工位右侧'));
  const listeners = {}, changes = [];
  function element() {
    const handlers = {}, attrs = {}, classes = new Set();
    return { children: [], hidden: false, textContent: '',
      classList: { contains: name => classes.has(name), add: name => classes.add(name) },
      setAttribute: (key, value) => { attrs[key] = value; },
      appendChild(child) { this.children.push(child); },
      addEventListener: (key, fn) => { (handlers[key] ||= []).push(fn); },
      dispatchEvent: event => { (handlers[event.type] || []).forEach(fn => fn(event)); },
      click() { (handlers.click || []).forEach(fn => fn({ stopPropagation() {} })); },
      focus() {}, attrs
    };
  }
  const select = element(); select.options = [{ value: 'all', textContent: '全部' }, { value: 'pending', textContent: '待采详情' }]; select.value = 'all';
  Object.defineProperty(select, 'selectedIndex', { get: () => select.options.findIndex(option => option.value === select.value) });
  select.addEventListener('change', () => changes.push(select.value));
  const field = element(); field.querySelector = selector => selector === 'select' ? select : null;
  let fields = [];
  const drawer = { querySelectorAll: () => fields };
  const document = { querySelectorAll: () => fields, createElement: element,
    getElementById: () => drawer, addEventListener: (key, fn) => { (listeners[key] ||= []).push(fn); } };
  vm.runInNewContext(drawn, { document, Event: class { constructor(type) { this.type = type; } } });
  fields = [field];
  listeners['collection-drawer-mounted'].forEach(fn => fn());
  assert.equal(field.classList.contains('c-tg-drawn'), true);
  assert.equal(field.children.length, 2);
  listeners['collection-drawer-mounted'].forEach(fn => fn());
  assert.equal(field.children.length, 2, 'repeated mounting cannot add duplicate controls');
  field.children[1].children[1].click();
  assert.equal(select.value, 'pending');
  assert.deepEqual(changes, ['pending']);
  assert.equal(field.children[0].children[0].textContent, '待采详情');
});
