import pathlib
f = pathlib.Path('page.css'); s = f.read_text()
def rep(old, new):
    global s
    assert s.count(old) == 1, (old[:80], s.count(old))
    s = s.replace(old, new)
# cool grey card foot instead of cream
rep("  --lgi-cr-card-foot-tint: #fffefa;      /* 卡片正文底部的暖白收尾 */\n", "")
rep("var(--lgi-canvas) 78%,var(--lgi-cr-card-foot-tint) 100%)", "var(--lgi-canvas) 72%,var(--lgi-canvas-low) 100%)")
# overview: two shared rows so horizontal rules align across columns
rep(".overview-top{display:grid;grid-template-columns:var(--lgi-cr-metric-col) minmax(0,1fr);gap:var(--lgi-cr-overview-gap);padding-top:var(--lgi-space-6);align-items:start}",
    """.overview-top{display:grid;grid-template-columns:var(--lgi-cr-metric-col) minmax(0,1fr);column-gap:var(--lgi-cr-overview-gap);row-gap:0;padding-top:var(--lgi-space-6);align-items:start}
.ov-cell{min-width:0}
.ov-row2{margin-top:var(--lgi-space-5);padding-top:var(--lgi-space-5);border-top:1px solid var(--lgi-hairline);align-self:stretch;display:flex;flex-direction:column}
.ov-row2.metric-col .candidate-link{margin-top:auto}
.ov-row2 .expression{margin:0;padding:0;border:0;display:flex;flex-direction:column;flex:1}
.ov-row2 .expression .wordcloud{flex:1}
.ov-cell.metric-col:not(.ov-row2) .metric-block:last-child{border-bottom:0;margin-bottom:0;padding-bottom:0}""")
rep(".data-stamp{display:flex;align-items:center;justify-content:space-between;gap:var(--lgi-space-3);color:var(--lgi-muted);font-size:var(--lgi-text-label);margin-bottom:var(--lgi-space-5)}",
    ".data-stamp{display:flex;align-items:center;justify-content:space-between;gap:var(--lgi-space-3);min-height:var(--lgi-space-8);color:var(--lgi-muted);font-size:var(--lgi-text-label);margin-bottom:var(--lgi-space-3)}")
# chart text at true size
rep(".chart-wrap svg{width:100%;height:100%;display:block;overflow:visible;font-family:var(--lgi-font-data)}",
    ".chart-wrap svg{width:100%;height:100%;display:block;overflow:visible}\n.chart-axis{fill:var(--lgi-muted);font:var(--lgi-weight-regular) var(--lgi-text-calibration) var(--lgi-font-data)}")
# problems toolbar: search left, status segments + sort right, one 40px line
rep(".pa-toolbar{height:62px;display:flex;align-items:center;justify-content:space-between;gap:var(--lgi-space-5)}",
    """.pa-toolbar{display:flex;align-items:center;justify-content:space-between;gap:var(--lgi-space-5);padding:var(--lgi-space-6) 0 var(--lgi-space-4)}
.pa-controls{display:flex;align-items:center;gap:var(--lgi-space-4);flex:none}
.seg{display:inline-flex;height:var(--lgi-action-height);border:var(--lgi-action-border-width) solid var(--lgi-ink)}
.seg button{border:0;border-left:var(--lgi-action-border-width) solid var(--lgi-ink);background:var(--lgi-canvas);color:var(--lgi-body);padding:0 var(--lgi-space-4);font:var(--lgi-type-control-weight) var(--lgi-type-control-size)/1 var(--lgi-font-sans);display:inline-flex;align-items:center;gap:var(--lgi-space-2)}
.seg button:first-child{border-left:0}
.seg button:hover{background:var(--lgi-canvas-low)}
.seg button b{font-family:var(--lgi-font-data);color:var(--lgi-muted)}
.seg button[aria-pressed=true]{background:var(--lgi-ink);color:var(--lgi-on-dark)}
.seg button[aria-pressed=true] b{color:var(--lgi-signal-mark)}
.inline-field{display:flex;align-items:center;gap:var(--lgi-space-2)}
.inline-field label{font-size:var(--lgi-text-label);color:var(--lgi-muted);white-space:nowrap}
.inline-field .sf-select{width:136px}
.pa-result{margin:0 0 var(--lgi-space-3);font-size:var(--lgi-text-label);color:var(--lgi-muted)}""")
rep(".pa-page{padding:var(--lgi-space-2) 0 var(--lgi-space-8)}", ".pa-page{padding:0 0 var(--lgi-space-8)}")
rep(".pa-search-shell{width:min(430px,43%);min-width:230px}", ".pa-search-shell{flex:1;max-width:520px;min-width:230px}")
rep("@media(max-width:980px){.pa-grid{grid-template-columns:1fr;max-width:760px;margin:0 auto}.pa-toolbar{height:auto;align-items:flex-start;flex-direction:column;padding:var(--lgi-space-3) 0}",
    "@media(max-width:980px){.pa-grid{grid-template-columns:1fr;max-width:760px;margin:0 auto}.pa-toolbar{align-items:stretch;flex-direction:column}.pa-controls{justify-content:space-between}")
# SelectField port (same tokens as apps/web/src/shared/SelectField.module.css)
rep("/* 选择框：与共享 SelectField 同形（落地时直接用 SelectField 组件） */", """/* 选择框：apps/web/src/shared/SelectField 的样式原样移植（落地时直接用组件） */
.sf-select{position:relative;display:inline-flex;min-width:0;max-width:100%}
.field .sf-select{display:flex;width:100%}
.sf-native{position:absolute!important;inset:0;width:1px;height:1px;opacity:0;pointer-events:none}
.sf-trigger{position:relative;display:flex;align-items:center;justify-content:space-between;gap:var(--lgi-space-3);width:100%;min-width:0;min-height:var(--lgi-action-height);border:var(--lgi-action-border-width) solid var(--lgi-ink);padding:0 var(--lgi-space-3);border-radius:var(--lgi-radius-none);background:var(--lgi-canvas);color:var(--lgi-ink);font:var(--lgi-type-control-weight) var(--lgi-type-control-size)/var(--lgi-type-control-leading) var(--lgi-font-sans);cursor:pointer;text-align:left}
.sf-trigger::after{content:'';position:absolute;right:0;top:0;bottom:0;width:var(--lgi-secondary-tail);background:var(--lgi-action-lattice-ink) 0 0/var(--lgi-secondary-pitch) var(--lgi-secondary-pitch);pointer-events:none}
.sf-value{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.sf-trigger[aria-expanded=true]{background:var(--lgi-focus-warm)}
.sf-trigger:focus-visible{outline:var(--lgi-focus-inset) solid var(--lgi-focus);outline-offset:var(--lgi-action-focus-offset)}
.sf-chevron{flex:none;margin-right:var(--lgi-secondary-tail)}
.sf-select[data-compact] .sf-trigger{min-height:var(--lgi-space-8);height:100%;border:0;border-right:1px solid var(--lgi-hairline);padding:0 var(--lgi-space-3);background:transparent;width:116px}
.sf-select[data-compact] .sf-trigger:focus-visible{outline:none;box-shadow:inset 0 calc(-1 * var(--lgi-focus-inset)) 0 var(--lgi-ink)}
.sf-menu{width:max-content;max-width:calc(100vw - var(--lgi-space-6));max-height:min(320px,60vh);position:fixed;box-sizing:border-box;margin:0;padding:var(--lgi-space-1);border:var(--lgi-action-border-width) solid var(--lgi-ink);border-radius:var(--lgi-radius-none);background:var(--lgi-canvas);color:var(--lgi-ink);box-shadow:var(--lgi-action-shadow-idle) var(--lgi-action-shadow-idle) 0 var(--lgi-ink);overflow:auto;overscroll-behavior:contain;z-index:var(--lgi-z-popover);font:var(--lgi-type-control-weight) var(--lgi-type-control-size)/var(--lgi-type-control-leading) var(--lgi-font-sans)}
.sf-menu>[role=option]{display:flex;align-items:center;justify-content:space-between;gap:var(--lgi-space-5);min-height:var(--lgi-space-10);padding:var(--lgi-space-2) var(--lgi-space-3);box-sizing:border-box;cursor:pointer}
.sf-menu>[aria-selected=true]{background:var(--lgi-ink);color:var(--lgi-on-dark)}
.sf-menu>[data-active=true]:not([aria-selected=true]){background:var(--lgi-focus-warm);box-shadow:inset var(--lgi-focus-inset) 0 var(--lgi-focus)}
.sf-menu>[aria-selected=true][data-active=true]{box-shadow:inset var(--lgi-focus-inset) 0 var(--lgi-signal-mark)}
.sf-menu>[aria-disabled=true]{color:var(--lgi-ghost);cursor:not-allowed}
.sf-menu>[role=option]>span:first-child{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}""")
f.write_text(s)
print('ok')
