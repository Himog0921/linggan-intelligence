import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { calls, DOMAIN, installFetch, json, overview, renderPage } from './testKit';
afterEach(()=>vi.unstubAllGlobals());
describe('restored comment overview on the current research contract',()=>{
 it('keeps source counts, the observation chart and word cloud when research was reset',async()=>{
  installFetch();renderPage(`domain=${DOMAIN}&view=overview`);
  const metrics=await screen.findByLabelText('研究累计读数与昨日活动');
  expect(metrics).toHaveTextContent('100');expect(metrics).toHaveTextContent('可显示 200');
  expect(screen.getByRole('img',{name:'评论柱状与研究信号渐变曲线，共同横轴及采集覆盖'})).toBeVisible();
  await screen.findAllByText('作业拖拉',{exact:true});
  expect(screen.getByRole('heading',{name:'已有依据的用户问题'})).toBeVisible();
  expect(screen.getByText(/已有原始评论仍可在/)).toBeVisible();
  expect(screen.queryByText('尚无新流程的研究记录。')).not.toBeInTheDocument();
  expect(screen.queryByRole('button',{name:'选择评论开始研究'})).not.toBeInTheDocument();
  expect(calls.every(call=>call.method==='GET')).toBe(true);
  expect(calls.some(call=>call.url.pathname==='/api/local/comment-study/overview')).toBe(false);
 });
 it('preserves the selected period and chart view while navigating the four views',async()=>{
  installFetch();renderPage(`domain=${DOMAIN}&view=overview`);
  await screen.findByLabelText('研究累计读数与昨日活动');
  await userEvent.click(screen.getByRole('button',{name:'近 7 天'}));
  await userEvent.click(screen.getByRole('button',{name:'表格'}));
  expect(within(screen.getByRole('table')).getAllByRole('row')).toHaveLength(8);
  await userEvent.click(screen.getByRole('button',{name:'用户评论'}));await screen.findByRole('searchbox',{name:'字面搜索'});
  await userEvent.click(screen.getByRole('button',{name:'概览'}));
  await waitFor(()=>expect(screen.getByRole('button',{name:'近 7 天'})).toHaveAttribute('aria-pressed','true'));
  expect(screen.getByRole('button',{name:'表格'})).toHaveAttribute('aria-pressed','true');
 });
 it('shows unindexed material as pending instead of implying there are zero researchable comments',async()=>{
  const value=overview();value.indexCoverage.pendingCount=30;value.corpusSummary.eligibleCommentCount=0;
  installFetch(url=>url.pathname.endsWith('/comment-research/overview')?json(value):undefined);renderPage(`domain=${DOMAIN}&view=overview`);
  const metrics=await screen.findByLabelText('研究累计读数与昨日活动');
  expect(metrics).toHaveTextContent('已有 200 条原始评论 · 30 条待索引');
  const headline=metrics.querySelector('strong');expect(headline).toHaveTextContent('—');
 });
 it('keeps word-frequency reads available when overview fails and rejects a different domain',async()=>{
  installFetch(url=>url.pathname.endsWith('/comment-research/overview')?json({...overview(),domainRef:'wrong-domain'}):undefined);renderPage(`domain=${DOMAIN}&view=overview`);
  await screen.findByRole('alert');await screen.findAllByText('作业拖拉',{exact:true});
  expect(screen.queryByLabelText('研究累计读数与昨日活动')).not.toBeInTheDocument();
  expect(screen.getByRole('button',{name:'用户评论'})).toBeEnabled();
 });
 it('hides previously displayed source text when qualification refresh fails',async()=>{
  const base=overview();const value={...base,knowledgeSummary:{...base.knowledgeSummary,solutionSignalCount:1,solutionWorkCount:1,solutionPreview:[{requestRef:'request-a',commentKey:{workRef:'work-a',commentExternalId:'comment-a'},quote:'撤回之前的合成原声',proposition:'合成概括',workTitle:'合成作品'}]}};
  let failed=false;
  installFetch(url=>url.pathname.endsWith('/comment-research/overview')?(failed?json({error:'unavailable'},503):json(value)):undefined);
  const {queryClient}=renderPage(`domain=${DOMAIN}&view=overview`);
  await screen.findByText('“撤回之前的合成原声”');failed=true;
  await act(async()=>{await queryClient.invalidateQueries({queryKey:['comment-research',DOMAIN,'overview']});});
  await screen.findByRole('alert');expect(screen.queryByText('“撤回之前的合成原声”')).not.toBeInTheDocument();
  expect(screen.queryByText('合成概括')).not.toBeInTheDocument();
 });

});
