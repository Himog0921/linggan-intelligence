import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { RunLedger } from './RunLedger';
import { parseRun } from './tasksApi';

const run = parseRun({ run_ref:'a0000000-0000-4000-8000-000000000001', domain_ref:'a0000000-0000-4000-8000-000000000002', name:'很长的合成任务名称'.repeat(15), domain_name:'合成领域', source_kind:'patrol', legacy:true, ended:true, control_intent:null, phase:'ended', comment_mode:null, created_at:'2026-10-14T00:00:00Z', updated_at:'2026-10-14T00:00:00Z', revision:null, work_limit:20, selected_works:20, available_details:0, delivered_comments:null, cumulative_comments:1234567 });
afterEach(cleanup);
describe('采集单行账本的数据与操作边界', () => {
  it('完整17字段保持同一个任务行，真实零/未知分开且完整名称可打开原对象', () => {
    const open=vi.fn();
    const {container}=render(<RunLedger runs={[run]} message="空" onOpen={open}/>);
    const rows=container.querySelectorAll('[data-run-row]');expect(rows).toHaveLength(1);
    const row=rows[0]!;expect(row.querySelectorAll('td')).toHaveLength(17);
    expect(row.querySelector('[data-field=details]')).toHaveTextContent('0');
    expect(row.querySelector('[data-field=comments]')).toHaveTextContent('—');
    expect(row.querySelector('[data-field=cumulative]')).toHaveTextContent('1234567');
    expect(row.querySelector('[data-field=rank]')).toHaveAttribute('title',expect.stringContaining('未提供搜索排序'));
    expect(row.querySelector('[data-field=name]')).toHaveAttribute('title',run.name);
    fireEvent.click(screen.getByRole('button',{name:run.name}));expect(open).toHaveBeenCalledWith(run);
  });
  it('用户可隐藏非核心字段，四个核心字段固定，恢复后仍是一行', async () => {
    const {container}=render(<RunLedger runs={[run]} message="空" onOpen={()=>{}}/>);
    fireEvent.click(screen.getByText('字段'));
    const panel=await screen.findByRole('group',{name:'显示字段'});
    expect(within(panel).getByRole('checkbox',{name:'任务名称'})).toBeDisabled();
    fireEvent.click(within(panel).getByRole('checkbox',{name:'累计评论'}));
    expect(container.querySelector('[data-run-row] [data-field=cumulative]')).toBeNull();
    fireEvent.click(within(panel).getByRole('button',{name:'恢复默认字段'}));
    expect(container.querySelectorAll('[data-run-row] td')).toHaveLength(17);
    fireEvent.keyDown(panel,{key:'Escape'});
    await waitFor(()=>expect(screen.queryByRole('group',{name:'显示字段'})).not.toBeInTheDocument());
  });
  it('读取失败与已知空使用不同反馈，不伪造任务行', () => {
    const {container,rerender}=render(<RunLedger runs={[]} message="正在读取采集记录…" onOpen={()=>{}}/>);
    expect(screen.getByText('正在读取采集记录…')).toBeInTheDocument();
    rerender(<RunLedger runs={[]} message="读取失败；不能据此认定没有任务" onOpen={()=>{}}/>);
    expect(screen.getByText('读取失败；不能据此认定没有任务')).toBeInTheDocument();
    expect(container.querySelectorAll('[data-run-row]')).toHaveLength(0);
  });
});
