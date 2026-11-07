# Creator Workbench G4/G5 continuation — verification ledger

> 状态: 草案
> 最后核对: 2026-11-07
> 适用范围: 创作工作台 G4 隔离预览及 G5 浏览器验收，PR #549
> 事实来源: Gold Master SHA-256 c1e564b38811bee179154ac3b2af1847abceab1fa5f44fb67a81d5b958f6350e；批准的 G3 SPEC；真实仓库代码及 CI
> 冲突时以谁为准: 真实代码、CI 和同条件浏览器证据；文档声明不可替代通过
> Authority: G3 SPEC, original HTML SHA-256 `c1e564b38811bee179154ac3b2af1847abceab1fa5f44fb67a81d5b958f6350e`.
> Branch: `preview/creator-workbench-g4-g5-verify-20261107`; PR #549; production `/ops/creation` untouched.

## G4 closure evidence

- LIDS: page-local style declarations rewritten to existing runtime tokens / token arithmetic; no exceptions added to the unit test.
- Shell: removed page-level `.v7-context-crumb` selector; the two previously authorized page-scoped header variables remain.
- Tests: added keyboard inspector navigation, focus return, canvas-card event isolation and collision-free local item ids.
- Full typecheck/lint/Vitest/build/LIDS CI results are authoritative, not this text.

## G5 browser proof

The G5 proof is routed through the existing `Delivery → web` reusable workflow. There is no independent PR-triggered G5 workflow. Mobile drawer close actions stay inside their respective overlay and must be keyboard-reachable.

`node apps/web/test/e2e/creator-workbench-g5.mjs` starts the *real* Vite development route on localhost, blocks all API calls and third-party requests, and records:
- six reference viewport sizes: 1440×1000, 1280×720, 1024×768, 820×900, 390×844, 375×812;
- shared-shell/sidebar/queue/editor/inspector geometry versus frozen 1440 contract (±2px);
- six screenshots, desktop asset dialog and materials desk, mobile inspector and focus/keyboard checks;
- page errors, prohibited network calls and any failed assertions, even when the job exits nonzero.

Do not treat this smoke as VISUAL_VERIFIED or BEHAVIOR_VERIFIED. Full acceptance still requires side-by-side screenshots of the **original Gold Master** and **actual React19** in identical browser/font/state conditions, 40 SPEC journeys, 20 negative cases, 8 a11y categories, and no unapproved P0/P1 deviations. Source screenshots and personal data do not enter the repository.

## Gate decision

G4: pending authoritative CI.
G5: NOT ACCEPTED until screenshot and interaction matrices are complete.
G6: NOT STARTED; no production API, merge or deployment.
