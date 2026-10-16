import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { DEFAULT_QUERY } from './evidenceQuery';
import { workRef } from './evidenceFixtures';
import { ReturnBanners } from './ReturnBanners';

describe('返回横幅', () => {
  it('没有集合参数：不出现横幅', () => {
    const { container } = render(<ReturnBanners query={DEFAULT_QUERY} domainParam="d" />);
    expect(container).toBeEmptyDOMElement();
  });

  it('创作者作品集合：文字与链接不变；returnTo 须是创作者页地址', () => {
    render(<ReturnBanners query={{ ...DEFAULT_QUERY, creatorKey: 'xhs:41', returnTo: '/corpus/creators?domain=d&page=2' }} domainParam="d" />);
    expect(screen.getByText(/当前为创作者作品筛选集合/)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '返回创作者' })).toHaveAttribute('href', '/corpus/creators?domain=d&page=2');
  });

  it('returnTo 不是创作者页：回创作者页本身（带领域）', () => {
    render(<ReturnBanners query={{ ...DEFAULT_QUERY, creatorFilter: '{}', returnTo: 'https://evil.example' }} domainParam="d" />);
    expect(screen.getByRole('link', { name: '返回创作者' })).toHaveAttribute('href', '/corpus/creators?domain=d');
  });

  it('本次采集的固定作品：说明详情、评论与媒体的回查口径；返回采集任务', () => {
    render(<ReturnBanners query={{ ...DEFAULT_QUERY, collectionRunRef: workRef(7), returnTo: '/collection/tasks?domain=d&run=r' }} domainParam="d" />);
    expect(screen.getByText(/当前为本次采集的固定作品；详情与评论按本次取得／明确复用版本回查；媒体为共享当前资源。/)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '返回采集任务' })).toHaveAttribute('href', '/collection/tasks?domain=d&run=r');
  });

  it('returnTo 不是采集任务页：回本次采集的任务页', () => {
    render(<ReturnBanners query={{ ...DEFAULT_QUERY, collectionRunRef: workRef(7), returnTo: '/corpus/creators?x' }} domainParam="d" />);
    expect(screen.getByRole('link', { name: '返回采集任务' })).toHaveAttribute('href', `/collection/tasks?domain=d&run=${workRef(7)}`);
  });
});
