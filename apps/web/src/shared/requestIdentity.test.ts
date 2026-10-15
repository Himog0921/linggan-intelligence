import { describe, expect, it } from 'vitest';
import { RequestIdentities } from './requestIdentity';

function sequence() {
  let next = 0;
  return () => `key-${++next}`;
}

describe('RequestIdentities', () => {
  it('结果未确认后用同样内容重试，沿用原编号', () => {
    const identities = new RequestIdentities(sequence());
    expect(identities.keyFor('probe', 'A')).toBe('key-1');
    expect(identities.keyFor('probe', 'A')).toBe('key-1');
  });

  it('内容变了就换新编号', () => {
    const identities = new RequestIdentities(sequence());
    identities.keyFor('config', 'A');
    expect(identities.keyFor('config', 'B')).toBe('key-2');
  });

  it('得到明确答复后换新编号', () => {
    const identities = new RequestIdentities(sequence());
    identities.keyFor('config', 'A');
    identities.settle('config');
    expect(identities.keyFor('config', 'A')).toBe('key-2');
  });

  it('不同动作互不影响', () => {
    const identities = new RequestIdentities(sequence());
    expect(identities.keyFor('probe:m1', 'A')).toBe('key-1');
    expect(identities.keyFor('probe:m2', 'A')).toBe('key-2');
    identities.settle('probe:m2');
    expect(identities.keyFor('probe:m1', 'A')).toBe('key-1');
  });
});
