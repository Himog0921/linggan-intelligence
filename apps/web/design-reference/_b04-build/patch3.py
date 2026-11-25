import pathlib, re
P = pathlib.Path('.')
def edit(name, pairs):
    f = P / name; s = f.read_text()
    for old, new in pairs:
        assert s.count(old) == 1, (name, old[:90], s.count(old))
        s = s.replace(old, new)
    f.write_text(s)

# ===== 1. chart drawn at its real pixel size (no stretched text) =====
edit('src-scripts.html', [
("""cMax=ceilMax(maxC),sMax=ceilMax(maxS),left=44,right=952,top=20,bottom=145,w=right-left,h=bottom-top,step=w/rows.length;""",
 """cMax=ceilMax(maxC),sMax=ceilMax(maxS),W=Math.max(320,Math.round(el('trendChart').clientWidth||1000)),H=Math.max(150,Math.round(el('trendChart').clientHeight||194)),left=40,right=W-40,top=30,bottom=H-46,w=right-left,h=bottom-top,step=w/rows.length,band=bottom+9,tick=H-10;"""),
("""<svg viewBox="0 0 1000 190" preserveAspectRatio="none" role="img\"""", """<svg viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" role="img\""""),
("""<g font-size="12" font-family="inherit" style="fill:var(--lgi-muted)"><text x="0" y="11">评论</text><text x="967" y="11">信号</text>${[0,.5,1].map(f=>`<path d="M${left} ${bottom-h*f}H${right}" style="stroke:var(--lgi-hairline)" stroke-width="1"/><text x="32" y="${bottom-h*f+4}" text-anchor="end">${cMax*f}</text><text x="964" y="${bottom-h*f+4}">${sMax*f}</text>`).join('')}</g>""",
 """<g class="chart-axis"><text x="0" y="12">评论</text><text x="${W}" y="12" text-anchor="end">信号</text>${[0,.5,1].map(f=>`<path d="M${left} ${bottom-h*f}H${right}" style="stroke:var(--lgi-hairline)" stroke-width="1"/><text x="${left-8}" y="${bottom-h*f+4}" text-anchor="end">${cMax*f}</text><text x="${right+8}" y="${bottom-h*f+4}">${sMax*f}</text>`).join('')}</g>"""),
("""stroke-width="2" vector-effect="non-scaling-stroke" fill="none"/>""", """stroke-width="2" fill="none"/>"""),
("""<rect x="${left+i*step}" y="154" width="${Math.max(1,step-1)}" height="5\"""", """<rect x="${left+i*step}" y="${band}" width="${Math.max(1,step-1)}" height="5\""""),
("""<g font-size="12" style="fill:var(--lgi-muted)">${tickIndexes.map(i=>`<text x="${left+(i+.5)*step}" y="181\"""", """<g class="chart-axis">${tickIndexes.map(i=>`<text x="${left+(i+.5)*step}" y="${tick}\""""),
# redraw on resize
("""    refreshAll();
  }
  return {data,""", """    refreshAll();
    if('ResizeObserver' in window){let last=0;new ResizeObserver(entries=>{const w=Math.round(entries[0].contentRect.width);if(w&&w!==last){last=w;if(state.chartMode==='chart')renderTrend();}}).observe(el('trendChart'));}
  }
  return {data,"""),
])

# ===== 2. overview as two aligned rows (rules line up across columns) =====
b = (P / 'src-body.html').read_text()
old_top = b[b.index('<div class="overview-top">'):b.index('<div class="overview-lower">')]
m = re.match(r'<div class="overview-top">\n<div class="metric-col">(.*?)</div>\n<div class="chart-col">(.*?)</div>\n</div>\n$', old_top, re.S)
assert m, 'overview-top shape'
left, right = m.group(1), m.group(2)
li = left.index('<div class="metric-pair">'); left_a, left_b = left[:li], left[li:]
ri = right.index('<div class="expression">'); right_a, right_b = right[:ri], right[ri:]
assert right_b.endswith('</div>') 
new_top = ('<div class="overview-top">\n'
           f'<div class="ov-cell metric-col">{left_a}</div>\n'
           f'<div class="ov-cell chart-col">{right_a}</div>\n'
           f'<div class="ov-cell ov-row2 metric-col">{left_b}</div>\n'
           f'<div class="ov-cell ov-row2 chart-col">{right_b}</div>\n'
           '</div>\n')
b = b.replace(old_top, new_top)

# ===== 3. problems: no duplicate header; one toolbar row =====
old = b[b.index('<header class="section-head"><h1 id="problemsTitle">'):b.index('<div class="pa-grid" id="problemGrid">')]
new = ('<h1 class="sr-only" id="problemsTitle">用户问题</h1><div class="pa-page"><div class="pa-toolbar">'
       '<div class="search-shell pa-search-shell"><button type="button" class="find" id="problemSearchButton">搜索</button>'
       '<input id="problemSearch" type="search" aria-label="搜索问题名称或描述" placeholder="搜索问题名称或描述"></div>'
       '<div class="pa-controls"><div class="seg" role="group" aria-label="问题状态">'
       '<button type="button" data-pstate="all" aria-pressed="true">全部 <b id="pcAll"></b></button>'
       '<button type="button" data-pstate="formal" aria-pressed="false" title="正式问题：至少 2 位不同作者提到">正式 <b id="pcFormal"></b></button>'
       '<button type="button" data-pstate="candidate" aria-pressed="false" title="候选问题：目前只有 1 位作者提到">候选 <b id="pcCandidate"></b></button></div>'
       '<select id="problemStatus" hidden aria-hidden="true" tabindex="-1"><option value="all">全部</option><option value="formal">正式</option><option value="candidate">候选</option></select>'
       '<div class="inline-field"><label for="problemSort">排序</label><select id="problemSort"><option value="recent">最新研究</option><option value="authors">作者数量</option><option value="comments">评论数量</option><option value="works">作品数量</option></select></div>'
       '</div></div><p class="pa-result" id="problemCount" aria-live="polite"></p>')
b = b.replace(old, new)
(P / 'src-body.html').write_text(b)

edit('src-scripts.html', [
("""el('problemCount').textContent=`显示 ${rows.length} / ${data.problems.length} 个问题`;""",
 """el('problemCount').textContent=q?`找到 ${rows.length} 个问题`:'';el('problemCount').hidden=!q;
    document.querySelectorAll('[data-pstate]').forEach(n=>n.setAttribute('aria-pressed',String(n.dataset.pstate===filter)));"""),
("""{const f=data.problems.filter(x=>x.state==='formal').length;el('problemsMeta').innerHTML=`正式 <b>${f}</b> · 候选 <b>${data.problems.length-f}</b><span class="meta-split">正式问题：至少 2 位不同作者提到</span>`;}""",
 """{const f=data.problems.filter(x=>x.state==='formal').length;el('pcAll').textContent=data.problems.length;el('pcFormal').textContent=f;el('pcCandidate').textContent=data.problems.length-f;}"""),
("""el('problemSearchButton').addEventListener('click',()=>{renderProblems();el('problemSearch').focus();});""",
 """el('problemSearchButton').addEventListener('click',()=>{renderProblems();el('problemSearch').focus();});
    document.querySelectorAll('[data-pstate]').forEach(n=>n.addEventListener('click',()=>{el('problemStatus').value=n.dataset.pstate;renderProblems();}));"""),
])

# ===== 4. every <select> becomes the project SelectField =====
sf = r"""
/* SelectField（与 apps/web/src/shared/SelectField 同一交互与样式；落地时直接用组件） */
(function(){
  const desc=Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype,'value');
  Object.defineProperty(HTMLSelectElement.prototype,'value',{configurable:true,get(){return desc.get.call(this);},set(v){desc.set.call(this,v);if(this._sfSync)this._sfSync();}});
  let open=null,seq=0;
  function close(focus){if(!open)return;const o=open;open=null;o.menu.remove();o.trigger.setAttribute('aria-expanded','false');o.trigger.removeAttribute('aria-activedescendant');if(focus)o.trigger.focus({preventScroll:true});}
  function choose(o,i){const opt=o.select.options[i];if(!opt||opt.disabled)return;if(o.select.value!==opt.value){o.select.value=opt.value;o.select.dispatchEvent(new Event('change',{bubbles:true}));}close(true);}
  function paint(o){[...o.menu.children].forEach((n,i)=>n.dataset.active=String(i===o.active));const n=o.menu.children[o.active];if(n){o.trigger.setAttribute('aria-activedescendant',n.id);n.scrollIntoView({block:'nearest'});}}
  function show(select,trigger){
    close(false);const id='sf-'+(++seq),menu=document.createElement('div');menu.className='sf-menu';menu.id=id;menu.setAttribute('role','listbox');
    const o={select,trigger,menu,active:Math.max(0,select.selectedIndex)};
    [...select.options].forEach((opt,i)=>{const n=document.createElement('div');n.id=id+'-'+i;n.setAttribute('role','option');n.setAttribute('aria-selected',String(i===select.selectedIndex));if(opt.disabled)n.setAttribute('aria-disabled','true');n.innerHTML='<span></span><span aria-hidden="true"></span>';n.firstChild.textContent=opt.textContent;n.lastChild.textContent=i===select.selectedIndex?'✓':'';n.addEventListener('mousedown',e=>e.preventDefault());n.addEventListener('click',()=>choose(o,i));n.addEventListener('mousemove',()=>{o.active=i;paint(o);});menu.appendChild(n);});
    (trigger.closest('dialog')||document.body).appendChild(menu);
    const r=trigger.getBoundingClientRect();menu.style.minWidth=r.width+'px';
    const below=window.innerHeight-r.bottom;menu.style.left=Math.min(r.left,window.innerWidth-menu.offsetWidth-12)+'px';
    menu.style.top=(below<menu.offsetHeight+12&&r.top>menu.offsetHeight?r.top-menu.offsetHeight-4:r.bottom+4)+'px';
    trigger.setAttribute('aria-expanded','true');trigger.setAttribute('aria-controls',id);open=o;paint(o);
  }
  function enhance(select){
    if(select._sfSync||select.hidden)return;
    const wrap=document.createElement('span');wrap.className='sf-select';if(select.closest('.search-shell'))wrap.dataset.compact='true';
    const trigger=document.createElement('button');trigger.type='button';trigger.className='sf-trigger';trigger.setAttribute('role','combobox');trigger.setAttribute('aria-haspopup','listbox');trigger.setAttribute('aria-expanded','false');
    trigger.innerHTML='<span class="sf-value"></span><span class="sf-chevron" aria-hidden="true">⌄</span>';
    const label=select.id&&document.querySelector('label[for="'+select.id+'"]');
    trigger.setAttribute('aria-label',select.getAttribute('aria-label')||(label?label.textContent.trim():''));
    if(label&&select.id){trigger.id=select.id+'-trigger';label.htmlFor=trigger.id;}
    select.after(wrap);wrap.append(trigger,select);select.classList.add('sf-native');select.tabIndex=-1;select.setAttribute('aria-hidden','true');
    select._sfSync=()=>{const opt=select.options[select.selectedIndex];trigger.querySelector('.sf-value').textContent=opt?opt.textContent:'';trigger.querySelector('.sf-value').title=opt?opt.textContent:'';};
    select.addEventListener('change',select._sfSync);select._sfSync();
    trigger.addEventListener('click',()=>open&&open.trigger===trigger?close(true):show(select,trigger));
    trigger.addEventListener('keydown',e=>{
      if(!open||open.trigger!==trigger){if(['ArrowDown','ArrowUp','Enter',' '].includes(e.key)){e.preventDefault();show(select,trigger);}return;}
      const o=open,n=select.options.length;
      if(e.key==='ArrowDown'){e.preventDefault();o.active=(o.active+1)%n;paint(o);}
      else if(e.key==='ArrowUp'){e.preventDefault();o.active=(o.active-1+n)%n;paint(o);}
      else if(e.key==='Home'){e.preventDefault();o.active=0;paint(o);}else if(e.key==='End'){e.preventDefault();o.active=n-1;paint(o);}
      else if(e.key==='Enter'||e.key===' '){e.preventDefault();choose(o,o.active);}
      else if(e.key==='Escape'){e.preventDefault();e.stopPropagation();close(true);}
      else if(e.key==='Tab')close(false);
    });
  }
  document.addEventListener('mousedown',e=>{if(open&&!open.menu.contains(e.target)&&!open.trigger.contains(e.target))close(false);},true);
  document.addEventListener('scroll',e=>{if(open&&!open.menu.contains(e.target))close(false);},true);
  window.addEventListener('resize',()=>close(false));
  window.SelectField={enhanceAll(root=document){root.querySelectorAll('select').forEach(enhance);}};
})();
"""
s = (P / 'src-scripts.html').read_text()
anchor = "window.App = (function () {"
assert s.count(anchor) == 1
s = s.replace(anchor, sf + anchor)
old = """    fillIcons();initializeModals();ResearchRecords.init();"""
assert s.count(old) == 1
s = s.replace(old, """    fillIcons();initializeModals();ResearchRecords.init();SelectField.enhanceAll();""")
(P / 'src-scripts.html').write_text(s)
print('ok')
