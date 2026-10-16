import { render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { feedback } from './evidenceFeedback';
import { FactGrid, FeedbackLine, InlineState, Section, SourceNote, StateLine } from './Primitives';

describe('状态线', () => {
  it('圆点＋中文状态词；原始码不进普通层文字，只放 data-state（F06）', () => {
    const { container } = render(<StateLine state="RISK_CONTROL" />);
    expect(screen.getByText('风险控制停止')).toBeInTheDocument();
    expect(screen.queryByText('RISK_CONTROL')).toBeNull();
    expect(container.firstElementChild).toHaveAttribute('data-state', 'RISK_CONTROL');
  });

  it('词表外的值写「来源未完整表达」；空值当作未知；技术位置可以带码', () => {
    render(
      <>
        <StateLine state="SOMETHING_NEW" />
        <StateLine state={null} />
        <StateLine state="ACQUIRED" showCode />
      </>,
    );
    expect(screen.getByText('来源未完整表达')).toBeInTheDocument();
    expect(screen.getByText('当前未知')).toBeInTheDocument();
    expect(screen.getByText('ACQUIRED')).toBeInTheDocument();
  });
});

describe('事实表', () => {
  const unknown = (label: string) => [label, '当前未知', 'UNKNOWN'] as const;

  it('少于 3 项未知：平铺，每行原文案', () => {
    render(<FactGrid rows={[['标题', '合成作品', 'KNOWN'], unknown('作者'), unknown('时间')]} />);
    expect(screen.queryByText(/尚未取得/)).toBeNull();
    expect(screen.getByText('合成作品')).toBeInTheDocument();
    expect(screen.getAllByText('当前未知')).toHaveLength(2);
  });

  it('连续 3 项及以上未知：折叠成「尚未取得 N 项」，展开后逐条原文案；已知值不会被折进去', () => {
    const { container } = render(<FactGrid rows={[['标题', '合成作品', 'KNOWN'], unknown('作者'), unknown('时间'), unknown('精度'), ['平台', 'xhs']]} />);
    const summary = screen.getByText('尚未取得 3 项');
    const details = summary.closest('details') as HTMLElement;
    expect(details).not.toHaveAttribute('open');
    expect(within(details).getAllByText('当前未知')).toHaveLength(3);
    expect(within(details).queryByText('合成作品')).toBeNull();
    expect(container.querySelectorAll('dl')).toHaveLength(2);
    expect(screen.getByText('xhs')).toBeInTheDocument();
  });

  it('全是未知：只有折叠区', () => {
    const { container } = render(<FactGrid rows={[unknown('a'), unknown('b'), unknown('c')]} />);
    expect(container.querySelectorAll('dl')).toHaveLength(1);
    expect(screen.getByText('尚未取得 3 项')).toBeInTheDocument();
  });

  it('技术码作旁注，不改写值', () => {
    render(<FactGrid rows={[['状态', '可检索', 'SEARCHABLE']]} />);
    expect(screen.getByText('可检索').closest('dd')).toHaveTextContent('可检索SEARCHABLE');
  });
});

describe('分节、行内状态与来源说明', () => {
  it('分节标题可带技术码；行内状态三种色调互不共用', () => {
    render(
      <Section title="本机媒体材料" tech="04">
        <InlineState tone="danger" title="评论研究通道读取失败" code="material_comment_channel_unavailable" />
        <InlineState tone="restricted" title="原文只在本机授权详情中按页读取">作者只显示匿名上下文。</InlineState>
        <SourceNote>当前详情没有返回限制列表；页面不据此推断没有限制。</SourceNote>
      </Section>,
    );
    expect(screen.getByRole('heading', { name: /本机媒体材料/ })).toBeInTheDocument();
    expect(screen.getByText('评论研究通道读取失败').closest('div')).toHaveAttribute('data-tone', 'danger');
    expect(screen.getByText('原文只在本机授权详情中按页读取').closest('div')).toHaveAttribute('data-tone', 'restricted');
    expect(screen.getByText('material_comment_channel_unavailable')).toBeInTheDocument();
  });
});

describe('动作回执', () => {
  it('没有回执时是空的 status 区；有回执时：结论词＋说明＋技术码，色调随结论', () => {
    const { rerender } = render(<FeedbackLine feedback={null} />);
    expect(screen.getByRole('status')).toBeEmptyDOMElement();
    rerender(<FeedbackLine feedback={feedback('refused', '请求已记录，没有形成工单。', 'REFUSED')}><button type="button">重新核对</button></FeedbackLine>);
    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('data-tone', 'refused');
    expect(within(status).getByText('未受理')).toBeInTheDocument();
    expect(within(status).getByText('REFUSED')).toBeInTheDocument();
    expect(within(status).getByRole('button', { name: '重新核对' })).toBeInTheDocument();
  });
});
