import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { TargetFeedback } from './TargetFeedback';
import type { Commands } from './useWorkspace';

const commands = (feedback: Commands['feedback'] = null): Commands => ({
  run: vi.fn(async () => false),
  verify: vi.fn(async () => undefined),
  busy: false,
  feedback,
});
afterEach(cleanup);

describe('观察目标回执呈现', () => {
  it.each([
    ['archive_not_requestable', '目标当前所处阶段不接受这次建档或补采申请。没有新建工单；请查看目标状态。'],
    ['archive_gap_not_schedulable', '目录仍有详情或评论缺口，但当前缺少可用访问地址，或受执行输入与重试条件限制。本次没有新建工单；请查看受阻作品和采集任务。'],
    ['material_retirement_empty', '一篇都没有勾选。没有勾选不等于全部确认，这次没有写下任何判断。'],
    ['material_retirement_failed', '这次判断没有写进去。作品目录与待补齐都没有变化，可以再试一次。'],
    ['batch_failed', '这次操作没有完成，也没有改动任何观察目标。'],
    ['read_model_not_connected', '当前无法读取目标状态，这次没有产生任何改动。'],
  ])('从地址恢复 %s 时保留失败原因且不作为成功角标消失', (code, explanation) => {
    render(<TargetFeedback commands={commands()} code={code} ahead="3" />);
    const status = screen.getByRole('status');
    expect(status).toHaveTextContent(explanation);
    expect(status.className).not.toBe('c-tg-toast');
    expect(status).not.toHaveTextContent('前面还有');
  });
  it('已收到的批量拒绝回执使用同一完整文案', () => {
    render(<TargetFeedback commands={commands({
      requestRef: '00000000-0000-4000-8000-000000000001', action: 'batch', text: '通用失败',
      unknown: false, code: 'batch_nothing_selected', applied: false,
    })} />);
    expect(screen.getByRole('status')).toHaveTextContent('没有选中任何观察目标。');
    expect(screen.getByRole('status')).toHaveClass('c-src-failure');
    expect(screen.getByRole('status')).not.toHaveTextContent('通用失败');
  });
  it('补采未确认保留可核对的诊断编号，非法后缀不冒充编号', () => {
    const reference = '00000000-0000-4000-8000-000000000001';
    const { rerender } = render(<TargetFeedback commands={commands()} code={`archive_request_failed_${reference}`} />);
    expect(screen.getByRole('status')).toHaveTextContent('提交结果待核对');
    expect(screen.getByRole('status')).toHaveTextContent(`诊断编号：${reference}。`);
    rerender(<TargetFeedback commands={commands()} code="archive_request_failed_<script>" />);
    expect(screen.getByRole('status')).toHaveTextContent('上一次动作没有完成。');
    expect(screen.getByRole('status')).not.toHaveTextContent('诊断编号');
  });
  it.each([['3', '前面还有 3 个在排队。'], ['0', '它就排在最前面。']])(
    '成功回执保留已知排队位置 %s', (ahead, expected) => {
      render(<TargetFeedback commands={commands()} code="archive_requested" ahead={ahead} />);
      expect(screen.getByRole('status')).toHaveClass('c-tg-toast');
      expect(screen.getByRole('status')).toHaveTextContent(expected);
    },
  );
  it('未知排队位置不编成零；退役成功与未知错误都可见', () => {
    const { rerender } = render(<TargetFeedback commands={commands()} code="archive_requested" />);
    expect(screen.getByRole('status')).not.toHaveTextContent('排在最前面');
    rerender(<TargetFeedback commands={commands()} code="material_retirement_done" />);
    expect(screen.getByRole('status')).toHaveTextContent('已记下这几篇不存在了');
    expect(screen.getByRole('status')).toHaveTextContent('作品若日后恢复并采到详情，会自动按已有详情算。');
    rerender(<TargetFeedback commands={commands()} code="future_unrecognised_error" />);
    expect(screen.getByRole('status')).toHaveTextContent('上一次动作没有完成。');
    expect(screen.getByRole('status')).toHaveClass('c-src-failure');
  });
  it('断网未知不套用成功文案并保留核实入口', () => {
    const request = commands({
      requestRef: '00000000-0000-4000-8000-000000000002', action: 'archive', text: '保留原请求编号',
      unknown: true, code: 'archive_requested', applied: false,
    });
    render(<TargetFeedback commands={request} ahead="0" />);
    expect(screen.getByRole('status')).toHaveTextContent('提交结果未确认');
    expect(screen.getByRole('status')).toHaveTextContent('00000000-0000-4000-8000-000000000002');
    expect(screen.getByRole('status')).not.toHaveTextContent('建档已入队');
    const verify = screen.getByRole('button', { name: '核实本次请求' });
    expect(verify).toHaveFocus();
    expect(verify).not.toBeDisabled();
    fireEvent.keyDown(verify, { key: 'Tab' });
    expect(verify).toHaveFocus();
    fireEvent.click(verify);
    expect(request.verify).toHaveBeenCalledOnce();
    expect(request.run).not.toHaveBeenCalled();
  });
});
