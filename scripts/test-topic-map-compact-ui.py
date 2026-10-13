#!/usr/bin/env python3
"""Exercise real topic-map assets against bounded synthetic HTTP reads.

Requires Python playwright and Chromium. Does not use a database, platform, model,
production credentials, or mutate files outside the specified temporary proof dir.
The shell/page markup is extracted from tracked Rust templates; Rust compilation
is a separate CI check, not implied by this browser proof.
"""
from __future__ import annotations
import argparse
import copy
import html
import json
import os
from pathlib import Path
import re
import shutil
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / 'apps/api/src/local_web'
STAGES = [('discover_understand', '发现与理解'), ('seek_assessment', '求助与评估'),
          ('choose_support', '选择支持'), ('begin_practice', '开始实践'),
          ('long_term_manage', '长期管理')]
OTHER = ['cross_stage', 'general_background', 'unclear', 'pending']
DOMAIN = '00000000-0000-0000-0000-000000000001'


def works_fixture() -> list[dict]:
    works = []
    for i in range(28):
        ref = f'work-{i}'
        stage = (STAGES[i % 5][0] if i < 24 else OTHER[i-24])
        text = '合成原文：晚间的任务开始之前，先整理时间和准备条件。'
        fragment = dict(fragmentId=f'{ref}.body', field='body', sourceKind='detail_body',
                        text=text, start=0, end=len(text), workRef=ref)
        scene = dict(label=f'晚间准备场景 {i % 8}', evidence=[dict(fragmentId=fragment['fragmentId'],start=0,end=len(text))])
        works.append(dict(workRef=ref, title=('很长的中文标题用于检查两行截断和封面不变形。'*9 if i == 0 else f'合成材料 {i} · 任务开始前的准备'),
            readable=True, platform='xhs' if i % 3 else 'douyin', creatorDisplayName=('很长的作者名称'*8 if i == 0 else f'合成作者 {i % 6}'),
            topicRefs=['topic-A', 'topic-B' if i % 2 else 'topic-C'], preview=dict(text=text*4),
            mainStage=stage, involvedStages=[stage, 'long_term_manage'] if stage not in OTHER else [],
            overlays=['obstruction_recurrence'] if i % 2 else ['transition_handoff'],
            publishedAt=f'2026-10-{12 - i%10:02}T09:00:00Z', publishedAtPrecision='exact',
            likes=None if i==0 else i*85, collects=0, comments=3, shares=None, own=False,
            media=dict(cover=dict(localAssetUrl='/api/local/media/synthetic-cover.svg')),
            research=dict(fragments=[fragment],output=dict(scenes=[scene],discussions=[]))))
    return works


def stats(ns: list[dict]) -> dict:
    return dict(workCount=len(ns), authorCount=len({w['creatorDisplayName'] for w in ns}),
                unknownAuthorCount=0, ownBreakoutCount=0, platforms=[dict(platform=p,
                workCount=len(part), knownLikeCount=sum(w['likes'] is not None for w in part),
                unknownLikeCount=sum(w['likes'] is None for w in part),highPerformanceThreshold=1000,
                highPerformanceCount=sum(w['likes'] is not None and w['likes']>=1000 for w in part),
                medianLikes=85 if part else None,p90Likes=1700 if part else None)
                for p in ['xhs','douyin'] if (part:=[w for w in ns if w['platform']==p])])


def journey(ns: list[dict]) -> dict:
    return dict(denominator=len(ns), main=[dict(stage=stage,label=dict(STAGES).get(stage,stage),
                    count=len(part),workRefs=[w['workRef'] for w in part])
                    for stage in [s for s,_ in STAGES]+OTHER if (part:=[w for w in ns if w['mainStage']==stage]) or stage],
                involved=[dict(stage=stage,label=name,count=len(part),workRefs=[w['workRef'] for w in part])
                    for stage,name in STAGES if (part:=[w for w in ns if stage in w['involvedStages']]) or stage])


class ProofState:
    def __init__(self):
        self.mode='normal'
        self.requests: list[tuple[str,str]]=[]
        self.works=works_fixture()

    def snapshot(self, q: dict[str,list[str]]) -> dict:
        allworks=copy.deepcopy(self.works) if self.mode not in ('empty','error') else []
        val=lambda key:q.get(key,[''])[0]
        if val('platform'): allworks=[w for w in allworks if w['platform']==val('platform')]
        if val('windowDays')=='7': allworks=allworks[:16]
        if val('path'): allworks=allworks[:14]
        if val('overlay'): allworks=[w for w in allworks if val('overlay') in w['overlays']]
        topics=[]
        for i in range(43):
            ref=['topic-A','topic-B','topic-C'][i] if i<3 else f'topic-{i}'
            label=['任务与日常实践','时间与准备条件','具体任务启动'][i] if i<3 else f'主题 {i:02} 很长名称用于检查固定计数列'
            part=[w for w in allworks if ref in w['topicRefs']]
            topics.append(dict(topicRef=ref,displayName=label,parentTopicRef='topic-A' if i in (1,2) else None,
                definitionRef=f'def-{i}',definitionVersion=1,bindingVersion=1,lifecycleState='candidate',
                workRefs=[w['workRef'] for w in part],statistics=stats(part),journey=journey(part),
                definitionText='合成主题的定义和材料边界。', aliases=['已保存别名'],
                core=dict(qualityState='current',sourceState='available',definitionSource='manual',
                          inclusionCriteria=['包含有实际来源的内容'],exclusionCriteria=['不推断作者身份'])) )
        ns=[w for w in allworks if val('topicRef') in w['topicRefs']] if val('topicRef') else allworks
        result=dict(domains=[dict(domainRef=DOMAIN,displayName='合成领域')],topics=topics,
          works=ns,statistics=stats(ns),journey=journey(ns),
          scope=dict(domainRef=DOMAIN,totalWorkCount=len(ns),readableWorkCount=len(ns),unclassifiedWorkCount=0,
              asOf='2026-10-13T02:00:00Z',ownIdentityState='known',ownHistoryPublishedCount=4,
              ownHistoryPublicationUnknownCount=0,recentReferenceWorkRefs=[w['workRef'] for w in ns]),
          sources=[], candidates=[], changes=[],alternatives=[],ownCreators=[],methodVersion='synthetic-browser-proof',
          sourceBoundary='合成测试数据；不是生产资料或研究结果。')
        if self.mode=='partial':result['works']=result['works'][:3]
        if self.mode=='no-scenes':
            for w in result['works']:w.pop('research',None)
        if self.mode=='unknown':
            for p in result['statistics']['platforms']:
                p.update(highPerformanceThreshold=None,medianLikes=None,p90Likes=None)
        return result


def markup() -> str:
    rust=(ASSETS/'shell.rs').read_text()
    shell=re.search(r'r#"(<header class="v7-global-header">[\s\S]+?</header>)"#',rust).group(1)
    links=''.join(f'<a class="v7-primary-item" href="{route}"><span class="v7-nav-zh">{label}</span></a>' for label,route in [('雷达','/'),('主题图谱','/topics'),('语料','/corpus'),('洞察','/insights'),('采集','/collection')])
    page=(ASSETS/'topic_map.rs').read_text()
    crumb=re.search(r'"(主题图谱\{domain\} [^\n]+)"',page).group(1).replace('\\"','"')
    domain=f'<span class="v7-slash">/</span><details class="v7-domain-picker"><summary><span>合成领域</span><small>28 篇</small></summary><nav><a href="/topics?domain={DOMAIN}"><span>合成领域</span><small>28 篇</small></a></nav></details>'
    crumb=crumb.replace('{domain}',domain)
    meta=re.search(r'r#"(<div id="topic-map-context-actions"[^\n]+)"#',page).group(1)
    for key,value in [('nav',links),('boundary_label','本机不访问平台'),('crumb',crumb),('meta',meta)]:shell=shell.replace('{'+key+'}',value)
    page_template=re.search(r'r#"(<!doctype html>[\s\S]+?)"#',page).group(1)
    return page_template.replace('{header}',shell).replace('{default_domain}',DOMAIN)


def handler(state: ProofState):
    class Handler(BaseHTTPRequestHandler):
        def log_message(self,*args):pass
        def do_POST(self):
            state.requests.append(('POST',self.path));self.send_error(405)
        def do_GET(self):
            state.requests.append(('GET',self.path))
            url=urlparse(self.path);path=url.path
            code=200;mime='application/json; charset=utf-8'
            if path=='/topics':body=markup();mime='text/html; charset=utf-8'
            elif path=='/assets/topic-map.js':body=(ASSETS/'topic_map.js').read_text();mime='text/javascript'
            elif path=='/assets/topic-map.css':body='\n'.join((ASSETS/f).read_text() for f in ['lids_tokens.css','shell.css','topic_map.css']);mime='text/css'
            elif path=='/api/local/media/synthetic-cover.svg':
                body='<svg xmlns="http://www.w3.org/2000/svg" width="84" height="112"><rect width="84" height="112" fill="#f7f8f8"/><path d="M12 12h60v88H12zM20 28h44M20 36h35" fill="none" stroke="#111315"/><text x="18" y="75" font-size="12">合成封面</text></svg>';mime='image/svg+xml'
            elif path=='/api/local/topic-map':
                code=503 if state.mode=='error' else 200
                body=json.dumps(dict(code='topic_map_not_connected') if code==503 else state.snapshot(parse_qs(url.query)),ensure_ascii=False)
            elif path=='/api/local/topic-map/research':body=json.dumps(dict(policy=dict(status='paused'),currentMethodSummary=dict(succeededCount=0,failedCount=2,unknownDispatchCount=0),runs=[],models=[]))
            elif path.startswith('/api/local/work-resources/'):
                body=json.dumps(dict(items=[],total=0) if path.endswith('/comments') else dict(item={}))
            else:body='{}'
            data=body.encode();self.send_response(code);self.send_header('Content-Type',mime);self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
    return Handler


def run(out: Path) -> None:
    state=ProofState();server=ThreadingHTTPServer(('127.0.0.1',0),handler(state))
    threading.Thread(target=server.serve_forever,daemon=True).start()
    origin=f'http://127.0.0.1:{server.server_address[1]}'
    out.mkdir(parents=True,exist_ok=True);errors=[];checks=[]
    def passed(name):checks.append(name);print('PASS:',name,flush=True)
    try:
      with sync_playwright() as pw:
        browser=pw.chromium.launch(executable_path=os.environ.get('CHROMIUM_PATH'),headless=True,args=['--no-sandbox'])
        page=browser.new_page(viewport=dict(width=1440,height=1000))
        page.set_default_timeout(12000)
        page.on('pageerror',lambda e:errors.append(str(e)))
        def fresh(view='overview',mode='normal',extra=''):
            state.mode=mode
            page.goto(origin+f'/topics?tm=1&view={view}'+extra)
            if mode!='error':page.locator('.lgi-tm-main[data-loaded="true"]').wait_for()
        def click(action,id=None,base=None):
            selector=f'[data-action="{action}"]'+(f'[data-id="{id}"]' if id is not None else '')
            (base or page).locator(selector).first.click()
        def no_overflow():
            size=page.evaluate('({width:innerWidth,scroll:document.documentElement.scrollWidth})')
            assert size['scroll']<=size['width']+1,size
        def metric_text():return page.locator('.lgi-tm-summary .lgi-tm-metrics').inner_text()
        fresh()
        assert page.locator('#topic-map-view-tabs [data-action="view"]').count()==2
        assert page.locator('.lgi-tm-sidebar [data-action="view"]').count()==0
        assert page.locator('.lgi-tm-summary .lgi-tm-metrics > div').count()==4
        tree=page.locator('.lgi-tm-sidebar').bounding_box();summary=page.locator('.lgi-tm-summary').bounding_box()
        assert tree['x']==0 and 236<=tree['width']<=248 and abs(tree['y']-summary['y'])<=16,(tree,summary)
        assert page.locator('.lgi-tm-work-tabs [data-action="tab"]').count()==6
        page.screenshot(path=str(out/'overview-1440.png'))
        passed('domain-adjacent tabs, flush left tree, top-aligned four metrics and six overview views')
        for tab in ['structure','patterns','sources','candidates','action','content']:
            click('tab',tab);no_overflow()
            assert page.locator('.lgi-tm-summary .lgi-tm-metrics').count()==1
        passed('all overview views retain statistics and actual content / empty states')
        click('view','journey');page.locator('.lgi-tm-main[data-loaded="true"]').wait_for()
        assert page.locator('.lgi-tm-journey-node').count()==5
        assert page.locator('.lgi-tm-sidebar .lgi-tm-tree').count()==1
        baseline=metric_text();basis=page.locator('.lgi-tm-journey-basis').inner_text()
        n=len(state.requests)
        click('stage','begin_practice',page.locator('.lgi-tm-map-shell'))
        click('map-mode','list');assert page.locator('.lgi-tm-journey-table tbody tr').count()==5
        assert metric_text()==baseline and page.locator('.lgi-tm-journey-basis').inner_text()==basis
        click('metric','involved');assert page.locator('.lgi-tm-journey-table th').nth(2).inner_text()=='阶段涉及率'
        click('map-mode','map');assert page.locator('.lgi-tm-journey-node.is-active').count()==1
        assert not state.requests[n:],'presentation and stage controls must not fetch or mutate'
        passed('map/list and primary/involved are independent; selected stage and summary stay stable without requests')
        main=page.locator('.lgi-tm-main');main.evaluate('(el)=>el.scrollTop=0')
        page.screenshot(path=str(out/'journey-1440.png'))
        click('stage','',page.locator('.lgi-tm-journey-scenes'))
        page.locator('#topic-map-scene-search').fill('不存在的场景')
        page.locator('[data-form="scene-search"]').evaluate('(form)=>form.requestSubmit()')
        assert '没有匹配的已研究场景' in main.inner_text()
        assert metric_text()==baseline and page.locator('.lgi-tm-evidence-row').count()>0
        assert page.locator('.lgi-tm-other-stages [data-action="stage"]').count()==4
        passed('scene query never changes summary/map/materials; every actual non-main category remains reachable')
        click('clear-scene-query')
        page.locator('[data-filter="windowDays"]').select_option('7');page.locator('.lgi-tm-main[data-loaded="true"]').wait_for()
        page.locator('[data-filter="path"]').select_option('family');page.locator('.lgi-tm-main[data-loaded="true"]').wait_for()
        click('view','overview');page.locator('.lgi-tm-main[data-loaded="true"]').wait_for()
        assert page.locator('[data-filter="windowDays"]').input_value()==''
        last=next(url for method,url in reversed(state.requests) if url.startswith('/api/local/topic-map?'))
        assert 'path=' not in last and 'overlay=' not in last
        click('view','journey');page.locator('.lgi-tm-main[data-loaded="true"]').wait_for()
        assert page.locator('[data-filter="windowDays"]').input_value()=='7'
        assert page.locator('[data-filter="path"]').input_value()=='family'
        passed('view-specific time windows restore; journey-only constraints cannot silently affect overview')
        fresh('journey')
        sidebar=page.locator('.lgi-tm-sidebar');sidebar.evaluate('(el)=>el.scrollTop=el.scrollHeight')
        before=sidebar.evaluate('(el)=>el.scrollTop')
        click('topic','topic-42',sidebar);page.locator('.lgi-tm-main[data-loaded="true"]').wait_for()
        assert sidebar.evaluate('(el)=>el.scrollTop')>=before-30
        assert page.locator('.lgi-tm-main').evaluate('(el)=>el.scrollTop')==0
        passed('topic selection preserves tree scroll, clears local stage and returns to data top')
        fresh()
        main=page.locator('.lgi-tm-main');main.evaluate('(el)=>el.scrollTop=400');before=main.evaluate('(el)=>el.scrollTop')
        click('rules',base=page.locator('.lgi-tm-summary'))
        page.locator('dialog').wait_for(state='visible');click('dialog-close')
        assert page.locator('dialog').count()==0
        click('research',base=page.locator('#topic-map-context-actions'))
        page.locator('dialog').wait_for(state='visible');click('dialog-close')
        assert page.evaluate('document.activeElement.dataset.action')=='research'
        passed('one details surface, context action focus restores after close')
        for width in [1920,1440,720,390]:
            page.set_viewport_size(dict(width=width,height=1000));fresh('journey')
            no_overflow()
            assert page.locator('[data-action="map-mode"][data-id="map"]').is_visible()
            assert page.locator('[data-action="map-mode"][data-id="list"]').is_visible()
            if width>760:
                height=page.locator('.lgi-tm-map-shell').bounding_box()['height'];assert 290<=height<=390,height
            else:
                click('toggle-navigation');assert sidebar.locator('[data-action="topic"][data-id="topic-B"]').is_visible()
                click('toggle-navigation')
            page.screenshot(path=str(out/f'journey-{width}.png'))
            click('map-mode','list');no_overflow();page.screenshot(path=str(out/f'list-{width}.png'))
        passed('1920/1440/720/390: reachable dual controls, long tree names, local scrolling, no document overflow')
        page.set_viewport_size(dict(width=1440,height=1000))
        for mode in ['empty','unknown','partial','no-scenes','error']:
            fresh('journey',mode)
            if mode=='error':
                page.locator('.lgi-tm-read-error').wait_for();assert page.locator('.lgi-tm-metrics').count()==0
                state.mode='normal';click('refresh',base=page.locator('.lgi-tm-read-error'));page.locator('.lgi-tm-main[data-loaded="true"]').wait_for()
            elif mode=='unknown':assert '未设置' in metric_text() and '—' in metric_text()
            elif mode=='partial':assert '3 / 28' in page.locator('.lgi-tm-partial').inner_text()
            elif mode=='no-scenes':assert page.locator('.lgi-tm-evidence-row').count()>0
            no_overflow()
        passed('empty, unknown, partial, no-scenes, read-error and explicit retry preserve honest boundaries')
        assert not errors,errors
        assert not [r for r in state.requests if r[0]!='GET'],state.requests
        assert not [r for r in state.requests if not r[1].startswith(('/topics','/assets/','/api/local/','/favicon'))]
        passed('no browser exceptions, no model/platform traffic and no write requests during browsing')
        browser.close()
    finally:
        server.shutdown();server.server_close()
        (out/'receipt.json').write_text(json.dumps(dict(checks=checks,pageErrors=errors,requestCount=len(state.requests),boundary='Synthetic HTTP fixture; no actual database, deployment, runtime Rust or user data validation'),ensure_ascii=False,indent=2))

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,default=Path(tempfile.mkdtemp(prefix='topic-map-compact-proof-')));args=parser.parse_args();run(args.output)
