# Linggan Intelligence Browser Package Rules

This package is Linggan-owned source migrated from `linggan-boom v2.0.91`.
It preserves source and UX, but it is **not** allowed to restore the old Content
Workbench runtime.

## Active runtime contract

- The only active service-worker entry is `src/linggan/background.js`.
- `src/background/index.js`, `src/workbench/`, `src/sync/`, historical probe
  scripts, and legacy collector implementation are retained migration source;
  they are not an authorization to call their old endpoints, run polling/lease,
  read Cookies, download media, or sync to the old workbench.
- Browser local storage is staging/recovery only, never Linggan truth.
- Before a named Linggan adapter contract is implemented and reviewed, a
  detail/comment/media/batch/Douyin/automation action must stay visibly pending
  and must not access a platform or write Linggan data.

## Change discipline

- Preserve user-facing placement and feedback before redesigning a legacy UI.
- Do not add old host permissions, Cookie/download/alarm/notification/network
  rule permissions, old hosts, endpoint fallbacks, or old workbench action paths.
- `npm run verify` is the package baseline. It proves build and static isolation,
  not a browser installation or real collection.
- Update `README.md` and `MIGRATION-MAP.md` whenever the active/pending
  boundary changes.
