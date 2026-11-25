"""Assemble 评论研究 B-04 对齐稿 from: the B-03 demo (content, data, behaviour, images),
the live app's shell markup + global CSS (dumped from the running dev app), and page.css (token-only)."""
import re, sys, pathlib

P = pathlib.Path(__file__).parent
demo = (P / 'demo' / 'd.html').read_text()
out = pathlib.Path(sys.argv[1])

# ---- 1. global CSS from the live app (tokens, shell, typography, shared modules used by the shell)
dump = (P / 'app-css.txt').read_text()
chunks = re.split(r'(?m)^/\* (\S+) \*/\n', dump)
keep = ('styles/lids_tokens.css', 'styles/shell.css', 'styles/typography.css', 'FontNotice.module.css',
        'ContextTexture.module.css', 'AppShell.module.css', 'IntelligenceIcon.css', 'PatternWaves.css')
app_css = []
for i in range(1, len(chunks), 2):
    if chunks[i].endswith(keep):
        app_css.append(f'/* {chunks[i].split("/apps/web/")[-1]} */\n' + chunks[i + 1])
assert len(app_css) == len(keep), [c.split('\n')[0] for c in app_css]
app_css = '\n'.join(app_css)

# ---- 2. image tokens (data URIs) from the demo
img_roots = re.findall(r':root\{--pa-[^}]*\}', demo)
assert len(img_roots) == 2
img_css = '\n'.join(img_roots)

# ---- 3. shell: header (static wave, records entry in place of meta) + sidebar (no 研究记录)
header = (P / 'shell-header.html').read_text()
wave = (P / 'wave.txt').read_text().strip()
header, n = re.subn(r'<canvas[^>]*></canvas>', f'<img src="{wave}" alt="" style="width:1512px;height:380px;display:block">', header); assert n == 1
header, n = re.subn(r'<b class="v7-annotated-current" aria-current="page">概览', '<b class="v7-annotated-current" aria-current="page"><span id="crumbView">概览</span>', header); assert n == 1
entry = ('<button type="button" class="record-entry" id="recordsEntry" aria-haspopup="dialog" aria-controls="recordsDialog" aria-expanded="false">'
         '<span data-icon="History"></span><span>研究记录</span><span class="entry-status" id="recordBadge"></span></button>')
header, n = re.subn(r'<div class="v7-context-meta">.*?</div>', f'<div class="v7-context-meta">{entry}</div>', header); assert n == 1

side = (P / 'shell-side.html').read_text()
side, n = re.subn(r'<a class="v7-side-subnav-item"[^>]*data-code="RUN">研究记录</a>', '', side); assert n == 1
for view in ('overview', 'comments', 'problems'):
    side, n = re.subn(rf'(<a class="v7-side-subnav-item") href="[^"]*view={view}"', rf'\1 href="#{view}" data-view="{view}"', side); assert n == 1, view
side = side.replace('评论理解 · 候选整理', '评论理解 · 用户问题')

# ---- 4. page body + behaviour from the demo
body = demo[demo.index('<main id="main"'):demo.index('</main>') + len('</main>')]
dialogs = demo[demo.index('<dialog class="inspector"'):demo.index('<script>')]
scripts = demo[demo.index('<script>'):demo.rindex('</body>')]

body = body.replace('<main id="main" tabindex="-1">', '<main id="main" class="cr-main" tabindex="-1" aria-labelledby="comment-study-title">', 1)
# keep the live page's yesterday deltas (demo dropped them; they carry information shown nowhere else)
body, n = re.subn(r'<div class="metric-sub"><span id="metricRaw"></span></div>',
                  '<div class="metric-sub"><span id="metricRaw"></span><span class="delta" title="上一完整自然日首次被观察的评论身份">↗ +8<small>昨日首次观察</small></span></div>', body); assert n == 1
body, n = re.subn(r'<div class="metric-sub"><span>来自当前有效结果</span></div>',
                  '<div class="metric-sub"><span>来自当前有效结果</span></div><div class="metric-sub"><span class="delta">↗ +6<small>昨日写入</small></span></div>', body); assert n == 1
body, n = re.subn(r'(<div class="metric-sub"><span id="metricProblemState"></span></div>)',
                  r'\1<div class="metric-sub"><span class="delta">↗ +1<small>昨日建档</small></span></div>', body); assert n == 1
# native filters adopt the shared field look
dialogs = dialogs.replace('class="records-search-field"', 'class="field records-search-field"').replace('class="records-filter-field"', 'class="field records-filter-field"')
# demo-only colours that live inside the JS templates → tokens
scripts = scripts.replace("root.querySelectorAll('[data-icon]')", "root.querySelectorAll('span[data-icon]')")
# Chrome ignores calc(% - px) on <col>; plain percentages + a fixed 34px check column lay out correctly
old = "(state.selecting?'calc('+p+'% - '+(p*34/100)+'px)':p+'%')"
assert old in scripts; scripts = scripts.replace(old, "p+'%'")
# the live app scrolls <main>, not the window
scripts = scripts.replace('window.scrollY', "document.getElementById('main').scrollTop").replace('window.scrollTo(', "document.getElementById('main').scrollTo(")
scripts = (scripts.replace('stop-color="#e93616"', 'stop-color="var(--lgi-signal)"')
           .replace('stroke="#e5eae9"', 'stroke="var(--lgi-hairline)"')
           .replace('fill="#596e75"', 'fill="var(--lgi-muted)"')
           .replace('fill="#101a1d"', 'fill="var(--lgi-ink)"')
           .replace('stroke="#df3518"', 'stroke="var(--lgi-signal)"')
           .replace("'#333b39'", "'var(--lgi-cr-particle-dark)'").replace("'#e8e9e1'", "'var(--lgi-cr-particle-light)'").replace("'#e95a35'", "'var(--lgi-signal-mark)'")
           .replace('<g fill="#151a1b"', '<g style="fill:var(--lgi-cr-banner-pixel)"')
           .replace('fill="${fill}"', 'style="fill:${fill}"')
           .replace("'#475857'", "'var(--lgi-body)'").replace("'#e2a900'", "'var(--lgi-warning-dot)'").replace("'#e7ebec'", "'var(--lgi-unknown-soft)'"))
# SVG presentation attributes do not resolve var(); move them to style
scripts = re.sub(r'(stop-color|stroke|fill)="(var\(--lgi-[a-z0-9-]+\))"', r'style="\1:\2"', scripts)
scripts = re.sub(r"fill=\"\$\{r\.coverage==='完整'\?'(var\([^)]*\))':r\.coverage==='部分'\?'(var\([^)]*\))':'(var\([^)]*\))'\}\"",
                 r"""style="fill:${r.coverage==='完整'?'\1':r.coverage==='部分'?'\2':'\3'}" """, scripts)
leftover = re.findall(r'#[0-9a-fA-F]{6}\b', scripts)
print('hex left in scripts:', sorted(set(leftover)))

import os
if not (P / 'src-body.html').exists():
    (P / 'src-body.html').write_text(body); (P / 'src-dialogs.html').write_text(dialogs); (P / 'src-scripts.html').write_text(scripts)
body = (P / 'src-body.html').read_text(); dialogs = (P / 'src-dialogs.html').read_text(); scripts = (P / 'src-scripts.html').read_text()
page_css = (P / 'page.css').read_text()
html = f'''<!doctype html>
<html lang="zh-CN" data-theme="linggan-intelligence" style="--v7-header-height:96.5px">
<head>
<meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>评论研究 · B-04 对齐稿 · Linggan Intelligence</title>
<!-- 1. 项目现有全局样式（从运行中的应用原样导出，未改动） -->
<style>
{app_css}
</style>
<!-- 2. 本页样式：只用 --lgi-* 现有 Token 与本稿登记的 --lgi-cr-* 新组件 Token -->
<style>
{page_css}
{img_css}
</style>
</head>
<body>
<div id="root"><div class="v7-app" data-sidebar="expanded">
{header}
<div class="v7-shell">
{side}
{body}
</div></div></div>
{dialogs}
{scripts}
</body>
</html>
'''
out.write_text(html)
print('wrote', out, len(html))
