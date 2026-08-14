# Reviewer Notes

This extension is a browser assistant for an authorized content operations team. It works on supported 小红书 and 抖音 pages and syncs collected results to the team's content workbench.

## Test Account And Access

The extension requires an authorization code from the content workbench before collection, sync, or station binding is enabled.

If reviewer testing needs access, provide a temporary authorization code and, if needed, a temporary execution station pairing code from the content workbench.

## How To Test The Main Flow

1. Install the extension.
2. Open the extension popup.
3. Enter the provided authorization code.
4. Open a supported 小红书 or 抖音 content page.
5. Use the page collection controls or popup controls to collect content.
6. Confirm the collected content appears in the extension dashboard.
7. If a workbench test account is provided, sync the collected result to `https://lingganboom.fun`.

## What The Extension Does

- Reads supported 小红书 and 抖音 pages selected by the user or by an authorized workbench task.
- Collects public content fields, comments, author fields, media references, and task result data.
- Downloads media only when the user asks for it or when a workbench task requires media collection.
- Sends collected results and task status to the authorized content workbench.
- Registers a Web Push subscription so the workbench can wake the extension when new authorized tasks or task-control events are available.

## What The Extension Does Not Do

- It does not load or execute remotely hosted JavaScript.
- It does not inject ads, affiliate links, or shopping codes.
- It does not sell user data.
- It does not collect health, financial, payment, or precise location data.
- It does not publish content to social platforms.

## Permission Notes

The extension requests broad-looking permissions because it must operate inside supported social content pages, manage task tabs, read user-authorized platform cookies, download requested media, and communicate with the content workbench.

The `notifications` permission is used for Manifest V3 Web Push. Push messages are limited to task-available and task-control wakeups from the authorized workbench, not marketing notifications.
