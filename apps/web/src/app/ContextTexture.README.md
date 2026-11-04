# Shared Context Bar – React Bits Pattern Waves

Design baseline: user-approved Intelligence dark breadcrumb; this supersedes the older creator-only dot/square preview. This single implementation is mounted in **AppShell**, so Corpus (Evidence, Comment Research and Creators) and other standard pages use the same dark material. Compact workbench/immersive routes keep their existing special shells.

## Exact recorded visual settings
```json
{
  "mode":"full","preset":"pond","pattern":"square","wave":"silk",
  "characters":".:-=+*#%@","spacing":6,"markSize":0.5,
  "depth":0.35,"light":-23,"shine":0.94,"contrast":1.2,
  "speed":0.42,"scale":85,"direction":35,"opacity":1,
  "fade":"none","fadeSize":0.5,"crop":31,"brightness":73,
  "safeZone":24,"safeFade":61,"interactive":true,
  "cursorStrength":0.5,"cursorSize":160,
  "color":"#ffffff","backgroundColor":"#120f17","playing":true
}
```

React Bits preset is the starting point; the explicit `wave="silk"` and all individual parameters take precedence over the recorded `preset="pond"`. The original PatternWaves TS/WebGL2/OGL shader stays in `../shared/patternWaves` (including its MIT + Commons Clause license). There is no video or Canvas 2D visual substitute.

Shader `scale` receives `0.85` (Studio's 85); 380px vertical stage is translated by -31% in CSS; brightness 73 becomes a 0.73 stage filter. `mode=full` makes the saved `safeZone=24` and `safeFade=61` inactive. A slight optical veil protects legibility, rather than hiding half the texture.

## Navigation integration
- Context Bar maintains its previous height, grid, breadcrumb, tertiary tabs, and the orange selected underline.
- Breadcrumb text is white/soft grey. Right-side status is light grey.
- DomainPicker behavior/routing remains unchanged. Its summary is now **white background, black text**, 2px vertical padding, content-driven width, with the compact grey checker tail and black chevron **retained**. The menu opens on white and maintains correct focus.
- Only the old creator-only URL preview `breadcrumbPattern=square` is retired; the final default is square everywhere.
- The texture layer is aria-hidden, noninteractive, and below all navigation controls; mouse ripple is still handled by upstream's global pointer observer.
- Reduced-motion/mobile/unsupported WebGL2 see a static dark strip, not an invented lower-quality texture.

## Pre-merge checks
1. `npm run typecheck && npm run lint && npm run test && npm run build` in apps/web (and LIDS check).
2. Verify Context Bar on /corpus/creators, /corpus/evidence, all /corpus/comments views, plus Topics/Collection/Ops standard pages.
3. Check domain menu with ADHD, multiword and long domain names; verify click, Escape, focus, switching domain and view tabs without losing context.
4. 1366, 1440, 1920, 820px widths; browser zoom 100/125%; Chrome/Safari WebGL2 and reduced-motion fallback; inspect GPU cost.
5. Validate that the dynamic texture never blocks clicks and does not change measured global header height.

Upstream: https://reactbits.dev/tools/background-studio?bg=pattern-waves&preset=silk&spacing=10&markSize=0.6&depth=0.35&light=-23
