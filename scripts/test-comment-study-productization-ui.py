#!/usr/bin/env python3
"""Headless browser regression for the Comment Study P2 command surface.

The browser loads the real HTML, CSS, and JavaScript. API replies are synthetic so this test
checks visible interaction and request construction only; Rust/Axum/PostgreSQL tests prove the
server contract separately.
"""

from __future__ import annotations

import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlsplit

from playwright.sync_api import sync_playwright


ROOT = Path(__file__).resolve().parents[1]
DOMAIN_REF = "00000000-0000-4000-8000-000000000001"
WORK_REF = "00000000-0000-4000-8000-000000000002"
OLD_POLICY_REF = "00000000-0000-4000-8000-000000000003"
NEW_POLICY_REF = "00000000-0000-4000-8000-000000000004"
RUN_REF = "00000000-0000-4000-8000-000000000005"


class StaticPageHandler(BaseHTTPRequestHandler):
    def do_GET(self) -> None:  # noqa: N802 - stdlib handler name
        path = urlsplit(self.path).path
        if path == "/corpus/comments":
            body = (ROOT / "apps/api/src/local_web/comment_study.html").read_text()
            body = body.replace("{{HEADER}}", "<header aria-label='本地合成验收'></header>")
            body = body.replace("{{SIDE_NAV}}", "<nav aria-label='测试导航'></nav>")
            content_type = "text/html; charset=utf-8"
        elif path == "/assets/comment-study.css":
            body = (ROOT / "apps/api/src/local_web/comment_study.css").read_text()
            content_type = "text/css; charset=utf-8"
        elif path == "/assets/comment-study.js":
            body = (ROOT / "apps/api/src/local_web/comment_study.js").read_text()
            content_type = "text/javascript; charset=utf-8"
        else:
            self.send_error(404)
            return

        encoded = body.encode()
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def log_message(self, _format: str, *_args: object) -> None:
        return


def main() -> None:
    httpd = ThreadingHTTPServer(("127.0.0.1", 0), StaticPageHandler)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    base_url = f"http://127.0.0.1:{httpd.server_port}"

    state = {"active_policy": OLD_POLICY_REF, "created": False, "stopped": False}
    requests: dict[str, dict] = {}
    unexpected: list[str] = []

    def policies() -> dict:
        return {
            "items": [
                {
                    "policyRef": OLD_POLICY_REF,
                    "methodName": "原默认方法",
                    "recordingState": "recorded",
                    "isActive": state["active_policy"] == OLD_POLICY_REF,
                },
                *(
                    [
                        {
                            "policyRef": NEW_POLICY_REF,
                            "methodName": "回归验收方法",
                            "recordingState": "recorded",
                            "isActive": state["active_policy"] == NEW_POLICY_REF,
                        }
                    ]
                    if "policy" in requests
                    else []
                ),
            ]
        }

    def run_record() -> dict:
        return {
            "runRef": RUN_REF,
            "createdAt": "2026-09-28T08:00:00Z",
            "finishedAt": "2026-09-28T08:01:00Z" if state["stopped"] else None,
            "state": "cancelled" if state["stopped"] else "running",
            "selectionContract": "comment-study.run-selection.v2",
            "dispatchState": "stopped" if state["stopped"] else "enabled",
            "dispatchReason": "user_stopped" if state["stopped"] else None,
            "controlVersion": 1 if state["stopped"] else 0,
            "pendingCount": 2,
            "workCount": 1,
            "primaryWorkCount": 1,
            "referenceWorkCount": 0,
            "targetCount": 2,
            "succeededCount": 0,
            "noSignalCount": 0,
            "needsContextCount": 0,
            "failedCount": 0,
            "excludedCount": 0,
        }

    def api_reply(route) -> None:
        request = route.request
        parsed = urlsplit(request.url)
        path = parsed.path.removeprefix("/api/local/comment-study/")
        payload = request.post_data_json if request.method == "POST" else None
        response: dict

        if request.method == "GET" and path == "setup":
            response = {
                "domainRef": DOMAIN_REF,
                "domainStatus": "active",
                "modelConfigs": [
                    {
                        "configRef": "00000000-0000-4000-8000-000000000006",
                        "modelId": "synthetic-model",
                        "inputTokenLimit": 8192,
                        "outputTokenLimit": 2048,
                        "enabled": True,
                    }
                ],
                "sourcePreview": {
                    "asOf": "2026-09-28T08:00:00Z",
                    "totalCommentCount": 3,
                    "eligibleCommentCount": 3,
                    "unknownAuthorCount": 0,
                    "excludedCounts": {},
                },
                "referenceSourcePreview": {
                    "asOf": "2026-09-28T08:00:00Z",
                    "totalCommentCount": 0,
                    "eligibleCommentCount": 0,
                    "unknownAuthorCount": 0,
                    "excludedCounts": {},
                },
            }
        elif request.method == "GET" and path == "works":
            response = {
                "items": [
                    {
                        "workRef": WORK_REF,
                        "contentPublicRef": WORK_REF,
                        "displayTitle": "合成作品 A",
                        "displayTitleSource": "platform_title",
                        "eligibleCommentCount": 3,
                        "observationRole": "primary",
                    }
                ],
                "totalWorkCount": 1,
                "indexCoverage": {"indexedCount": 3, "pendingCount": 0},
                "page": {"nextCursor": None},
            }
        elif request.method == "GET" and path == "policies":
            response = policies()
        elif request.method == "GET" and path == "runs":
            response = {"runs": [run_record()] if state["created"] else []}
        elif request.method == "GET" and path == "overview":
            response = {"cleanLayerState": "ready", "latestRun": None}
        elif request.method == "GET" and path == "problems":
            response = {"problems": []}
        elif request.method == "POST" and path == "policies":
            requests["policy"] = payload
            response = {"policy": {"policyRef": NEW_POLICY_REF}}
        elif request.method == "POST" and path == f"policies/{NEW_POLICY_REF}/activate":
            requests["activate"] = payload
            state["active_policy"] = NEW_POLICY_REF
            response = {"data": {"policyRef": NEW_POLICY_REF}}
        elif request.method == "POST" and path == "selection-preview":
            requests["preview"] = payload
            response = {
                "requestedWorkCount": 1,
                "scopeCommentCount": 3,
                "targetCount": 2,
                "indexCoverage": {"pendingCount": 0},
            }
        elif request.method == "POST" and path == "runs":
            requests["start"] = payload
            state["created"] = True
            response = {
                "outcome": "created",
                "runRef": RUN_REF,
                "coveredWorkCount": 1,
                "targetCount": 2,
                "requestRef": payload["requestRef"],
            }
        elif request.method == "POST" and path == f"runs/{RUN_REF}/stop":
            requests["stop"] = payload
            state["stopped"] = True
            response = {
                "data": {
                    "runRef": RUN_REF,
                    "controlVersion": payload["expectedControlVersion"] + 1,
                    "dispatchState": "stopped",
                    "dispatchReason": "user_stopped",
                }
            }
        else:
            unexpected.append(f"{request.method} {request.url}")
            route.fulfill(status=501, content_type="application/json", body='{"error":"unexpected request"}')
            return

        route.fulfill(status=200, content_type="application/json", body=json.dumps(response))

    try:
        with sync_playwright() as playwright:
            browser = playwright.chromium.launch(headless=True)
            page = browser.new_page()
            page.set_default_timeout(10000)
            page.route("**/api/local/comment-study/**", api_reply)
            page.goto(f"{base_url}/corpus/comments?domain={DOMAIN_REF}")
            page.locator(f"#work-{WORK_REF}").wait_for(state="attached")
            page.get_by_role("button", name="发起研究").click()
            page.locator(f"#work-{WORK_REF}").wait_for(state="visible")
            page.locator(f"#study-policy option[value='{OLD_POLICY_REF}']").wait_for(state="attached")

            page.locator("#method-name").fill("回归验收方法")
            page.locator("#stage-semantic").fill("只用于隔离浏览器回归")
            page.locator("#save-policy").click()
            page.get_by_text("已保存不可变方法版本", exact=False).wait_for()
            page.locator("#study-policy").select_option(NEW_POLICY_REF)
            page.locator("#activate-policy").click()
            page.get_by_text("已将所选方法设为当前领域默认版本。", exact=True).wait_for()

            page.locator(f"#work-{WORK_REF}").check()
            page.locator("#comment-budget").fill("2")
            page.locator("#context-character-budget").fill("3500")
            page.locator("#token-budget").fill("4096")
            page.locator("#preview-run").click()
            page.get_by_text("预计创建 2 条目标", exact=False).wait_for()

            page.locator("#start-run").click()
            page.get_by_role("button", name="停止本次运行").wait_for()
            page.get_by_role("button", name="停止本次运行").click()
            page.get_by_text(f"Run {RUN_REF} · 当前未终态 2 条 · 目标总数 2 条", exact=True).wait_for()
            page.locator("#study-stop-confirm").click()
            page.get_by_text("已按服务端回执停止本次运行。", exact=True).wait_for()

            assert requests.get("policy", {}).get("stageInstructions", {}).get("semantic") == "只用于隔离浏览器回归"
            assert requests.get("activate", {}).get("expectedActivePolicyRef") == OLD_POLICY_REF
            preview = requests.get("preview", {})
            assert preview.get("limits") == {
                "commentBudget": 2,
                "contextCharacterBudget": 3500,
                "tokenLimit": 4096,
            }
            start = requests.get("start", {})
            assert start.get("policyRef") == NEW_POLICY_REF
            assert start.get("requestRef")
            assert start.get("workRoles") == [{"contentPublicRef": WORK_REF, "observationRole": "primary"}]
            assert requests.get("stop", {}).get("expectedControlVersion") == 0
            assert not unexpected, "Unexpected API calls: " + "; ".join(unexpected)
            browser.close()
    finally:
        httpd.shutdown()
        httpd.server_close()

    print("Comment Study P2 synthetic browser regression passed")


if __name__ == "__main__":
    main()
