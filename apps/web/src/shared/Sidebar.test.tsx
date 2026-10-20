import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe,expect,it } from 'vitest';
import { AppShell } from '../app/AppShell';
import { Sidebar, NavIcon } from './Sidebar';

describe('A壳手动折叠',()=>{
 it('折叠只改变呈现，保留入口、当前项与焦点',async()=>{
  render(<AppShell active="collection" boundaryLabel="只读" crumb="采集" meta={null}><div className="v7-shell"><Sidebar aria-label="测试导航"><a className="v7-side-nav" href="/collection/tasks?domain=same" aria-current="page" aria-label="采集任务" title="采集任务"><NavIcon name="tasks"/><span>采集任务</span></a></Sidebar><main>现有工作区</main></div></AppShell>);
  const user=userEvent.setup();await user.click(screen.getByRole('button',{name:'折叠侧栏'}));
  const toggle=screen.getByRole('button',{name:'展开侧栏'});expect(toggle).toHaveFocus();expect(toggle).toHaveAttribute('aria-expanded','false');
  const link=screen.getByRole('link',{name:'采集任务'});expect(link).toHaveAttribute('href','/collection/tasks?domain=same');expect(link).toHaveAttribute('aria-current','page');
  await user.keyboard('{Enter}');expect(screen.getByRole('button',{name:'折叠侧栏'})).toHaveAttribute('aria-expanded','true');
 });
});
