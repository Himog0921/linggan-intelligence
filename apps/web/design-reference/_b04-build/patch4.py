import pathlib
P = pathlib.Path('.')
def edit(name, pairs):
    f = P / name; s = f.read_text()
    for old, new in pairs:
        assert s.count(old) == 1, (name, old[:90], s.count(old))
        s = s.replace(old, new)
    f.write_text(s)

FILTER_BTN = '<button type="button" class="btn" id="advancedButton" aria-expanded="false" aria-controls="advancedFilters"><span data-icon="SlidersHorizontal"></span>筛选</button>'
OLD_ADV = '<div id="advancedFilters" class="advanced" hidden><div class="field"><label for="minLikes">最低点赞</label><input type="number" id="minLikes" min="0" step="1" placeholder="不限"></div><div class="field"><label for="markFilter">改帖标记</label><select id="markFilter"><option value="all">全部</option><option value="marked">已标记</option><option value="unmarked">未标记</option></select></div><button class="btn quiet" id="clearFilters">清除筛选</button></div>\n'
POP = ('<div class="filter-anchor"><button type="button" class="btn" id="advancedButton" aria-haspopup="dialog" aria-expanded="false" aria-controls="advancedFilters"><span data-icon="SlidersHorizontal"></span>筛选<b class="filter-count" id="advancedCount" hidden></b></button>'
       '<div id="advancedFilters" class="filter-pop" role="dialog" aria-label="更多筛选" hidden>'
       '<div class="pop-row"><label for="minLikes">最低点赞</label><input type="number" id="minLikes" min="0" step="1" placeholder="不限"></div>'
       '<div class="pop-row"><label for="markFilter">改帖标记</label><select id="markFilter"><option value="all">全部评论</option><option value="marked">已标记</option><option value="unmarked">未标记</option></select></div>'
       '<div class="pop-foot"><button type="button" class="btn quiet small" id="clearFilters">清除筛选</button><button type="button" class="btn small primary" id="advancedDone">完成</button></div>'
       '</div></div>')
edit('src-body.html', [
 ('<header class="section-head"><h1 id="commentsTitle">用户评论</h1><p id="commentsMeta"></p></header>', '<header class="section-head"><h1 id="commentsTitle">用户评论</h1></header>'),
 ('<option value="all">全部</option><option value="never">尚未研究</option>', '<option value="all">全部状态</option><option value="never">尚未研究</option>'),
 (FILTER_BTN, POP),
 (OLD_ADV, ''),
 ('<div class="table-foot"><span>点击评论，查看原声与研究证据</span><span id="commentPageMeta"></span></div>',
  '<div class="table-foot"><span class="page-range" id="pageRange"></span><nav class="pager" id="pager" aria-label="评论分页"></nav>'
  '<div class="pager-tools"><label class="page-jump">跳至<input type="number" id="pageJump" min="1" step="1" inputmode="numeric" aria-label="跳转到页码">页</label>'
  '<select id="pageSize" aria-label="每页条数"><option value="10">10 条/页</option><option value="20">20 条/页</option><option value="50">50 条/页</option></select></div></div>'),
 # problems: same page header + same toolbar standard as comments
 ('<h1 class="sr-only" id="problemsTitle">用户问题</h1><div class="pa-page"><div class="pa-toolbar"><div class="search-shell pa-search-shell">',
  '<header class="section-head"><h1 id="problemsTitle">用户问题</h1></header><div class="pa-page"><div class="toolbar pa-toolbar"><div class="field search-field"><label for="problemSearch">搜索问题</label><div class="search-shell">'),
 ('placeholder="搜索问题名称或描述"></div><div class="pa-controls"><div class="seg" role="group" aria-label="问题状态"><button type="button" data-pstate="all" aria-pressed="true">全部 <b id="pcAll"></b></button><button type="button" data-pstate="formal" aria-pressed="false" title="正式问题：至少 2 位不同作者提到">正式 <b id="pcFormal"></b></button><button type="button" data-pstate="candidate" aria-pressed="false" title="候选问题：目前只有 1 位作者提到">候选 <b id="pcCandidate"></b></button></div><select id="problemStatus" hidden aria-hidden="true" tabindex="-1"><option value="all">全部</option><option value="formal">正式</option><option value="candidate">候选</option></select><div class="inline-field"><label for="problemSort">排序</label><select id="problemSort"><option value="recent">最新研究</option><option value="authors">作者数量</option><option value="comments">评论数量</option><option value="works">作品数量</option></select></div></div></div>',
  'placeholder="搜索问题名称或描述"></div></div><div class="field"><label for="problemStatus">问题状态</label><select id="problemStatus"><option value="all">全部问题</option><option value="formal">正式问题</option><option value="candidate">候选问题</option></select></div><div class="field"><label for="problemSort">排序</label><select id="problemSort"><option value="recent">最新研究</option><option value="authors">作者最多</option><option value="comments">评论最多</option><option value="works">作品最多</option></select></div></div>'),
])
edit('src-body.html', [('<input id="problemSearch" type="search" aria-label="搜索问题名称或描述"', '<input id="problemSearch" type="search"')])

S = P / 'src-scripts.html'
edit('src-scripts.html', [
 # header meta removed
 ("""    {const sm=data.summary;el('commentsMeta').innerHTML=researchSegments(sm).map(([label,n])=>`${label} <b>${fmt(n)}</b>`).join(' · ')+(sm.waiting?`<span class="meta-split">${fmt(sm.waiting)} 条待作品语境</span>`:'');}\n""", ""),
 # pagination
 ("""    el('commentPageMeta').textContent=`第 1 页 · 共 ${rows.length} 条`;\n""", """    renderPager(rows.length);\n"""),
 ("""    el('commentRows').innerHTML=rows.map(c=>{""", """    const sig=[el('search').value,el('searchScope').value,el('statusFilter').value,el('tagFilter').value,el('commentSort').value,el('minLikes').value,el('markFilter').value,state.workId,state.problemId,state.pageSize].join('|');if(sig!==state.filterSig){state.filterSig=sig;state.page=1;}
    const pages=Math.max(1,Math.ceil(rows.length/state.pageSize));state.page=Math.min(Math.max(1,state.page),pages);
    const pageRows=rows.slice((state.page-1)*state.pageSize,state.page*state.pageSize);
    el('commentRows').innerHTML=pageRows.map(c=>{"""),
 ("""  function updateSelection(rows=visibleComments()) {""", """  function renderPager(total) {
    const size=state.pageSize,pages=Math.max(1,Math.ceil(total/size)),p=state.page,from=total?(p-1)*size+1:0,to=Math.min(total,p*size);
    el('pageRange').textContent=`第 ${from}–${to} 条 · 共 ${total} 条`;
    const nums=[...new Set([1,p-1,p,p+1,pages].filter(n=>n>=1&&n<=pages))].sort((a,b)=>a-b);
    let html=`<button type="button" class="page-btn" data-page="${p-1}" ${p<=1?'disabled':''} aria-label="上一页">‹</button>`,prev=0;
    nums.forEach(n=>{if(n-prev>1)html+='<span class="page-gap" aria-hidden="true">…</span>';html+=`<button type="button" class="page-btn" data-page="${n}" ${n===p?'aria-current="page"':''} aria-label="第 ${n} 页">${n}</button>`;prev=n;});
    html+=`<button type="button" class="page-btn" data-page="${p+1}" ${p>=pages?'disabled':''} aria-label="下一页">›</button>`;
    el('pager').innerHTML=html;el('pageJump').max=pages;el('pageJump').placeholder=String(p);
    el('pager').closest('.table-foot').hidden=!total;
  }
  function gotoPage(n){state.page=n;renderComments();const wrap=document.querySelector('.table-wrap');document.getElementById('main').scrollTo({top:Math.max(0,wrap.offsetTop-120),behavior:'smooth'});}
  function updateSelection(rows=visibleComments()) {"""),
 ("""workId:null, problemId:null,""", """workId:null, problemId:null, page:1, pageSize:10, filterSig:'',"""),
 # filter popover
 ("""    el('advancedButton').addEventListener('click',()=>{el('advancedFilters').hidden=!el('advancedFilters').hidden;el('advancedButton').setAttribute('aria-expanded',String(!el('advancedFilters').hidden));});""",
  """    const pop=el('advancedFilters'),popBtn=el('advancedButton');
    const setPop=open=>{pop.hidden=!open;popBtn.setAttribute('aria-expanded',String(open));if(open)el('minLikes').focus();};
    popBtn.addEventListener('click',()=>setPop(pop.hidden));
    el('advancedDone').addEventListener('click',()=>{setPop(false);popBtn.focus();});
    pop.addEventListener('keydown',e=>{if(e.key==='Escape'&&!document.querySelector('.sf-menu')){e.preventDefault();setPop(false);popBtn.focus();}});
    document.addEventListener('mousedown',e=>{if(!pop.hidden&&!pop.contains(e.target)&&!popBtn.contains(e.target)&&!e.target.closest('.sf-menu'))setPop(false);});
    el('pageJump').addEventListener('change',()=>{const n=Math.round(Number(el('pageJump').value));el('pageJump').value='';if(n>=1)gotoPage(Math.min(n,Number(el('pageJump').max)||1));});
    el('pageJump').addEventListener('keydown',e=>{if(e.key==='Enter')el('pageJump').dispatchEvent(new Event('change'));});
    el('pageSize').addEventListener('change',()=>{state.pageSize=Number(el('pageSize').value);renderComments();});"""),
 ("""      else if(b.id==='clearProblemFilter'){state.problemId=null;renderComments();}""",
  """      else if(b.id==='clearProblemFilter'){state.problemId=null;renderComments();}
      else if(b.classList.contains('page-btn')&&!b.disabled)gotoPage(Number(b.dataset.page));"""),
 # problems: status is a SelectField with counts in its options
 ("""    document.querySelectorAll('[data-pstate]').forEach(n=>n.setAttribute('aria-pressed',String(n.dataset.pstate===filter)));""", ""),
 ("""{const f=data.problems.filter(x=>x.state==='formal').length;el('pcAll').textContent=data.problems.length;el('pcFormal').textContent=f;el('pcCandidate').textContent=data.problems.length-f;}""",
  """{const f=data.problems.filter(x=>x.state==='formal').length,sel=el('problemStatus'),labels={all:`全部问题 · ${data.problems.length}`,formal:`正式问题 · ${f}`,candidate:`候选问题 · ${data.problems.length-f}`};[...sel.options].forEach(o=>o.textContent=labels[o.value]);sel._sfSync&&sel._sfSync();}"""),
 ("""    document.querySelectorAll('[data-pstate]').forEach(n=>n.addEventListener('click',()=>{el('problemStatus').value=n.dataset.pstate;renderProblems();}));""", ""),
])
# active-filter count on the 筛选 button
s = S.read_text()
old = """    renderPager(rows.length);\n"""
assert s.count(old) == 1
s = s.replace(old, old + """    {const n=(Number(el('minLikes').value)>0?1:0)+(el('markFilter').value!=='all'?1:0);el('advancedCount').hidden=!n;el('advancedCount').textContent=n;}\n""")
S.write_text(s)
print('ok')
