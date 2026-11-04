# Creator breadcrumb / React Bits Pattern Waves
Visual reference: https://reactbits.dev/tools/background-studio?bg=pattern-waves&preset=silk&spacing=10&markSize=0.6&depth=0.35&light=-23

- **Scope:** Only Corpus → Creators. Other context bars retain the existing white/grey dot texture.
- **Source:** Original React Bits Pattern Waves TypeScript GLSL implementation (unmodified shader); see `../shared/patternWaves/PatternWaves.LICENSE.md` for MIT + Commons Clause terms.
- **Default:** black strip, white breadcrumb labels, original Silk + dot lattice.
- **Preview square:** on the real creators URL append `&breadcrumbPattern=square` (or `?breadcrumbPattern=square` when it has no query), for the same shader, lighting, and crop but native square glyphs. Remove the parameter for dot. There is deliberately no extra control in the production navigation.
- **Stage/crop:** 380px high GPU canvas cropped by the compact 44px Context Bar; initial offset is center + 4px (near 48% of source height). The CSS veil keeps the left navigation reading region dark and protects the right-hand timestamp.
- **Performance/fallback:** lazy-loaded only on creators, requires WebGL2; reduced motion, mobile widths <=820px, or unavailable WebGL2 get an unobtrusive static dark strip. Original component internally observes visibility and resizes.
- **Controls:** shader layer is aria-hidden and pointer-events:none; the existing DomainPicker menu stays clickable and above the animation.
- **QA before merge:** compare dot and square on 1366/1440/1920 desktop widths, test Chrome and Safari at 100%/125%, open/close domain selection, verify tab/focus and reduced-motion/static fallback, and inspect shader cost on integrated GPU.
