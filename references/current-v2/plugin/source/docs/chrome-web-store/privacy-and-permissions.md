# Privacy And Permission Fields

## Single Purpose

Help authorized content team members collect content, comments, authors, media references, and task results from supported 小红书 and 抖音 pages, then sync those results to the 灵感爆爆爆 content workbench.

## Data Collection Disclosure

Select these data categories:

- Website content
- Web browsing activity
- Authentication information
- User activity

Explanation:

- Website content: note/video text, author information, comments, metrics, media links, and page URLs from supported 小红书 and 抖音 pages.
- Web browsing activity: supported platform URLs and page context needed to identify what the user is collecting and which workbench task is running.
- Authentication information: plugin authorization tokens, execution station tokens, push subscription endpoint and keys, and locally saved platform cookies when the user explicitly saves platform accounts in the plugin.
- User activity: collection actions, task status, progress logs, media download actions, and sync status.

Do not select:

- Health information
- Financial and payment information
- Personal communications
- Location

## Limited Use Certification

Use wording equivalent to:

The extension uses collected data only to provide or improve its single purpose: collecting supported platform content and syncing it to the user's authorized content workbench. Data is not sold, transferred for advertising, or used for creditworthiness, lending, or unrelated profiling.

## Remote Code Declaration

Select: **No, I am not using remote code.**

Reason: the package contains its own background, popup, dashboard, content, and injected scripts. The extension makes network requests to supported platforms and to `https://lingganboom.fun`, but it does not download and execute remotely hosted JavaScript.

## Permission Justifications

### activeTab

Needed when the user opens the popup or uses a page action so the extension can identify the current 小红书 or 抖音 page and show the correct collection controls.

### tabs

Needed to find, open, update, and monitor task tabs created by the user or by the authorized content workbench. This lets the extension run collection tasks and report progress for the correct page.

### storage

Needed to save local settings, collected draft data, authorization state, execution station binding, task state, quota state, and retry/backoff state.

### cookies

Needed for user-authorized platform account switching and account health checks on supported 小红书 and 抖音 domains. Cookies are used to keep the browser aligned with the selected collection account. Platform cookies are not sold or used for advertising.

### downloads

Needed when the user asks the extension to download collected images, videos, Live Photo files, or comment media to the local computer.

### alarms

Needed for scheduled task checks, execution station heartbeat, retry timing, and daily quota reset while using Manifest V3 background service workers.

### scripting

Needed to inject collection controls and helper scripts into supported 小红书 and 抖音 pages after user action or authorized workbench task dispatch.

### declarativeNetRequest

Needed to temporarily block heavy media loading during certain batch collection flows, then restore normal page loading after collection. This reduces page load pressure during collection.

### declarativeNetRequestWithHostAccess

Needed so the temporary media-blocking rule only applies to the supported target tabs and host permissions declared by the extension.

### notifications

Needed for Web Push support in Manifest V3. The content workbench can send silent task-available or task-control messages to wake the extension background worker, so the extension checks workbench tasks sooner. It is not used for ads or promotional notifications.

## Host Permission Justifications

### `https://lingganboom.fun/*`

Needed to connect the extension to the authorized content workbench for activation, station binding, task dispatch, progress reporting, result sync, media upload, and push subscription registration.

### `http://localhost/*`

Needed for local or internal workbench testing and development environments. Chrome treats this pattern as matching any localhost port. If the production store build should only serve regular colleagues, consider removing this permission in a store-only build.

### 小红书 hosts

Needed to read supported 小红书 page content, comments, author information, media references, and current login state when the user runs collection.

### 抖音 and related media hosts

Needed to read supported 抖音 page content, comments, author information, media references, video metadata, and related media files when the user runs collection.
