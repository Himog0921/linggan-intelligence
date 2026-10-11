#!/usr/bin/env python3
"""Browser proof for the served creator assets using synthetic HTTP receipts only.
Run: python scripts/test-creator-approved-ui.py [--output /tmp/creator-ui-proof]
Requires Python Playwright and a Chromium installation. No model/database access.
"""
import argparse
import asyncio
import copy
import json
import os
from pathlib import Path
from urllib.parse import parse_qs, urlsplit
from playwright.async_api import async_playwright

ROOT = Path(__file__).resolve().parents[1]
WEB = ROOT / 'apps/api/src/local_web'
D = '00000000-0000-4000-8000-000000000001'
W = '00000000-0000-4000-8000-000000001001'
BASE = 'http://127.0.0.1:18731'
URL = BASE + '/corpus/creators?domain=' + D
# Only the shell fixture is synthetic. In the full repository the two canonical
# stylesheets are read unchanged, as they are by the Rust CSS composition route.
HEADER = '''<header class="v7-global-header"><div class="v7-global-row"><div class="v7-global-brand"><b class="v7-li-mark">LI</b><div><strong>Linggan Intelligence</strong><small>领域情报工作台</small></div></div><nav class="v7-primary-nav"><button>雷达</button><button>主题图谱</button><a aria-current="page">语料</a><button>洞察</button><button>采集</button></nav><div class="v7-global-flex"></div><div class="v7-global-system">合成 API 验证</div></div><div class="v7-context-row"><div></div><div class="v7-context-main"><span>语料　/　示例领域　/　<b>创作者</b></span><span id="creator-header-readout"></span></div></div></header>'''
NAV = '<aside class="v7-side"><a class="v7-side-nav">01　证据库</a><a class="v7-side-nav">02　评论研究</a><a class="v7-side-nav" aria-current="page">03　创作者</a><small class="v7-side-foot">合成测试，不连接业务数据库</small></aside>'

def fixture():
    items = []
    names = ['林间学习记录', '周末慢慢来', '阿澄的日常观察', '长名字用于校验单行截断与作者入口可达性的一位创作者']
    for i in range(80):
        works = []
        for j in range(3):
            ref = f'00000000-0000-4000-8000-{1001+i*3+j:012d}'
            fr = [dict(fragmentId=ref+'.body', field='body', text='这是合成的第一人称实践片段，用来验证引用分组与长段落换行。'*9, sourceType='work_material', workRef=ref, workTitle='合成实践记录', sourceRef=ref), dict(fragmentId=ref+'.title', field='title', text='合成的方法说明标题', sourceType='work_material', workRef=ref, workTitle='合成方法记录', sourceRef=ref)]
            val = lambda value, n: dict(value=value, reason='根据本次提供的片段作出的合成判断，不能用于实际内容研究。', evidenceFragmentIds=[fr[n]['fragmentId']])
            auto = dict(relevance=val('related',0), traits=dict(personal_experience=val('yes',0),professional_output=val('yes',1),explicit_promotion=val('unknown',1)))
            works.append(dict(workRef=ref,title='把一项任务拆成三个小步骤之后，记录发生了什么以及哪些方法仍然需要调整 · 合成长标题',relevance='related',likes=1360+i,collects=19,comments=8,acquisitionKind='profile_discovery',analysis=dict(automatic=auto,manual={},evidence=fr,supportFragments=fr,state='idle',resultAt='2026-10-10T00:00:00Z',resultAtDisplay='示例时间')))
        bio=dict(fragmentId=f'author.{i}.biography',field='biography',text='这是合成公开简介：记录学习方法与日常实践。',sourceType='author_profile',sourceRef=W)
        focus=dict(value='vertical_tendency',reason='简介与已有作品呈现相近的内容方向；本判断仅为界面测试。'*4,evidenceFragmentIds=[bio['fragmentId'],works[0]['workRef']+'.body'])
        analysis=dict(automatic=dict(focus=focus,evidence=[bio,*works[0]['analysis']['evidence']]),institution_or_brand=dict(value='unknown',reason='未取得足够资料',evidenceFragmentIds=[]),supportFragments=[bio,*works[0]['analysis']['evidence'],*works[1]['analysis']['evidence']],identitySupportFragments=[bio],manual={},state='queued',resultAtDisplay=None)
        o=dict(inCurrentDomain=i%4==0,monitoringState='monitoring',lifecycleState='active',targetRef='target-'+str(i),usageRole='primary',existsInOtherDomains=False,hasOtherPrimaryDomain=False)
        items.append(dict(creatorKey='xhs:synthetic-'+str(i),displayName=names[i%4]+f' {i+1:02d}',authorExternalId='synthetic-'+str(i),profile=dict(followerCount=None if i==1 else 2026+i,biography=bio['text']),focus=focus,authorAnalysis=analysis,collectedWorkCount=35,relatedWorkCount=7,matchedRelatedWorkCount=2,unknownRelevanceWorkCount=28,unrelatedWorkCount=0,viralWorkCount=1,maxRelatedLikeCount=0 if i==1 else 1360+i,candidateHighLikeCount=2000+i,matchedWorkRefs=[w['workRef'] for w in works],topicHints=['任务拆解与日常练习','如何记录长周期的实践变化与支持方式'],traits=['personal_experience','professional_output'],representatives=works,observation=o))
    return dict(domain=dict(domainRef=D,name='示例领域',status='active',description='',researchGoal=''),policy=dict(analysisEnabled=True,likeThreshold=1000,revision=3,configRef='synthetic-config',dailyTokenLimit=100000),stats=dict(authors=80,works=240,vertical=20,personal=40,outside=60,outsideViral=10,relatedAuthors=70,unknownAuthorWorks=2,unknownDateWorks=3),total=80,items=items,displayAsOf='示例读取时间')


# Offline isolation: history and storage are mock navigation state, not evidence
# of live routing/BFCache. fetch never opens a socket, and every write is synthetic.
MOCK = r"""(data) => {
 window.__fixture=data;window.__scenario='normal';window.__requests=[];window.__writes=[];
 window.__mockURL='';window.__delayPath='';window.__status=200;window.__release=null;
 Object.defineProperty(window,'sessionStorage',{value:(()=>{const m=new Map();return {getItem:k=>m.get(k)||null,setItem:(k,v)=>m.set(k,v),removeItem:k=>m.delete(k)}})()});
 history.pushState=history.replaceState=function(state,title,url){window.__mockURL=url;};
 window.fetch=async (url,options={})=>{
  const u=new URL(url,'http://offline.invalid'),method=options.method||'GET';
  window.__requests.push({url:u.pathname+u.search,method});
  if(method!=='GET'){
   window.__writes.push({path:u.pathname,body:JSON.parse(options.body)});
   if(window.__delayPath&&u.pathname.includes(window.__delayPath))await new Promise(r=>window.__release=r);
   return {ok:window.__status===200,json:async()=>window.__status===200?{queued:true,targetRef:'synthetic-target',usageRole:'primary'}:{error:{code:'synthetic_error'}}};
  }
  if(u.pathname==='/api/local/model-settings')return {ok:window.__scenario!=='model_failure',json:async()=>({config:{configRef:'synthetic-config',modelId:'Synthetic model'}})};
  if(u.pathname==='/api/local/work-resources')return {ok:true,json:async()=>({items:[]})};
  if(u.pathname!=='/api/local/creators')throw Error('Unmocked request '+url);
  if(window.__scenario==='read_failure')return {ok:false,json:async()=>({error:{code:'creator_read_unavailable'}})};
  const d=structuredClone(window.__fixture);
  if(window.__scenario==='closed')d.policy.analysisEnabled=false;
  if(window.__scenario==='no_threshold')d.policy.likeThreshold=null;
  if(window.__scenario==='paused')d.domain.status='paused';
  for(const it of d.items){
   if(window.__scenario==='failed')it.authorAnalysis.state='failed';
   if(window.__scenario==='unknown'){d.stats.outside=null;it.observation.inCurrentDomain=null;}
   if(window.__scenario==='conflict')Object.assign(it.observation,{inCurrentDomain:false,hasOtherPrimaryDomain:true,existsInOtherDomains:true});
  }
  if(window.__scenario==='empty'){d.total=0;d.items=[];}
  const p=Number(u.searchParams.get('page')||0),size=Number(u.searchParams.get('pageSize')||50);d.items=d.items.slice(p*size,p*size+size);
  return {ok:true,json:async()=>d};
 };
}"""

async def run(output: Path):
    output.mkdir(parents=True,exist_ok=True)
    html=(WEB/'creators.html').read_text().replace('{{HEADER}}',HEADER).replace('{{SIDE_NAV}}',NAV).replace('{{DOMAIN_VALUE}}',D)
    html=html.replace('<link rel="stylesheet" href="/assets/creators.css">','').replace('<script src="/assets/creators.js" defer></script>','')
    css='\n'.join((WEB/f).read_text() for f in ['lids_tokens.css','shell.css','creators.css'])
    js=(WEB/'creators.js').read_text()
    proofs=[]; errors=[]
    async with async_playwright() as pw:
        options=dict(headless=True)
        binary=os.environ.get('CHROMIUM_PATH') or ('/usr/bin/chromium' if Path('/usr/bin/chromium').exists() else None)
        if binary:options['executable_path']=binary
        browser=await pw.chromium.launch(**options)
        context=await browser.new_context(viewport={'width':1440,'height':900})
        page=None
        async def fresh(scenario='normal'):
            nonlocal page
            if page:await page.close()
            page=await context.new_page();page.on('pageerror',lambda error:errors.append(str(error)))
            # No navigation to localhost or any live target.
            await page.route('**/*',lambda r:r.abort())
            await page.set_content(html);await page.add_style_tag(content=css)
            await page.evaluate(MOCK,fixture());await page.evaluate('(s)=>window.__scenario=s',scenario)
            await page.add_script_tag(content=js);await page.wait_for_function("document.querySelector('#creator-results').getAttribute('aria-busy')==='false'")
        async def loaded():await page.wait_for_function("document.querySelector('#creator-results').getAttribute('aria-busy')==='false' && document.querySelector('#rows tr')")
        async def open_author(i=0):await page.locator('#rows .creator-name').nth(i).click();await page.locator('#drawer').wait_for(state='visible')
        def proof(name):proofs.append(name);print('PASS',name,flush=True)
        await fresh();assert await page.locator('#creator-results th').count()==8;assert await page.locator('#rows tr').count()==50;proof('eight_columns_server_response_page')
        for width,height in [(1440,900),(1280,720),(1920,1080),(900,720),(375,812),(640,360)]:
            await fresh();await page.set_viewport_size({'width':width,'height':height})
            geometry=await page.evaluate("""()=>({width:innerWidth,doc:document.documentElement.scrollWidth,rows:[...document.querySelectorAll('#rows tr')].slice(0,5).map(x=>x.getBoundingClientRect().height),head:document.querySelector('thead').getBoundingClientRect().height,pager:document.querySelector('[data-page-nav]').getBoundingClientRect().bottom,table:document.querySelector('#creator-results').getBoundingClientRect().height})""")
            await page.screenshot(path=str(output/f'list-{width}.png'))
            assert geometry['doc']<=width+1,geometry
            assert all(abs(h-80)<=1 for h in geometry['rows']),geometry
            assert geometry['pager']<=height+1 and geometry['table']>=70,geometry
            assert abs(geometry['head']-40)<=1,geometry
            await open_author()
            d=await page.evaluate("""()=>({top:document.querySelector('#drawer').getBoundingClientRect().top,shell:document.querySelector('.v7-shell').getBoundingClientRect().top,body:document.querySelector('#drawer-body').getBoundingClientRect().height,bottom:document.querySelector('.creator-drawer-footer').getBoundingClientRect().bottom})""")
            await page.screenshot(path=str(output/f'drawer-{width}.png'))
            assert abs(d['top']-max(0,d['shell']))<=1 and d['body']>=55 and d['bottom']<=height+1,d
            await page.keyboard.press('Escape');proof(f'geometry_{width}x{height}')
        await fresh()
        await page.locator('#filter-open').click();await page.locator('#filter-form [name=focus]').select_option('vertical_tendency');n=await page.evaluate('__requests.length');await page.locator('#filter-dialog [data-close]').first.click();assert await page.evaluate('__requests.length')==n;assert 'focus=' not in await page.evaluate('__mockURL');proof('draft_cancel_no_query')
        await page.locator('#filter-open').click();await page.locator('#filter-form [name=traits]').nth(0).check();await page.locator('#filter-form [name=traits]').nth(1).check();await page.locator('#filter-form [name=focus]').select_option('vertical_tendency');await page.locator('#filter-form [type=submit]').click();await loaded();url=await page.evaluate('__mockURL');assert 'traits=personal_experience%2Cprofessional_output' in url and 'focus=vertical_tendency' in url;proof('draft_apply_group_or_intersection_params')
        await page.locator('#active-conditions [data-remove=traits]').first.click();await loaded();url=await page.evaluate('__mockURL');assert 'personal_experience' not in url and 'professional_output' in url;proof('remove_individual_condition')
        await page.locator('#quick [data-quick=all]').click();await loaded();await page.locator('#creator-query').fill('合成检索');await page.wait_for_function('__mockURL.includes("query=")');await loaded();assert parse_qs(urlsplit(await page.evaluate('__mockURL')).query)['query']==['合成检索'];proof('debounced_search')
        await page.locator('#search-clear').click();await loaded();await page.locator('[data-sort=high_likes]').click();await loaded();assert await page.locator('#creator-sort').input_value()=='high_likes';assert await page.locator('#likes-heading').inner_text()=='命中最高赞';proof('sort_header_sync')
        await page.locator('[data-page-next]').click();await loaded();assert await page.locator('#rows tr').count()==30;await page.locator('[data-page-prev]').click();await loaded();assert await page.locator('#rows tr').count()==50;proof('page_request_serialization')
        await fresh();assert await page.locator('#rows tr').nth(1).locator('td').nth(5).inner_text()=='0\n已判相关作品';assert '—' in await page.locator('#rows tr').nth(1).inner_text();proof('unknown_not_zero')
        await open_author();await page.locator('#drawer-tab-evidence').click();await page.locator('#drawer-evidence [data-work]').first.locator('summary').first.click()
        first=page.locator('#drawer-evidence [data-work]').first
        assert await first.locator('.creator-field').count()==4
        await first.locator('.creator-field').first.locator('details summary').click();assert '第一人称' in await first.locator('.creator-field').first.locator('blockquote').inner_text();assert '方法说明标题' not in await first.locator('.creator-field').first.locator('blockquote').inner_text();proof('field_specific_evidence')
        await page.screenshot(path=str(output/'evidence-1440.png'))
        link=first.locator('blockquote a').first;href=await link.get_attribute('href');assert parse_qs(urlsplit(href).query)['work']==[W];assert 'returnTo' in parse_qs(urlsplit(href).query);proof('exact_evidence_link_fields_not_live_navigation')
        await page.evaluate("document.addEventListener('click',e=>{if(e.target.closest('a[href^=\"/corpus/evidence?\"]'))e.preventDefault()})")
        await link.click();saved=await page.evaluate("JSON.parse(sessionStorage.getItem('linggan.creatorDiscovery.return'))");assert saved['tab']=='evidence' and W in saved['expanded'];proof('evidence_click_saves_return_state')
        await page.keyboard.press('Escape');await page.wait_for_function('document.activeElement.className.includes("creator-name")');proof('escape_focus_restore')
        await fresh('conflict');await open_author();assert await page.locator('#drawer-action [data-add]').get_attribute('data-role')=='reference';proof('other_domain_conflict_keeps_reference_choice')
        await fresh('no_threshold');assert await page.locator('#quick [data-quick=high]').inner_text()=='未观察的高赞作者';proof('no_threshold_high_like_available')
        await fresh('unknown');assert not await page.locator('#quick [data-quick=outside]').is_enabled();proof('relationship_unknown_not_outside')
        await fresh('read_failure');assert await page.locator('#result-count').inner_text()=='读取失败';assert await page.locator('#rows tr').count()==0;assert not await page.locator('#table-empty').is_visible();proof('read_error_not_empty')
        await fresh('empty');assert await page.locator('#table-empty').is_visible();assert not await page.locator('[data-page-next]').is_enabled();proof('empty_page')
        for kind in ['correction','policy','add','retry']:
            for status in [200,503]:
                await fresh('failed' if kind=='retry' else 'normal')
                paths={'correction':'creator-discovery-overrides','policy':'creator-discovery-policy','add':'observation-target','retry':'creator-discovery-analysis/retry'}
                await page.evaluate('([path,status])=>{__delayPath=path;__status=status}',[paths[kind],status])
                if kind=='policy':
                    await page.locator('#policy-open').click();await page.wait_for_function('!document.querySelector("#model-config").disabled');await page.locator('#policy-form [type=submit]').click();await page.wait_for_function('__release!==null');await page.locator('#policy-close').click();await page.locator('#policy-open').click();await page.wait_for_function('!document.querySelector("#model-config").disabled');await page.locator('#threshold').fill('555')
                elif kind=='correction':
                    await open_author();await page.locator('#drawer-overview [data-correct=focus]').click();await page.locator('#correction-value').select_option('unknown');await page.locator('#correction-form [type=submit]').click();await page.wait_for_function('__release!==null');await page.locator('#correction-close').click();await page.locator('#drawer-overview [data-correct=focus]').click();await page.locator('#correction-reason').fill('新的未保存理由')
                else:
                    await open_author(1);button=page.locator('#drawer-action [data-add]' if kind=='add' else '#drawer-overview [data-retry-author]');await button.click();await page.wait_for_function('__release!==null');await page.locator('#drawer-close').click();await open_author(2)
                await page.evaluate('__release()');await page.wait_for_timeout(20)
                if kind=='policy':assert await page.locator('#policy-dialog').is_visible();assert await page.locator('#threshold').input_value()=='555'
                elif kind=='correction':assert await page.locator('#correction-dialog').is_visible();assert await page.locator('#correction-reason').input_value()=='新的未保存理由'
                else:assert await page.locator('#drawer').is_visible();assert await page.locator('#drawer-action-status').inner_text()==''
                proof(f'{kind}_stale_{status}')
        await fresh();await page.evaluate("__delayPath='observation-target';__status=503");await open_author(1);await page.locator('#drawer-action [data-add]').click();await page.wait_for_function('__release!==null');await page.locator('#drawer-close').click();await open_author(1);assert not await page.locator('#drawer-action [data-add]').is_enabled();await page.evaluate('__release()');await page.wait_for_function('!document.querySelector("#drawer-action [data-add]").disabled');proof('same_author_pending_button_recovers')
        for destination in ['drawer','filter']:
            await fresh();await page.evaluate("__delayPath='observation-target';__status=200");await page.locator('#rows [data-add]').first.click();await page.wait_for_function('__release!==null')
            if destination=='drawer':await open_author(2)
            else:await page.locator('#filter-open').click();await page.locator('#filter-form [name=minLikes]').fill('456')
            await page.evaluate('__release()');await page.wait_for_timeout(20);assert await page.locator('#drawer' if destination=='drawer' else '#filter-dialog').is_visible();proof('row_receipt_keeps_new_'+destination)
        await fresh('model_failure');await page.locator('#policy-open').click();await page.wait_for_function('document.querySelector("#policy-status").textContent.includes("读取失败")');await page.locator('#threshold').fill('400');await page.locator('#policy-form [type=submit]').click();await loaded();payload=await page.evaluate('__writes.at(-1).body');assert set(payload)=={'domain','revision','likeThreshold'};proof('settings_failure_preserves_analysis_fields')
        await fresh();await open_author();await page.locator('#drawer-overview [data-correct=focus]').click();n=await page.evaluate('__writes.length');await page.locator('#correction-form [type=submit]').click();assert await page.evaluate('__writes.length')==n;assert '支持依据' in await page.locator('#correction-status').inner_text();proof('positive_override_requires_support')
        await page.locator('#correction-value').select_option('unknown');await page.locator('#correction-form [type=submit]').click();await page.locator('#correction-dialog').wait_for(state='hidden');await loaded();proof('current_save_completes')
        assert not errors,errors
        (output/'results.json').write_text(json.dumps({'passed':len(proofs),'checks':proofs,'pageErrors':errors,'mode':'offline HTML + mocked fetch/history/storage; no live routing, database, model, or HTTP writes'},ensure_ascii=False,indent=2))
        print(json.dumps({'passed':len(proofs),'pageErrors':errors,'output':str(output)},ensure_ascii=False));await browser.close()

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--output',default='/tmp/creator-approved-ui-proof');args=parser.parse_args();asyncio.run(run(Path(args.output)))
