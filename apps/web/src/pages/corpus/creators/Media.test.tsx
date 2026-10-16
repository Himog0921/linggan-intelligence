import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { Avatar, Cover } from './Media';

describe('头像与封面：可访问名称随加载状态同步（F04）', () => {
  it('地址还在读取：说「读取中」，不说「未取得」', () => {
    render(<Avatar name="合成作者" url={null} pending className="a" />);
    expect(screen.getByRole('img', { name: '头像读取中' })).toBeInTheDocument();
  });

  it('确认没有地址：「未取得」，并显示名字首字作占位', () => {
    render(<Avatar name="合成作者" url={null} pending={false} className="a" />);
    const image = screen.getByRole('img', { name: '头像未取得' });
    expect(image).toHaveTextContent('合');
    expect(screen.queryByRole('img', { name: '作者头像' })).toBeNull();
  });

  it('有地址但图片还没加载完：仍是「读取中」；加载成功后名称变成「作者头像」，不再同时说未取得', () => {
    const { container } = render(<Avatar name="合成作者" url="/api/local/media/a" pending={false} className="a" />);
    expect(screen.getByRole('img', { name: '头像读取中' })).toBeInTheDocument();
    fireEvent.load(container.querySelector('img') as HTMLImageElement);
    expect(screen.getByRole('img', { name: '作者头像' })).toBeInTheDocument();
    expect(screen.queryByRole('img', { name: /未取得|读取中/ })).toBeNull();
  });

  it('加载失败：名称变成「未取得」，坏图被移除，占位回来', () => {
    const { container } = render(<Avatar name="合成作者" url="/api/local/media/a" pending={false} className="a" />);
    fireEvent.error(container.querySelector('img') as HTMLImageElement);
    expect(screen.getByRole('img', { name: '头像未取得' })).toHaveTextContent('合');
    expect(container.querySelector('img')).toBeNull();
  });

  it('封面同理；地址换了就是另一张图，不沿用上一张的「已加载」', () => {
    const { container, rerender } = render(<Cover url="/api/local/media/1" pending={false} className="c" />);
    fireEvent.load(container.querySelector('img') as HTMLImageElement);
    expect(screen.getByRole('img', { name: '作品封面' })).toBeInTheDocument();
    rerender(<Cover url="/api/local/media/2" pending={false} className="c" />);
    expect(screen.getByRole('img', { name: '封面读取中' })).toBeInTheDocument();
    fireEvent.error(container.querySelector('img') as HTMLImageElement);
    expect(screen.getByRole('img', { name: '封面未取得' })).toHaveTextContent('▧');
  });

  it('图片本身是装饰（alt 为空），名称只由外层一处给出，读屏不会听到两个相反的说法', () => {
    const { container } = render(<Avatar name="合成作者" url="/api/local/media/a" pending={false} className="a" />);
    expect(container.querySelector('img')).toHaveAttribute('alt', '');
    expect(screen.getAllByRole('img')).toHaveLength(1);
  });
});
