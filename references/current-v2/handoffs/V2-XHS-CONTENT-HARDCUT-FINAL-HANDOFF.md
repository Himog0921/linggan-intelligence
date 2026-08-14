# V2 XHS Content-only Hard-cut Final Handoff

Date: 2026-08-12 (Asia/Shanghai)

## Outcome

- Candidate implementation: complete.
- Production hard cut: **NOT STARTED**.
- Blocking external fact: `https://lingganboom.fun` and both authoritative CLI checks return HTTP 530; nine-station inventory, production freeze, backup, migration, canary and 48h observation cannot be truthfully executed.
- No production/formal-test database write, push, merge, deploy, launchctl mutation or workstation install was performed.

## Immutable commit graph

- Baseline: `744deeeeb455d109565e4578679f7c4c7c8d9318`
- PRE_CUT_A: `4f8e9bfae560fedf5708f8284e5677cb7a0a8351`
- HARD_CUT_B: `8646bbc5aecb95f20f6a9748ecd5ab8a470a54ae`
- `merge-base(A,B) == A`; B is the direct child of A.
- Plugin baseline: `c22fa1b160a74b741bf56a52ebab0e6cfed9eb1a`
- Plugin candidate: `74651ca527b2efbf905bc98480a3a945f2c9ee66`

Local branches:

- `v2/xhs-content-precut-final`
- `v2/xhs-content-hardcut-final`
- plugin `v2/xhs-content-rc-final`

## A/B boundary

PRE_CUT_A contains the security expand, dedicated runtime identities, controlled audited reader, disabled durable worker/read foundation, cutover/preflight/observe/freeze machinery, tests and operator packaging. It contains no final hard-cut migration, production V2 execution route, manual/recovery caller switch, Material read switch, worker scheduler enable, old-sync rejection or proxy canary activation.

HARD_CUT_B adds the atomic final migration, three production ingress switches, strict public execution route, Material V2-only read, worker scheduler enable, legacy sync rejection, proxy/canary hard-cut integration and its attack tests.

## Plugin candidate

- Version: `2.0.92`
- Package: `/private/tmp/v2-xhs-content-rc-packages-74651c/linggan-boom-v2-xhs-content-rc-2.0.92-fbfcfdf6.zip`
- SHA-256: `fbfcfdf6bfec96438322640fb6f84f715ac71abc180a514ec3d072a69bfd8774`
- Size: 441752 bytes; 25 files. Sidecar `source.head` exactly equals the clean plugin candidate commit.
- Six real XHS producers persist explicit `captureReport`; the mapper validates/transcribes only. Zero-result is explicit observed/emitted=0; stopped/failed never infer success.

## Verified gates

- Prisma validate/generate and TypeScript: exit 0.
- Full suite on final R2 tree: 676 files / 4657 passed / 0 failed; 151 opt-in DB tests skipped by default.
- Final production build on native R2 worktree: exit 0; 154/154 pages; output trace guard 276/0 violations; runtime fingerprint `f4b4b386a55f29c307d85257cfae759f84cd12f5531fa369ab4dd1c9432a0b71` (1182 files).
- PRE_CUT_A independent checkout: default Turbopack failed only because the verification worktree used an out-of-root `node_modules` symlink; Webpack build passed 153/153, trace guard 275/0, fingerprint `a4c79c10acd71968cec40149a64c4289ec4b70afde1e82ff2645a37f3435c2be` (1179 files).
- ESLint: exit 0, 0 errors, six fixed-baseline warnings.
- Single-track scan and six cross-repository contracts: exit 0.
- `git diff --check` and deploy script syntax: exit 0.
- Governance: `BASELINE_DEBT_ACCEPTED`, exit 1 only for the unchanged TODO 229-line and existing 2024-line execution-sync service debts; no candidate-new governance issue.

## Database proofs (independent PostgreSQL 54330 only)

- Public execution route + strict version/HMAC/nonce + Evidence/durable work: `content_workbench_v2_rc_e2e_20260812_1758`.
- Public manual-import orchestration through audited reader, B2, worker, B3 and V2 read: same proof cluster, 1/1 end-to-end.
- Projection attacks: `content_workbench_b3_projection_proof_1786527780668`, 55/55.
- Pre-cut migration order/read-only reader: `content_workbench_v2_preflight_order_20260812_1840`, 1/1, including migration ledger SELECT and write rejection SQLSTATE 25006.
- Worker concurrent claim/lease/target receipt proof: 3/3; temporary proof database removed after the run.
- R2 worker transaction-fence proof: 5/5, including B2/B3 lease expiry and second-worker takeover with zero stale-owner Domain mutation.
- Runtime security provision proof: six distinct roles verified against 54330; one manifest-bound workspace completed Evidence→reader→B2→B3→Projection, and an ungranted workspace remained rejected with SQLSTATE 42501.
- Formal database `127.0.0.1:54329/content_workbench_local` was not modified.

## Operator sequence after the external 530 is repaired

1. Re-run authoritative workbench and plugin doctor checks.
2. Validate exact 9/9 station inventory and plugin package/SHA.
3. Enter continuous maintenance freeze; drain active jobs, leases, old outbox and all runners.
4. Create and independently list/restore a fresh production backup.
5. Deploy PRE_CUT_A and run `pre_cut_dark`; install the plugin offline while intake remains frozen.
6. Run authoritative `pre_cutover`; only a fresh receipt binding A, B, plugin SHA, target DB, backup, freeze, zero drain and 9/9 stations may advance.
7. Deploy HARD_CUT_B, apply the final atomic migration and run `post_cut_canary_ready` while intake remains frozen.
8. Run one allowlisted canary; verify exact Evidence→B2→B3→Material receipt and keep freeze on failure.
9. Restore intake only after canary success, then run `post_cut_observe` with real non-empty denominators.
10. Observe the XHS Content/Material slice for 48 real hours. Author/Comment/Metric, absent/unavailable, live_photo and global Release-C remain out of scope.

## Explicit blockers before operator execution

- The production deploy workflow does not yet invoke the new runtime role/DSN/workspace-grant provisioner. Adding production secrets and launchd credential installation is a high-impact security boundary change and was rejected without a separate explicit authorization. The provisioner itself and its independent database proof are complete; operator hard cut remains blocked until the workflow integration is explicitly authorized, implemented and re-reviewed.
- BLK-010 TaskStatus four new axes remain `SOURCE_INCOMPLETE`: the authoritative blocker ledger still says their terminal values and unique writer are not approved. The candidate does not invent those facts. If BLK-010 is required for this first Content slice, a business/architecture decision is still required before hard cut.
- Cloudflare Tunnel remains down with error 1033/HTTP 530 because Shadowrocket fake-IP/TUN is breaking edge port 7844. Local app and database are healthy; operator must repair the network path before any freeze or deployment.

## Review

Final independent Spec and Standards reviews are pending at the time this file was first written. Replace this section with their final immutable result before operator execution.
