import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, useLocation } from 'react-router';
import { afterEach, expect, it, vi } from 'vitest';
import { TasksPage } from './TasksPage';
function Location(){return <output data-testid="route">{useLocation().search}</output>;}
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
it('搜索手动提交，组词期间不查询，清空恢复范围且不读取旧诊断',async()=>{
  const shell={domains:[],domainsReadable:true,selectedDomain:null,totalTargets:0,monitoringTargets:0,scheduler:'unknown'};
  const fetch=vi.fn(async(path:string)=>new Response(JSON.stringify(path.startsWith('/api/collection/workspace/shell')?shell:{items:[],total:0,page:1,pageSize:20}),{status:200,headers:{'Content-Type':'application/json'}}));vi.stubGlobal('fetch',fetch);
  const client=new QueryClient({defaultOptions:{queries:{retry:false}}});
  render(<MemoryRouter initialEntries={['/collection/tasks?domain=all']}><QueryClientProvider client={client}><TasksPage/><Location/></QueryClientProvider></MemoryRouter>);
  await screen.findByText('当前筛选没有采集记录。');
  const input=screen.getByRole('searchbox',{name:'搜索采集任务'}),form=screen.getByRole('search');
  fireEvent.compositionStart(input);fireEvent.change(input,{target:{value:'测试搜索'}});fireEvent.submit(form);
  expect(new URLSearchParams(screen.getByTestId('route').textContent??'').get('q')).toBeNull();
  fireEvent.compositionEnd(input);fireEvent.submit(form);
  await waitFor(()=>expect(new URLSearchParams(screen.getByTestId('route').textContent??'').get('q')).toBe('测试搜索'));
  fireEvent.click(screen.getByRole('button',{name:'清空任务搜索'}));
  expect(input).toHaveValue('');
  expect(new URLSearchParams(screen.getByTestId('route').textContent??'').get('q')).toBeNull();
  expect(screen.queryByText('历史执行记录与诊断')).not.toBeInTheDocument();
  expect(fetch.mock.calls.every(([path])=>!path.includes('executions')&&!path.includes('legacy'))).toBe(true);
});
