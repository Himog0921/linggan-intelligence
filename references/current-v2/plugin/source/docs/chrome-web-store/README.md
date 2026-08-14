# Chrome Web Store Submission Pack

Updated: 2026-08-12

## Current Release Candidate

- Extension: 灵感爆爆爆
- Version: 2.0.95
- Candidate only: generated below `/private/tmp/v2-xhs-content-rc-packages/`
- Future store filename after approval: `releases/linggan-boom-v2.0.95.zip`
- Upload/publish status: **not executed**

The V2 candidate must not be described as uploaded or published until the
operator creates and verifies the store release artifact. Its exact temporary
filename, SHA256 and size come from the candidate packaging report.

## Last Recorded Upload Package

- Version: 2.0.91
- Upload ZIP: `releases/linggan-boom-v2.0.91.zip`
- SHA256: `0ba19da59898694c265d7e8771fb0f8ac1008e91943dffd124e3741aaa47be4f`
- Size: `432085` bytes

The package has been checked with:

```bash
npm run build
npm run release:verify -- --version 2.0.95 --zip releases/linggan-boom-v2.0.95.zip
node --test
```

## Recommended Publishing Choice

Use **Unlisted** visibility for the first submission.

Reason: your goal is team distribution and automatic update, not public search traffic. Unlisted gives colleagues a Chrome Web Store link, Chrome handles updates automatically, and the extension will not appear in general store search. If your company has a Google Workspace domain and you want strict domain-only access, use **Private** instead.

## Dashboard Steps

1. Open Chrome Web Store Developer Dashboard.
2. Click **Add new item**.
3. After operator approval, create, verify, and upload `releases/linggan-boom-v2.0.95.zip`.
4. Fill **Store listing** using `store-listing.md`.
5. Upload images from `assets/`.
6. Fill **Privacy** using `privacy-and-permissions.md`.
7. Paste reviewer instructions from `reviewer-notes.md`.
8. Set **Distribution** to Unlisted, Free, all regions unless you want to restrict regions.
9. Submit for review.

## Files Prepared

- `store-listing.md`: store name, short summary, long description, category, language.
- `privacy-and-permissions.md`: single purpose, data disclosure, permission explanations.
- `privacy-policy.md`: public privacy policy text. A matching content workbench page has been added at `/privacy/linggan-boom-extension`; after deploying the workbench, use `https://lingganboom.fun/privacy/linggan-boom-extension`.
- `reviewer-notes.md`: instructions for Google's reviewer.
- `assets/icon-128.png`: store icon.
- `assets/screenshot-dashboard-notes-1280x800.png`: required screenshot.
- `assets/screenshot-dashboard-comments-1280x800.png`: optional screenshot.
- `assets/screenshot-dashboard-authors-1280x800.png`: optional screenshot.
- `assets/promo-small-440x280.png`: small promo tile.

## Remaining Owner Action

Google requires actions that must be done from the registered developer account:

- Upload and submit in the Developer Dashboard.
- Paste the public privacy policy URL: `https://lingganboom.fun/privacy/linggan-boom-extension`.

## Current Review Risk Notes

- The extension requests powerful permissions because it works inside 小红书 and 抖音 pages, downloads media, manages task tabs, reads platform cookies, and receives workbench push wakeups. These are explainable, but the privacy and permission fields must be filled carefully.
- The package includes `http://localhost/*` because the plugin can connect to a local content workbench during development or internal operation. Chrome treats this pattern as matching any localhost port. If the store reviewer questions this, either explain it as an internal/local workbench mode or publish a store-only build without the localhost host permission.
- The extension uses silent Web Push to wake the background worker when a workbench task or task-control event is available. This is why the `notifications` permission is present.
