# Chrome Web Store Dashboard Fill-In Cheatsheet

Updated: 2026-05-19

Use this file when the Developer Dashboard says "Unable to publish" because required fields are missing.

## Privacy Tab

### Single Purpose

Help authorized content team members collect content, comments, authors, media references, and task results from supported 小红书 and 抖音 pages, then sync those results to the 灵感爆爆爆 content workbench.

### activeTab Justification

Used to identify the current 小红书 or 抖音 page only after the user clicks the extension or a page collection control, so the extension can show the correct collection actions and read the current page context.

### alarms Justification

Used for scheduled workbench task checks, execution station heartbeat, retry timing, and daily quota reset. It is not used for advertising or unrelated background activity.

### cookies Justification

Used for user-authorized platform account switching and account health checks on supported 小红书 and 抖音 domains. Users can save or delete platform account cookies in the extension. These cookies are not sold or used for advertising.

### declarativeNetRequest Justification

Used during certain batch collection flows to temporarily block heavy image or video loading on the current target tab, reducing page load pressure. The rule is removed after collection and is not used on unrelated websites.

### declarativeNetRequestWithHostAccess Justification

Used so the temporary media-blocking rule only applies to supported hosts declared in the manifest and to the active collection tab.

### downloads Justification

Used when the user asks the extension to download collected images, videos, Live Photo files, or comment media to the local computer.

### Host Permissions Justification

The extension needs `https://lingganboom.fun/*` for authorization, task sync, result upload, media upload, and push subscription registration. It needs 小红书, 抖音, and related media hosts to collect public content, comments, author information, and media references from pages opened by the user or assigned by authorized workbench tasks. It needs `http://localhost/*` for internal local workbench mode.

### notifications Justification

Used for Manifest V3 Web Push. The content workbench can send silent task-available or task-control messages to wake the extension background worker so it checks the authorized task queue sooner. It is not used for marketing notifications.

### Remote Code Use

Select: **No**

Reason if a text field is shown:

The extension does not download or execute remotely hosted JavaScript. All executable code is packaged inside the extension zip. Network requests are used only to read supported platform data and sync with the authorized content workbench.

### scripting Justification

Used to inject the extension's packaged collection controls and helper scripts into supported 小红书 and 抖音 pages after user action or authorized workbench task dispatch. It is not used to inject ads, affiliate links, or unrelated code.

### storage Justification

Used to save local settings, collected draft data, plugin authorization state, execution station binding, task state, quota state, retry state, and sync state.

### tabs Justification

Used to find, open, update, and monitor task tabs created by the user or by the authorized content workbench, so the extension can run collection tasks and report progress for the correct page.

### Data Collection Categories

Select:

- Website content
- Web browsing activity
- Authentication information
- User activity

Do not select unless the dashboard forces a different interpretation:

- Health information
- Financial and payment information
- Personal communications
- Location

### Data Use Certification

The extension uses collected data only to provide or improve its single purpose: collecting supported platform content and syncing it to the user's authorized content workbench. Data is not sold, transferred for advertising, or used for creditworthiness, lending, or unrelated profiling.

### Privacy Policy URL

Use after the content workbench is deployed:

`https://lingganboom.fun/privacy/linggan-boom-extension`

## Store Listing Tab

### Category

Choose: Productivity

If the Chinese UI shows localized names, choose the equivalent of "效率/生产力".

### Language

Choose: Chinese Simplified

### Detailed Description

灵感爆爆爆是给内容团队使用的浏览器助手，帮助运营人员在小红书和抖音页面上采集内容素材、评论、作者信息和媒体线索，并把结果同步到灵感爆爆爆内容工作台。

它适合已经获得团队授权的内部成员使用。插件不会面向普通访客提供娱乐或广告功能，也不会替用户公开发布内容。它的核心目标是减少人工复制、截图、整理和重复下载，让团队把采集到的内容沉淀到统一的内容数据库中。

主要能力：

- 在小红书和抖音详情页识别可采集内容。
- 支持单篇内容、批量内容、评论、作者资料和媒体线索采集。
- 支持按用户操作下载图片、视频、Live Photo 或评论图片等素材。
- 支持连接灵感爆爆爆内容工作台，把采集结果同步到团队数据库。
- 支持作为工位接收工作台下发的采集任务，并把过程状态和结果回传。
- 支持授权码连接和自动工位身份，避免未授权浏览器接入团队任务。

数据使用说明：

- 插件只在用户打开或授权的目标平台页面上工作。
- 插件采集的数据用于内容整理、评论分析、素材归档和团队运营管理。
- 插件会连接灵感爆爆爆内容工作台，用于同步采集结果、授权状态、工位状态和任务进度。
- 插件不销售用户数据，不把数据用于广告投放，不把数据转让给无关第三方。

适用对象：

- 内容运营人员
- 选题研究人员
- 需要把小红书/抖音公开内容整理到团队工作台的内部成员

首次使用：

1. 从团队管理员处获取插件授权码。
2. 在插件弹窗中点击“连接插件”，完成授权并自动准备工位。
3. 打开小红书或抖音页面后，使用页面上的采集按钮或工作台任务开始采集。

## Account Settings

### Contact Email

Add a contact email in the Developer Dashboard settings page. Google requires a verified contact email before publishing.

Use the email that should receive Chrome Web Store review notices. It must be accessible because Google may send action-required emails there.
