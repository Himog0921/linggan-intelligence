import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync,mkdirSync,writeFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {violations} from './check-web-react-boundary.mjs';

test('guard rejects a Rust page, old asset and a second root; JSON and fixture data remain valid',()=>{
 const root=mkdtempSync(path.join(tmpdir(),'linggan-react-boundary-'));
 try {
  const api=path.join(root,'apps/api/src'),web=path.join(root,'apps/web/src');mkdirSync(api,{recursive:true});mkdirSync(web,{recursive:true});
  writeFileSync(path.join(api,'read.rs'),'fn api() -> Result<Body, Error> { Json(value) }');writeFileSync(path.join(web,'main.tsx'),'createRoot(container).render(<App/>);');
  assert.deepEqual(violations(root),[]);
  writeFileSync(path.join(api,'page.rs'),'fn page() { format!("<main>{value}</main>") }');assert.match(violations(root).join('\n'),/Rust business page HTML/);
  rmSync(path.join(api,'page.rs'));writeFileSync(path.join(api,'old.html'),'<html></html>');assert.match(violations(root).join('\n'),/production UI assets/);
  rmSync(path.join(api,'old.html'));writeFileSync(path.join(web,'other.tsx'),'hydrateRoot(container, <App/>);');assert.match(violations(root).join('\n'),/exactly one React root/);
  rmSync(path.join(web,'other.tsx'));writeFileSync(path.join(web,'main.tsx'),'createRoot(first).render(<App/>); createRoot(second).render(<App/>);');assert.match(violations(root).join('\n'),/exactly one React root/);
  writeFileSync(path.join(web,'main.tsx'),'createRoot(container).render(<App/>);');writeFileSync(path.join(api,'latest.rs'),'fn page() { format!("<main>{value}</main>") }');assert.match(violations(root).join('\n'),/Rust business page HTML/);
  rmSync(path.join(api,'latest.rs'));writeFileSync(path.join(api,'page_tests.rs'),'let synthetic = "<main>fixture</main>";');assert.deepEqual(violations(root),[]);
 } finally {rmSync(root,{recursive:true,force:true});}
});
