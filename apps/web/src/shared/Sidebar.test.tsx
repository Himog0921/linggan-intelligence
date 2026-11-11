import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe,expect,it } from 'vitest';
import { AppShell } from '../app/AppShell';
import { Sidebar, NavIcon } from './Sidebar';

describe('A壳手动折叠',()=>{
 it('折叠只改变呈现，保留入口、当前项与焦点',async()=>{
  render(<AppShell active="collection" boundaryLabel="只读" contextBar={{ breadcrumb: "采集", meta: null }}><div className="v7-shell"><Sidebar aria-label="测试导航"><a className="v7-side-nav" href="/collection/tasks?domain=same" aria-current="page" aria-label="采集任务" title="采集任务"><NavIcon name="tasks"/><span>采集任务</span></a></Sidebar><main>现有工作区</main></div></AppShell>);
  const user=userEvent.setup();await user.click(screen.getByRole('button',{name:'折叠侧栏'}));
  const toggle=screen.getByRole('button',{name:'展开侧栏'});expect(toggle).toHaveFocus();expect(toggle).toHaveAttribute('aria-expanded','false');
  const link=screen.getByRole('link',{name:'采集任务'});expect(link).toHaveAttribute('href','/collection/tasks?domain=same');expect(link).toHaveAttribute('aria-current','page');expect(link.querySelector('svg')).toHaveAttribute('data-icon','tasks');
  await user.keyboard('{Enter}');expect(screen.getByRole('button',{name:'折叠侧栏'})).toHaveAttribute('aria-expanded','true');
 });
 it('renders level 02 parents and level 03 destinations in one collapsible navigation tree',async()=>{
  const user=userEvent.setup();
  render(<AppShell active="ops" boundaryLabel="本机" contextBar={{ breadcrumb: "运营", meta: null }}><div className="v7-shell"><Sidebar aria-label="运营导航" eyebrow="OPERATIONS / NAV" title="运营工作区" activeId="data-publications" activeParentId="data" items={[
    {id:'data',label:'数据',index:'01',icon:'overview',href:'/ops/overview',children:[
      {id:'data-overview',label:'概览',code:'OV',href:'/ops/overview'},
      {id:'data-publications',label:'发布记录',code:'PUB',href:'/ops/publications'},
    ]},
    {id:'creation',label:'创作',index:'02',icon:'creation',href:'/ops/creation',children:[
      {id:'creation-today',label:'今日选题',code:'TODAY',href:'/ops/creation?section=today'},
      {id:'creation-works',label:'我的创作',code:'WORKS',href:'/ops/creation?section=works'},
    ]},
  ]}/><main>运营工作区</main></div></AppShell>);
  const parent=screen.getByRole('link',{name:'数据'});
  expect(parent).toHaveAttribute('href','/ops/overview');
  expect(screen.getByRole('link',{name:'发布记录'})).toHaveAttribute('aria-current','page');
  await user.click(screen.getByRole('button',{name:'展开创作子页面'}));
  expect(screen.getByRole('link',{name:'今日选题'})).toHaveAttribute('href','/ops/creation?section=today');
  expect(screen.getByRole('link',{name:'我的创作'})).toHaveAttribute('href','/ops/creation?section=works');
  const disclosure=screen.getByRole('button',{name:'收起数据子页面'});
  expect(disclosure).toHaveAttribute('aria-expanded','true');
  await user.click(disclosure);
  expect(screen.queryByRole('link',{name:'发布记录'})).not.toBeInTheDocument();
  await user.click(screen.getByRole('button',{name:'展开数据子页面'}));
  expect(screen.getByRole('link',{name:'发布记录'})).toBeVisible();
 });

});