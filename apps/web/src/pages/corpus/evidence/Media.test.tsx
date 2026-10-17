import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { AuthorAvatar, CoverPreview } from './Media';

describe('头像（F04：可访问名称随加载状态同步）', () => {
  it('地址在读取时叫「头像读取中」，加载成功后叫「<作者>的头像」，加载失败后叫「头像读取失败」', () => {
    render(<AuthorAvatar author="合成作者" url="/api/local/media/a/b" state="ACQUIRED" />);
    const avatar = screen.getByRole('img', { name: '头像读取中' });
    fireEvent.load(avatar.querySelector('img') as HTMLImageElement);
    expect(screen.getByRole('img', { name: '合成作者的头像' })).toBeInTheDocument();
    expect(screen.queryByRole('img', { name: '头像读取中' })).toBeNull();
    expect(screen.queryByText('作者')).toBeNull();
  });

  it('加载失败：撤掉图片，显示占位，名称不再声称图片存在', () => {
    render(<AuthorAvatar author="合成作者" url="/api/local/media/a/b" state="ACQUIRED" />);
    fireEvent.error(screen.getByRole('img', { name: '头像读取中' }).querySelector('img') as HTMLImageElement);
    expect(screen.getByRole('img', { name: '头像读取失败' }).querySelector('img')).toBeNull();
    expect(screen.getByText('作者')).toBeInTheDocument();
  });

  it('没有受控副本：显示占位，名称写头像状态词，绝不使用平台地址', () => {
    render(<AuthorAvatar author="合成作者" url={null} state="NOT_OBSERVED" />);
    const avatar = screen.getByRole('img', { name: '作者头像尚未形成观察' });
    expect(avatar.querySelector('img')).toBeNull();
    expect(avatar).toHaveAttribute('data-media-state', 'absent');
  });

  it('换了一张图就重置加载状态', () => {
    const { rerender } = render(<AuthorAvatar author="甲" url="/api/local/media/a/1" state="ACQUIRED" />);
    fireEvent.load(screen.getByRole('img', { name: '头像读取中' }).querySelector('img') as HTMLImageElement);
    expect(screen.getByRole('img', { name: '甲的头像' })).toBeInTheDocument();
    rerender(<AuthorAvatar author="甲" url="/api/local/media/a/2" state="ACQUIRED" />);
    expect(screen.getByRole('img', { name: '头像读取中' })).toBeInTheDocument();
  });
});

describe('封面预览', () => {
  it('受控封面：图片完整显示；没有封面：显示状态词，名称写封面状态', () => {
    const { rerender } = render(<CoverPreview title="合成作品" url="/api/local/media/c/1" state="AVAILABLE" />);
    expect(screen.getByRole('img', { name: '封面读取中' }).querySelector('img')).toHaveAttribute('src', '/api/local/media/c/1');
    rerender(<CoverPreview title="合成作品" url={null} state="NOT_OBSERVED" />);
    const empty = screen.getByRole('img', { name: '封面尚未形成观察' });
    expect(empty).toHaveTextContent('尚未形成观察');
    expect(empty.querySelector('img')).toBeNull();
  });
});
