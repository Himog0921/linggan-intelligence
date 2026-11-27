"""Assemble 主题图谱 · 重设计对齐稿 from: the live app's global CSS + shell markup (shared with the
评论研究 B-04 build in ../_b04-build), page.css (token-only), body.html and app.js.

Usage: python3 build.py ../topic-map-redesign.html
"""
import re, sys, pathlib

P = pathlib.Path(__file__).parent
B04 = P.parent / '_b04-build'
out = pathlib.Path(sys.argv[1])

# ---- 1. global CSS from the live app (same dump and same kept modules as B-04)
dump = (B04 / 'app-css.txt').read_text()
chunks = re.split(r'(?m)^/\* (\S+) \*/\n', dump)
keep = ('styles/lids_tokens.css', 'styles/shell.css', 'styles/typography.css', 'FontNotice.module.css',
        'ContextTexture.module.css', 'AppShell.module.css', 'IntelligenceIcon.css', 'PatternWaves.css')
app_css = [f'/* {chunks[i].split("/apps/web/")[-1]} */\n' + chunks[i + 1]
           for i in range(1, len(chunks), 2) if chunks[i].endswith(keep)]
assert len(app_css) == len(keep), [c.split('\n')[0] for c in app_css]
app_css = '\n'.join(app_css)

# ---- 2. shell header: 主题图谱 is the current primary page; Z02 carries crumb + domain picker + context actions
header = (B04 / 'shell-header.html').read_text()
wave = (B04 / 'wave.txt').read_text().strip()
header, n = re.subn(r'<canvas[^>]*></canvas>', f'<img src="{wave}" alt="" style="width:1512px;height:380px;display:block">', header); assert n == 1
header, n = re.subn(r'<a href="/corpus" aria-current="page">', '<a href="/corpus">', header); assert n == 1
header, n = re.subn(r'<a href="/topics">', '<a href="/topics" aria-current="page">', header); assert n == 1
header, n = re.subn(r'<div class="v7-system-boundary">[^<]*</div>', '<div class="v7-system-boundary">本机不访问平台</div>', header); assert n == 1
crumb = ('<nav class="v7-context-crumb" aria-label="当前位置"><b class="v7-annotated-current" aria-current="page">主题图谱'
         '<svg viewBox="0 0 54 8" preserveAspectRatio="none" aria-hidden="true" focusable="false"><path d="M2 4.5C13 2 29 6 52 3.5M7 6c15-1 26-.8 39-1.4"></path></svg></b></nav>'
         '<details class="v7-domain-picker"><summary aria-label="切换当前观察领域"><span>ADHD</span><small>3,282 篇作品</small><i aria-hidden="true"></i></summary>'
         '<nav aria-label="可选观察领域"><a href="#" aria-current="page"><span>ADHD</span><small>3,282 篇作品</small></a></nav></details>')
header, n = re.subn(r'<nav class="v7-context-crumb".*?</nav>', crumb, header, flags=re.S); assert n == 1
actions = ('<div id="topic-map-context-actions" class="v7-context-actions" aria-label="图谱操作">'
           '<a class="v7-context-action" href="#">离线预演</a></div>')
header, n = re.subn(r'<div class="v7-context-meta">.*?</div>', f'<div class="v7-context-meta">{actions}</div>', header, flags=re.S); assert n == 1

# ---- 3. page
page_css = (P / 'page.css').read_text()
body = (P / 'body.html').read_text()
script = (P / 'app.js').read_text()
leftover = sorted(set(re.findall(r'#[0-9a-fA-F]{6}\b', page_css.split('/* ---------- 页面骨架')[1])))
print('hex outside token block:', leftover)

html = f'''<!doctype html>
<html lang="zh-CN" data-theme="linggan-intelligence" style="--v7-header-height:96.5px">
<head>
<meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>主题图谱 · 重设计对齐稿 · Linggan Intelligence</title>
<!-- 1. 项目现有全局样式（从运行中的应用原样导出，未改动；与评论研究 B-04 稿同一份） -->
<style>
{app_css}
</style>
<!-- 2. 本页样式：只用 --lgi-* 现有 Token 与本稿登记的 --lgi-tm-* 新组件 Token -->
<style>
{page_css}
</style>
</head>
<body>
<div id="root"><div class="v7-app topic-map-app" data-sidebar="expanded">
{header}
{body}
</div></div>
<script>
{script}
</script>
</body>
</html>
'''
out.write_text(html)
print('wrote', out, len(html))
