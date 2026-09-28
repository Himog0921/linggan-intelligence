#!/usr/bin/env python3
"""Headless browser regression for the Comment Study P2 command surface.

Default mode serves the real page assets with synthetic API replies. --api-base-url mode drives
the same browser flow through a live Axum router backed by a disposable PostgreSQL proof fixture.
Both modes are synthetic-only and never start a worker or call a model provider.
"""

from __future__ import annotations

import argparse
import ipaddress
import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlsplit

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


def run_synthetic() -> None:
    httpd = ThreadingHTTPServer(("127.0.0.1", 0), StaticPageHandler)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    base_url = f"http://127.0.0.1:{httpd.server_port}"

    state = {
        "active_policy": OLD_POLICY_REF,
        "created": False,
        "stopped": False,
        "deep_active_mode": False,
        "policy_page_cursors": [],
    }
    requests: dict[str, dict] = {}
    unexpected: list[str] = []

    def policies(cursor: str | None = None) -> dict:
        if state["deep_active_mode"]:
            state["policy_page_cursors"].append(cursor)
            if cursor == "older-page":
                return {
                    "items": [
                        {
                            "policyRef": OLD_POLICY_REF,
                            "methodName": "历史默认方法",
                            "recordingState": "legacy_unrecorded",
                            "isActive": state["active_policy"] == OLD_POLICY_REF,
                        }
                    ],
                    "page": {"hasMore": False, "nextCursor": None},
                }
            return {
                "items": [
                    {
                        "policyRef": NEW_POLICY_REF,
                        "methodName": "最近记录方法",
                        "recordingState": "recorded",
                        "isActive": state["active_policy"] == NEW_POLICY_REF,
                    },
                    *[
                        {
                            "policyRef": f"00000000-0000-4000-8000-{index:012d}",
                            "methodName": f"记录方法 {index}",
                            "recordingState": "recorded",
                            "isActive": False,
                        }
                        for index in range(6, 105)
                    ],
                ],
                "page": {"hasMore": True, "nextCursor": "older-page"},
            }
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
        query = parse_qs(parsed.query)
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
            response = policies(query.get("cursor", [None])[0])
        elif request.method == "GET" and path == "runs":
            response = {"runs": [run_record()] if state["created"] else []}
        elif request.method == "GET" and path == "overview":
            response = {"cleanLayerState": "ready", "latestRun": None}
        elif request.method == "GET" and path == "problems":
            response = {"problems": []}
        elif request.method == "POST" and path == "policies":
            requests["policy"] = payload
            response = {"policy": {"policyRef": NEW_POLICY_REF}}
        elif request.method == "POST" and path.endswith("/activate"):
            requests["activate"] = payload
            activated_ref = path.split("/")[1]
            state["active_policy"] = activated_ref
            response = {"data": {"policyRef": activated_ref}}
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

            state.update(
                active_policy=OLD_POLICY_REF,
                created=False,
                stopped=False,
                deep_active_mode=True,
            )
            cursor_start = len(state["policy_page_cursors"])
            page = browser.new_page()
            page.route("**/api/local/comment-study/**", api_reply)
            page.goto(f"{base_url}/corpus/comments?domain={DOMAIN_REF}")
            page.locator(f"#work-{WORK_REF}").wait_for(state="attached")
            page.locator(f"#study-policy option[value='{NEW_POLICY_REF}']").wait_for(state="attached")
            page.get_by_role("button", name="发起研究").click()
            page.locator("#activate-policy").click()
            page.get_by_text("已将所选方法设为当前领域默认版本。", exact=True).wait_for()
            assert state["policy_page_cursors"][cursor_start : cursor_start + 2] == [None, "older-page"]
            assert requests.get("activate", {}).get("expectedActivePolicyRef") == OLD_POLICY_REF
            assert not unexpected, "Unexpected API calls: " + "; ".join(unexpected)
            page.close()
            browser.close()

        try:
            run_live_api(
                base_url,
                DOMAIN_REF,
                WORK_REF,
                NEW_POLICY_REF,
                OLD_POLICY_REF,
                "not-the-live-proof-token",
            )
        except ValueError as error:
            assert "not the isolated browser proof server" in str(error)
        else:
            raise AssertionError("live mode accepted a server without the isolated proof handshake")
    finally:
        httpd.shutdown()
        httpd.server_close()

    print("Comment Study P2 synthetic browser regression passed")


def run_live_api(
    base_url: str,
    domain_ref: str,
    work_ref: str,
    existing_policy_ref: str,
    initial_active_policy_ref: str,
    proof_token: str,
) -> None:
    parsed_base = urlsplit(base_url)
    try:
        loopback = parsed_base.hostname == "localhost" or ipaddress.ip_address(parsed_base.hostname or "").is_loopback
    except ValueError:
        loopback = False
    if parsed_base.scheme != "http" or not loopback or not parsed_base.port:
        raise ValueError("live API browser mode only accepts an explicit HTTP loopback host and port")

    calls: list[dict] = []
    responses: list[dict] = []
    page_errors: list[str] = []
    browser_requests: list[str] = []
    failed_requests: list[str] = []
    console_errors: list[str] = []

    def record_request(request) -> None:
        browser_requests.append(f"{request.method} {urlsplit(request.url).path}")
        path = urlsplit(request.url).path
        if not path.startswith("/api/local/comment-study/"):
            return
        payload = None
        if request.post_data:
            try:
                payload = json.loads(request.post_data)
            except json.JSONDecodeError:
                payload = request.post_data
        calls.append({"method": request.method, "path": path, "payload": payload})

    def record_response(response) -> None:
        path = urlsplit(response.url).path
        if not path.startswith("/api/local/comment-study/"):
            return
        try:
            body = response.json()
        except Exception:  # Non-JSON error responses are retained as status evidence.
            body = None
        responses.append({"method": response.request.method, "path": path, "status": response.status, "body": body})

    def record_failed_request(request) -> None:
        failed_requests.append(f"{request.method} {request.url}: {request.failure}")

    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=True)
        page = browser.new_page()
        page.set_default_timeout(15000)
        page.on("request", record_request)
        page.on("response", record_response)
        page.on("pageerror", lambda error: page_errors.append(str(error)))
        page.on("requestfailed", record_failed_request)
        page.on("console", lambda message: console_errors.append(message.text) if message.type == "error" else None)
        proof = page.request.get(f"{base_url}/__comment-study-browser-proof")
        if proof.status != 200 or proof.text() != proof_token:
            browser.close()
            raise ValueError("live API base URL is not the isolated browser proof server")
        navigation = page.goto(f"{base_url}/corpus/comments?domain={domain_ref}")
        try:
            page.locator(f"#work-{work_ref}").wait_for(state="attached")
        except Exception as error:
            request_summary = [(item["method"], item["path"]) for item in calls]
            response_summary = [(item["method"], item["path"], item["status"]) for item in responses]
            page_summary = {
                "url": page.url,
                "navigation_status": navigation.status if navigation else None,
                "title": page.title(),
                "body_text": page.locator("body").inner_text()[:500],
            }
            raise AssertionError(
                "Real API page did not render the seeded work selector; "
                f"requests={request_summary!r}; responses={response_summary!r}; "
                f"page_errors={page_errors!r}; console_errors={console_errors!r}; "
                f"browser_requests={browser_requests!r}; failed_requests={failed_requests!r}; "
                f"page={page_summary!r}"
            ) from error
        page.get_by_role("button", name="发起研究").click()
        page.locator(f"#work-{work_ref}").wait_for(state="visible")
        page.locator(f"#study-policy option[value='{existing_policy_ref}']").wait_for(state="attached")
        assert page.locator("#study-policy").input_value() == existing_policy_ref

        page.locator("#method-name").fill("隔离浏览器方法")
        page.locator("#stage-semantic").fill("仅用于真实 Axum 与隔离 PostgreSQL 浏览器回归")
        page.locator("#save-policy").click()
        try:
            page.get_by_text("已保存不可变方法版本", exact=False).wait_for()
        except Exception as error:
            raise AssertionError(
                "Method creation did not receive its success feedback; "
                f"requests={[item for item in calls if item['method'] == 'POST']!r}; "
                f"responses={[item for item in responses if item['method'] == 'POST']!r}; "
                f"page_errors={page_errors!r}; save_disabled={page.locator('#save-policy').is_disabled()}; "
                f"form_valid={page.locator('#policy-form').evaluate('(form) => form.checkValidity()')}; "
                f"status={page.locator('#policy-status').inner_text()!r}"
            ) from error
        created_policy_ref = page.locator("#study-policy").input_value()
        policy_response = next(
            (item for item in responses if item["method"] == "POST" and item["path"].endswith("/policies")),
            None,
        )
        response_policy_ref = (
            policy_response.get("body", {}).get("policy", {}).get("policyRef")
            if policy_response
            else None
        )
        if (
            not created_policy_ref
            or created_policy_ref == existing_policy_ref
            or not policy_response
            or policy_response["status"] != 201
            or created_policy_ref != response_policy_ref
        ):
            raise AssertionError(
                "Saved method success feedback did not select the policy returned by the API; "
                f"selected={created_policy_ref!r}; existing={existing_policy_ref!r}; "
                f"response_status={policy_response['status'] if policy_response else None!r}; "
                f"response_policy_ref={response_policy_ref!r}; "
                f"options={page.locator('#study-policy').locator('option').evaluate_all('(options) => options.map(option => option.value)')!r}"
            )
        page.locator("#activate-policy").click()
        page.get_by_text("已将所选方法设为当前领域默认版本。", exact=True).wait_for()

        page.locator(f"#work-{work_ref}").check()
        page.locator("#comment-budget").fill("2")
        page.locator("#context-character-budget").fill("3500")
        page.locator("#token-budget").fill("4096")
        page.locator("#preview-run").click()
        page.get_by_text("预计创建 2 条目标", exact=False).wait_for()

        page.locator("#start-run").click()
        page.get_by_role("button", name="停止本次运行").wait_for()
        page.get_by_role("button", name="停止本次运行").click()
        identity = page.locator("#study-stop-run-identity").inner_text()
        run_ref = identity.split("·", 1)[0].strip().removeprefix("Run ")
        if not run_ref:
            raise AssertionError(f"Stop confirmation omitted the server Run identity: {identity!r}")
        page.get_by_text(f"Run {run_ref} · 当前未终态 2 条 · 目标总数 2 条", exact=True).wait_for()
        page.locator("#study-stop-confirm").click()
        page.get_by_text("已按服务端回执停止本次运行。", exact=True).wait_for()

        preview = next(
            (item for item in calls if item["method"] == "POST" and item["path"].endswith("/selection-preview")),
            None,
        )
        start = next(
            (item for item in calls if item["method"] == "POST" and item["path"].endswith("/runs")),
            None,
        )
        stop = next(
            (item for item in calls if item["method"] == "POST" and item["path"].endswith(f"/runs/{run_ref}/stop")),
            None,
        )
        assert preview and preview["payload"]["limits"] == {
            "commentBudget": 2,
            "contextCharacterBudget": 3500,
            "tokenLimit": 4096,
        }
        policy = next(
            (item for item in calls if item["method"] == "POST" and item["path"].endswith("/policies")),
            None,
        )
        assert policy and policy["payload"]["methodName"] == "隔离浏览器方法"
        assert policy["payload"]["stageInstructions"] == {
            "semantic": "仅用于真实 Axum 与隔离 PostgreSQL 浏览器回归",
            "resolution": "",
            "pair": "",
        }
        activation = next(
            (item for item in calls if item["method"] == "POST" and item["path"].endswith("/activate")),
            None,
        )
        assert activation and activation["payload"]["expectedActivePolicyRef"] == initial_active_policy_ref
        assert start and start["payload"]["policyRef"] == created_policy_ref
        assert start["payload"]["requestRef"]
        assert start["payload"]["workRoles"] == [{"contentPublicRef": work_ref, "observationRole": "primary"}]
        assert stop and stop["payload"]["expectedControlVersion"] == 0
        assert any(item["path"].endswith("/selection-preview") and item["status"] == 200 for item in responses)
        assert any(item["method"] == "POST" and item["path"].endswith("/policies") and item["status"] == 201 for item in responses)
        assert any(item["method"] == "POST" and item["path"].endswith("/activate") and item["status"] == 200 for item in responses)
        start_response = next(
            item for item in responses if item["method"] == "POST" and item["path"].endswith("/runs")
        )
        assert start_response["status"] == 201
        assert start_response["body"]["runRef"] == run_ref
        stop_response = next(
            item for item in responses if item["method"] == "POST" and item["path"].endswith(f"/runs/{run_ref}/stop")
        )
        assert stop_response["status"] == 200
        assert stop_response["body"]["data"]["dispatchState"] == "stopped"
        assert stop_response["body"]["data"]["controlVersion"] == 1
        assert not page_errors, "Browser page errors: " + "; ".join(page_errors)
        browser.close()

    print("Comment Study P2 live Axum/PostgreSQL browser regression passed")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--api-base-url")
    parser.add_argument("--domain-ref")
    parser.add_argument("--work-ref")
    parser.add_argument("--existing-policy-ref")
    parser.add_argument("--initial-active-policy-ref")
    parser.add_argument("--proof-token")
    args = parser.parse_args()
    if args.api_base_url:
        required = {
            "--domain-ref": args.domain_ref,
            "--work-ref": args.work_ref,
            "--existing-policy-ref": args.existing_policy_ref,
            "--initial-active-policy-ref": args.initial_active_policy_ref,
            "--proof-token": args.proof_token,
        }
        missing = [name for name, value in required.items() if not value]
        if missing:
            parser.error("--api-base-url requires " + ", ".join(missing))
        run_live_api(
            args.api_base_url.rstrip("/"),
            args.domain_ref,
            args.work_ref,
            args.existing_policy_ref,
            args.initial_active_policy_ref,
            args.proof_token,
        )
        return
    if any((args.domain_ref, args.work_ref, args.existing_policy_ref, args.initial_active_policy_ref, args.proof_token)):
        parser.error("fixture references are only valid with --api-base-url")
    run_synthetic()


if __name__ == "__main__":
    main()
