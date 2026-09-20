# Linggan Intelligence Browser Package Rules

This package is Linggan-owned source migrated from `linggan-boom v2.0.91`.
It preserves source and UX, but it is **not** allowed to restore the old Content
Workbench runtime.

## Source authority

- The authoritative package location is `plugins/linggan-intelligence-browser/` in the
  current Linggan `main` or an explicitly assigned, current worktree.
- Historical `plugin-retrofit-*` directories and old-version package copies are read-only
  migration evidence. They must not receive new feature work, generate release ZIPs, or be
  presented as the current installable plugin.
- Station identity, installation replacement, claim windows, authorization, quotas, and
  Admission Gate 5 belong to Linggan's server and collection surface. This package reports
  an installation and executes approved work; it does not become the authority for either.

## Active runtime contract

- The only active service-worker entry is `src/linggan/background.js`.
- The former `src/background/`, `src/workbench/`, `src/sync/` control plane and
  remote-content handlers have been removed from this package. Historical
  retrofit directories remain read-only evidence outside this active source;
  they are not an authorization to restore old endpoints, polling/lease,
  Cookie reads, media download, or Workbench sync.
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
