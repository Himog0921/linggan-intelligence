import { describe, expect, it } from 'vitest';
import { apiPath, controlledPath, detailPath, directDetailPath, imagePath, mediaPath, xhsNoteUrl } from './evidencePaths';
import { DOMAIN, workRef } from './evidenceFixtures';

const ORIGIN = 'http://127.0.0.1:3000';
const REF = workRef(1);

it('原文导航仅接受小红书笔记HTTPS地址，保留实际来源查询参数', () => {
  const url = 'https://www.xiaohongshu.com/explore/0123456789abcdef01234567?xsec_token=synthetic';
  expect(xhsNoteUrl(url)).toBe(url);
  for (const value of [null, '', 'javascript:alert(1)', '//www.xiaohongshu.com/explore/0123456789abcdef01234567', 'https://www.xiaohongshu.com.evil.test/explore/0123456789abcdef01234567', 'https://www.xiaohongshu.com/user/profile/0123456789abcdef01234567']) expect(xhsNoteUrl(value)).toBeNull();
});

describe('受控地址', () => {
  it('同源且在允许前缀下才通过；返回路径加查询串', () => {
    expect(controlledPath('/api/local/media/a/b?x=1', ['/api/local/media/'], ORIGIN)).toBe('/api/local/media/a/b?x=1');
    expect(controlledPath(`${ORIGIN}/api/local/media/a/b`, ['/api/local/media/'], ORIGIN)).toBe('/api/local/media/a/b');
  });

  it('跨源、协议相对、其他路径、空值与非字符串一律拒绝：永不回退到平台 CDN', () => {
    for (const bad of ['https://cdn.example/api/local/media/a/b', '//evil.example/api/local/media/a/b', '/api/other/x', '', null, undefined, 3, 'http://[::1']) {
      expect(controlledPath(bad, ['/api/local/media/'], ORIGIN)).toBeNull();
    }
  });

  it('图片句柄接受媒体与派生物；原图与头像只接受媒体', () => {
    expect(imagePath('/api/local/derivative/x')).toBe('/api/local/derivative/x');
    expect(imagePath('/api/local/media/x/y')).toBe('/api/local/media/x/y');
    expect(mediaPath('/api/local/derivative/x')).toBeNull();
    expect(mediaPath('/api/local/media/x/y')).toBe('/api/local/media/x/y');
  });
});

describe('接口地址', () => {
  it('详情地址：同源、正好是「根/作品引用」这一层', () => {
    expect(detailPath(`/api/local/work-resources/${REF}?domain=${DOMAIN}`)).toBe(`/api/local/work-resources/${REF}?domain=${DOMAIN}`);
    expect(detailPath(`/api/local/work-resources/${REF}`)).toBe(`/api/local/work-resources/${REF}`);
  });

  it('评论通道、底稿、复观测等子通道，以及其他前缀与跨源，都不是详情地址（不猜路由）', () => {
    for (const bad of [
      `/api/local/work-resources/${REF}/comments?domain=${DOMAIN}`,
      `/api/local/work-resources/${REF}/research-dossier`,
      `/api/local/work-resources/${REF}/reobserve`,
      '/api/local/work-resources/',
      '/api/local/work-resources',
      `/api/local/other/${REF}`,
      `https://evil.example/api/local/work-resources/${REF}`,
      null,
    ]) {
      expect(detailPath(bad)).toBeNull();
    }
  });

  it('接口前缀：评论通道地址与复观测入口按此校验', () => {
    expect(apiPath(`/api/local/work-resources/${REF}/comments?domain=${DOMAIN}`)).toBe(`/api/local/work-resources/${REF}/comments?domain=${DOMAIN}`);
    expect(apiPath('/api/local/creators')).toBeNull();
  });

  it('work 不在已读页内时直读：保持所选领域与 collectionRunRef，非 UUID 不采用', () => {
    expect(directDetailPath(REF, DOMAIN, null)).toBe(`/api/local/work-resources/${REF}?domain=${DOMAIN}`);
    expect(directDetailPath(REF, DOMAIN, workRef(9))).toBe(`/api/local/work-resources/${REF}?domain=${DOMAIN}&collectionRunRef=${workRef(9)}`);
    expect(directDetailPath('abc', DOMAIN, null)).toBeNull();
  });
});
