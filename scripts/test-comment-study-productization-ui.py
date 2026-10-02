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
REQUEST_REF = "00000000-0000-4000-8000-000000000012"
SIGNAL_REF = "00000000-0000-4000-8000-000000000013"
RECOVERY_RUN_REF = "00000000-0000-4000-8000-000000000010"
OLDER_RUN_REF = "10000000-0000-4000-8000-000000000007"
MODEL_REASON = "评论仅缺少直接父评论中的指代对象，无法确认具体情境。"


class StaticPageHandler(BaseHTTPRequestHandler):
    def do_GET(self) -> None:  # noqa: N802 - stdlib handler name
        path = urlsplit(self.path).path
        if path == "/corpus/comments":
            body = (ROOT / "apps/api/src/local_web/comment_study.html").read_text()
            body = body.replace("{{HEADER}}", "<header aria-label='本地合成验收'></header>")
            body = body.replace("{{SIDE_NAV}}", "<nav aria-label='测试导航'></nav>")
            content_type = "text/html; charset=utf-8"
        elif path == "/assets/comment-study.css":
            body = "\n".join((ROOT / f"apps/api/src/local_web/{name}").read_text()
                             for name in ("lids_tokens.css", "shell.css", "comment_study.css"))
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
        "recovered": False,
        "deep_active_mode": False,
        "overview_with_run": False,
        "candidate_fail": False,
        "request_source_state": "known",
        "policy_page_cursors": [],
        "run_page_cursors": [],
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
                            "defaults": {"commentBudget": 100, "contextCharacterBudget": 6000},
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
                        "defaults": {"commentBudget": 100, "contextCharacterBudget": 6000},
                        "recordingState": "recorded",
                        "isActive": state["active_policy"] == NEW_POLICY_REF,
                    },
                    *[
                        {
                            "policyRef": f"00000000-0000-4000-8000-{index:012d}",
                            "methodName": f"记录方法 {index}",
                            "defaults": {"commentBudget": 100, "contextCharacterBudget": 6000},
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
                    "defaults": {"commentBudget": 100, "contextCharacterBudget": 6000},
                    "recordingState": "recorded",
                    "isActive": state["active_policy"] == OLD_POLICY_REF,
                },
                *(
                    [
                        {
                            "policyRef": NEW_POLICY_REF,
                            "methodName": "回归验收方法",
                            "defaults": {"commentBudget": 100, "contextCharacterBudget": 6000},
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
            "pendingCount": 0 if state["stopped"] else 2,
            "workCount": 1,
            "primaryWorkCount": 1,
            "referenceWorkCount": 0,
            "targetCount": 2,
            "succeededCount": 0,
            "noSignalCount": 0,
            "needsContextCount": 0,
            "failedCount": 0,
            "excludedCount": 0,
            "cancelledCount": 2 if state["stopped"] else 0,
            "limits": {"commentBudget": 2, "contextCharacterBudget": 3500, "tokenLimit": 4096},
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
        elif request.method == "GET" and path == f"policies/{OLD_POLICY_REF}":
            response = {
                "policy": {
                    "policyRef": OLD_POLICY_REF,
                    "methodName": "原默认方法",
                    "defaults": {"commentBudget": 100, "contextCharacterBudget": 6000},
                    "methodManifest": {
                        "modelConfigRef": "00000000-0000-4000-8000-000000000006",
                        "stages": {
                            stage: {
                                "systemInstruction": (
                                    "基础研究规则\n<stage-instructions>\n原有补充说明\n</stage-instructions>"
                                )
                            }
                            for stage in ("semantic", "resolution", "pair")
                        },
                    },
                }
            }
        elif request.method == "GET" and path == "runs":
            cursor = query.get("cursor", [None])[0]
            state["run_page_cursors"].append(cursor)
            if cursor == "older-run":
                older_run = {
                    **run_record(),
                    "runRef": OLDER_RUN_REF,
                    "createdAt": "2026-09-20T08:00:00Z",
                    "finishedAt": "2026-09-20T08:01:00Z",
                    "state": "completed",
                    "dispatchState": "stopped",
                    "dispatchReason": "user_stopped",
                    "targetCount": 5,
                    "pendingCount": 0,
                    "cancelledCount": 0,
                }
                response = {"runs": [older_run], "page": {"hasMore": False, "nextCursor": None}}
            else:
                response = {
                    "runs": ([{**run_record(), "runRef": RECOVERY_RUN_REF, "recoverySourceRunRef": RUN_REF,
                              "finishedAt": None, "state": "queued", "dispatchState": "enabled",
                              "cancelledCount": 0, "pendingCount": 1}, run_record()]
                             if state["recovered"] else [run_record()] if state["created"] else []),
                    "page": {"hasMore": state["created"], "nextCursor": "older-run" if state["created"] else None},
                }
        elif request.method == "GET" and path in (f"runs/{RUN_REF}", f"runs/{RECOVERY_RUN_REF}", f"runs/{OLDER_RUN_REF}"):
            requested_run = path.split("/")[1]
            historical = requested_run == OLDER_RUN_REF
            response = {"run": {**run_record(), "runRef": requested_run,
                                "policyRef": OLD_POLICY_REF,
                                "method": {"name": "合成方法", "hash": "synthetic-hash",
                                           "manifest": {"stages": {}}},
                                "semanticSummary": {"targetCount": 2, "succeededTargetCount": 1,
                                                    "noSignalTargetCount": 1,
                                                    "needsContextTargetCount": 0,
                                                    "failedTargetCount": 0, "excludedTargetCount": 0,
                                                    "attemptCount": 2, "signalCount": 1,
                                                    "currentSignalCount": 1},
                                "knowledgeSummary": {"eligibleSignalCount": 1,
                                                     "assignedSignalCount": 0,
                                                     "createdProblemCount": 0,
                                                     "pendingResolutionCount": 0,
                                                     "pendingPairCount": 0,
                                                     "deferredNovelCount": 1,
                                                     "deferredAmbiguousCount": 0,
                                                     "deferredContextCount": 0,
                                                     "retrievalIncompleteCount": 0,
                                                     "budgetStoppedCount": 0,
                                                     "protocolRejectedCount": 0,
                                                     "failedResolutionCount": 0},
                                "costSummary": {"recordingState": "unrecorded" if historical else "recorded",
                                                "requestCount": None if historical else 1,
                                                "dispatchedRequestCount": None if historical else 1,
                                                "usageKnownRequestCount": None if historical else 1,
                                                "usageUnknownRequestCount": None if historical else 0,
                                                "knownInputTokens": None if historical else 100,
                                                "knownOutputTokens": None if historical else 20,
                                                "totalInputTokens": None if historical else 100,
                                                "totalOutputTokens": None if historical else 20,
                                                "chargedTokens": None if historical else 120}}}
        elif request.method == "GET" and path == "targets":
            requested_run = query.get("runRef", [None])[0]
            response = {
                "runRef": requested_run,
                "page": {"hasMore": False, "nextCursor": None},
                "targets": ([{
                    "targetRef": "00000000-0000-4000-8000-000000000011",
                    "sourceRef": "00000000-0000-4000-8000-000000000009",
                    "workRef": WORK_REF,
                    "observationRole": "primary",
                    "commentText": "补跑目标重新冻结的评论",
                    "sourceState": "known",
                    "researchText": "补跑目标重新冻结的评论",
                    "dependencyState": "self_contained",
                    "contextState": "ready",
                    "state": "queued",
                    "workContext": {"sources": []},
                    "parentContext": {"state": "none"},
                    "attemptCount": 0,
                    "signalCount": 0,
                }] if requested_run == RECOVERY_RUN_REF else [
                    {
                        "targetRef": "00000000-0000-4000-8000-000000000008",
                        "sourceRef": "00000000-0000-4000-8000-000000000009",
                        "parentSourceRef": None,
                        "workRef": WORK_REF,
                        "observationRole": "primary",
                        "commentText": "黑脸了。可能和上课心情一样",
                        "sourceState": "known",
                        "researchText": "黑脸了。可能和上课心情一样",
                        "dependencyState": "self_contained",
                        "contextState": "ready",
                        "state": "needs_context",
                        "workContext": {"sources": [{"kind": "native_title", "text": "一年级的奔溃时刻"}]},
                        "parentContext": {"state": "not_included"},
                        "modelReason": MODEL_REASON,
                        "latestAttempt": {
                            "attemptOrdinal": 1,
                            "state": "accepted",
                            "usageKnown": True,
                            "inputTokens": 500,
                            "outputTokens": 200,
                            "chargedTokens": 700,
                            "httpStatus": 200,
                            "receivedBytes": 1200,
                            "terminalReceived": True,
                        },
                        "attemptCount": 1,
                        "signalCount": 0,
                    }
                ]),
            }
        elif request.method == "GET" and path == "signals":
            response = {"runRef": query.get("runRef", [RUN_REF])[0],
                        "page": {"nextCursor": None},
                        "signals": [{"signalRef": SIGNAL_REF,
                                     "targetRef": "00000000-0000-4000-8000-000000000008",
                                     "observationRole": "primary", "kind": "problem",
                                     "proposition": "合成待建档表达", "evidence": "合成证据",
                                     "sourceState": "known", "eligibilityState": "eligible",
                                     "resolutionState": "deferred_novel"}]}
        elif request.method == "GET" and path == f"runs/{RUN_REF}/requests":
            response = {"runRef": RUN_REF, "page": {"nextCursor": None},
                        "requests": [{"invocationRef": REQUEST_REF, "stage": "semantic",
                                      "state": "accepted", "dispatched": True,
                                      "modelIdentity": {"modelRef": "00000000-0000-4000-8000-000000000017",
                                                        "connectionVersionRef": "00000000-0000-4000-8000-000000000018",
                                                        "modelId": "synthetic-model"},
                                      "modelConfigRef": "00000000-0000-4000-8000-000000000006",
                                      "createdAt": "2026-09-28T08:00:00Z", "attemptOrdinal": 1,
                                      "usageKnown": True, "inputTokens": 100,
                                      "outputTokens": 20, "chargedTokens": 120}]}
        elif request.method == "GET" and path == f"requests/{REQUEST_REF}":
            source_state = state["request_source_state"]
            response = {"request": {"invocationRef": REQUEST_REF, "runRef": RUN_REF,
                                    "stage": "semantic", "state": "accepted", "dispatched": True,
                                    "modelIdentity": {"modelRef": "00000000-0000-4000-8000-000000000017",
                                                      "connectionVersionRef": "00000000-0000-4000-8000-000000000018",
                                                      "modelId": "synthetic-model"},
                                    "modelConfigRef": "00000000-0000-4000-8000-000000000006",
                                    "createdAt": "2026-09-28T08:00:00Z", "recordingState": "recorded",
                                    "sourceState": source_state,
                                    "requestManifest": {"prompt": "合成请求快照"} if source_state == "known" else None}}
        elif request.method == "GET" and path == "overview":
            response = {"cleanLayerState": "ready", "latestRun": run_record() if state["overview_with_run"] else None,
                        "asOf": "2026-09-28T08:00:00Z", "domainRef": DOMAIN_REF,
                        "corpusSummaryState": "known",
                        "corpusSummary": {"displayableCommentCount": 3, "eligibleCommentCount": 2},
                        "indexCoverage": {"indexedCount": 3, "pendingCount": 0},
                        "researchSummary": {"studiedCommentCount": 1, "succeededCommentCount": 1,
                                            "noSignalCommentCount": 0, "currentSignalCount": 1},
                        "knowledgeSummary": {"problemCount": 0, "activeProblemCount": 0,
                                             "supportInsufficientProblemCount": 0,
                                             "assignedSignalCount": 0, "deferredNovelCount": 1,
                                             "supportWindowDays": 28, "problemSupportPreview": [],
                                             "voicePreview": [], "solutionPreview": [],
                                             "experiencePreview": []},
                        "observationSeries": {"timezone": "Asia/Shanghai", "points": [
                            {"date": "2026-09-28", "newObservedCommentCount": 3,
                             "studiedCommentCount": 1, "acceptedSignalCount": 1,
                             "coveredWorkCount": 1, "observationCoverage": "recorded"}]}}
        elif request.method == "GET" and path == "problems":
            response = {"problems": []}
        elif request.method == "GET" and path == "problem-candidates":
            if state["candidate_fail"]:
                route.fulfill(status=503, content_type="application/json", body='{"error":"temporarily unavailable"}')
                return
            candidate_state = query.get("state", ["all"])[0]
            novel = {"resolutionRef": "00000000-0000-4000-8000-000000000014",
                     "signalRef": SIGNAL_REF,
                     "targetRef": "00000000-0000-4000-8000-000000000008",
                     "runRef": RUN_REF, "state": "deferred_novel", "kind": "problem",
                     "proposition": "合成待建档表达", "evidence": "合成证据",
                     "commentText": "合成评论原声", "authorDisplayName": "合成作者",
                     "commentKey": {"workRef": WORK_REF, "commentExternalId": "comment-1"},
                     "pairOutcomes": [{"pairRef": "00000000-0000-4000-8000-000000000015",
                                       "state": "rejected", "decisionReason": "not_same_problem"}],
                     "sourceState": "known", "createdAt": "2026-09-28T08:00:00Z"}
            budget_stopped = {**novel, "resolutionRef": "00000000-0000-4000-8000-000000000016",
                              "state": "budget_stopped", "pairOutcomes": [],
                              "proposition": "合成机器未完成表达"}
            pending = {**novel, "resolutionRef": "00000000-0000-4000-8000-000000000019",
                       "state": "pending", "pairOutcomes": [], "proposition": "合成归并中表达"}
            expressions = [novel, budget_stopped, pending] if candidate_state == "all" else (
                [novel] if candidate_state == "deferred_novel" else
                [budget_stopped] if candidate_state == "budget_stopped" else
                [pending] if candidate_state == "pending" else [])
            response = {"expressions": expressions,
                        "page": {"nextCursor": None, "asOf": "2026-09-28T08:00:00Z"}}
        elif request.method == "GET" and path == "catalog-summary":
            response = {"summary": {"displayableCommentCount": 3, "eligibleCommentCount": 2},
                        "indexCoverage": {"indexedCount": 3, "pendingCount": 0}}
        elif request.method == "GET" and path == "comments":
            cursor = query.get("cursor", [None])[0]
            def comment_item(comment_id: str, eligible: bool) -> dict:
                return {"commentKey": {"workRef": WORK_REF, "commentExternalId": comment_id},
                        "commentText": f"合成评论 {comment_id}", "workTitle": "合成作品 A",
                        "workTitleSource": "platform_title", "authorDisplayName": "合成作者",
                        "observationRole": "primary", "observedAt": "2026-09-28T08:00:00Z",
                        "studyEligibility": {"eligible": eligible,
                                             "reasons": [] if eligible else ["commentAuthorUnknown"]}}
            response = {"items": [comment_item("comment-3", True)] if cursor == "page-2" else [
                comment_item("comment-1", True), comment_item("comment-2", False)],
                "page": {"nextCursor": None if cursor == "page-2" else "page-2"},
                "indexCoverage": {"indexedCount": 3, "pendingCount": 0}}
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
        elif request.method == "POST" and path == f"runs/{RUN_REF}/recover":
            requests["recover"] = payload
            state["recovered"] = True
            response = {"outcome": "created", "runRef": RECOVERY_RUN_REF,
                        "targetCount": 1, "requestRef": payload["requestRef"]}
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
            assert page.locator("#study-mode").input_value() == "new_only"

            page.get_by_role("button", name="编辑方法").click()
            page.locator("#method-name").wait_for(state="visible")
            page.get_by_text("编辑副本已载入", exact=False).wait_for()
            page.locator("#method-name").fill("回归验收方法")
            page.locator("#stage-semantic").fill("只用于隔离浏览器回归")
            page.locator("#save-policy").click()
            page.get_by_text("已保存并选中新方法版本", exact=False).wait_for()
            assert page.locator("#policy-form").is_hidden()
            page.locator("#study-policy").select_option(NEW_POLICY_REF)
            page.locator("#activate-policy").click()
            page.get_by_text("已将所选方法设为当前领域默认版本。", exact=True).wait_for()

            page.locator(f"#work-{WORK_REF}").check()
            page.locator("#run-comment-budget").fill("2")
            page.locator("#run-context-character-budget").fill("3500")
            page.locator("#token-budget").fill("4096")
            page.locator("#preview-run").click()
            page.get_by_text("预计创建 2 条目标", exact=False).wait_for()

            page.locator("#start-run").click()
            page.get_by_role("button", name="停止本次运行").wait_for()
            page.get_by_text("查看本次输入、原因与处理建议", exact=True).click()
            page.get_by_text(MODEL_REASON, exact=True).wait_for()
            page.get_by_text("本条是回复，但本次运行没有冻结父评论", exact=True).wait_for()
            page.get_by_text("这次 Run 漏带了父评论；修正输入组装后可从源 Run 显式补跑。", exact=True).wait_for()
            page.get_by_role("button", name="加载更早运行").click()
            page.locator(".study-review-table tbody tr").filter(has_text=OLDER_RUN_REF[:8]).wait_for()
            page.get_by_role("button", name="停止本次运行").click()
            page.get_by_text(f"Run {RUN_REF} · 当前未终态 2 条 · 目标总数 2 条", exact=True).wait_for()
            page.locator("#study-stop-confirm").click()
            page.get_by_text("已按服务端回执停止本次运行。", exact=True).wait_for()
            page.locator(".study-review-table tbody tr").filter(has_text=RUN_REF[:8]).get_by_role("button", name="补跑未完成").click()
            page.get_by_text(f"源 Run {RUN_REF}。", exact=False).wait_for()
            page.locator("#study-recover-confirm").click()
            page.get_by_text(f"已按服务端回执创建补跑 Run {RECOVERY_RUN_REF}", exact=False).wait_for()
            page.locator(".study-run-detail blockquote").filter(has_text="补跑目标重新冻结的评论").wait_for()
            assert page.get_by_text(MODEL_REASON, exact=True).count() == 0

            assert requests.get("policy", {}).get("stageInstructions", {}).get("semantic") == "只用于隔离浏览器回归", requests.get("policy")
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
            assert start.get("mode") == "new_only"
            assert start.get("workRoles") == [{"contentPublicRef": WORK_REF, "observationRole": "primary"}]
            assert requests.get("stop", {}).get("expectedControlVersion") == 0
            assert requests.get("recover", {}).get("domainRef") == DOMAIN_REF
            assert requests.get("recover", {}).get("requestRef")
            assert not unexpected, "Unexpected API calls: " + "; ".join(unexpected)

            assert page.evaluate("""async () => {
              const originalFetch = window.fetch;
              const pending = [];
              const template = allRuns[0];
              window.fetch = (url, options) => String(url).includes('/api/local/comment-study/runs?')
                ? new Promise(resolve => pending.push(resolve)) : originalFetch(url, options);
              try {
                const older = loadRunListPage(true);
                const newer = loadRunListPage(true);
                if (pending.length !== 2) return false;
                const reply = state => new Response(JSON.stringify({
                  runs: [{ ...template, state }], page: { nextCursor: null }
                }), { status: 200, headers: { 'Content-Type': 'application/json' } });
                pending[1](reply('running'));
                await newer;
                pending[0](reply('cancelled'));
                await older;
                return allRuns.length === 1 && allRuns[0].state === 'running';
              } finally {
                window.fetch = originalFetch;
              }
            }"""), "a late Run-list response replaced the newer server snapshot"

            state.update(
                active_policy=OLD_POLICY_REF,
                created=False,
                stopped=False,
                recovered=False,
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

            state.update(deep_active_mode=False, candidate_fail=True)
            page = browser.new_page()
            page.route("**/api/local/comment-study/**", api_reply)
            page.goto(f"{base_url}/corpus/comments?domain={DOMAIN_REF}&view=overview")
            page.locator(".study-series-chart").wait_for()
            page.get_by_text("序列截至 2026-09-28", exact=False).wait_for()
            page.locator(".study-readout-label").get_by_text("可研究评论", exact=True).wait_for()
            page.locator('.study-tabs [data-view="problems"]').click()
            page.get_by_role("heading", name="已建档用户问题").wait_for()
            page.get_by_text("未建档表达暂时读取失败；已建档问题仍可查看。", exact=True).wait_for()
            page.close()

            state["candidate_fail"] = False
            page = browser.new_page()
            page.route("**/api/local/comment-study/**", api_reply)
            page.goto(f"{base_url}/corpus/comments?domain={DOMAIN_REF}&view=comments&q=合成&workRef={WORK_REF}&state=never_studied")
            page.locator("#comment-query").wait_for()
            assert page.locator("#comment-query").input_value() == "合成"
            assert page.locator("#comment-study-state").input_value() == "never_studied"
            assert page.locator("[data-select-comment]").count() == 2
            assert page.locator("[data-select-comment]").nth(1).is_disabled()
            page.locator("#comment-next").click()
            page.locator("[data-comment-id='comment-3']").first.wait_for()
            assert "cursor=page-2" in page.url
            page.reload()
            page.locator("[data-comment-id='comment-3']").first.wait_for()
            page.go_back()
            page.locator("[data-comment-id='comment-1']").first.wait_for()
            assert "cursor=" not in page.url
            page.locator("[data-select-comment]").first.check()
            assert page.locator("#comment-selection-count").inner_text().startswith("已选 1/3000")
            page.locator("#comment-next").click()
            page.locator("[data-comment-id='comment-3']").first.wait_for()
            assert page.locator("#comment-selection-count").inner_text().startswith("已选 1/3000"), page.locator("#comment-selection-count").inner_text()
            page.locator("[data-select-comment]").first.check()
            selected_status = page.locator("#comment-selection-count").inner_text()
            assert selected_status.startswith("已选 2/3000 条评论"), selected_status
            page.locator("#study-selected-comments").click()
            page.locator("#preview-run").click()
            page.get_by_text("预计创建 2 条目标", exact=False).wait_for()
            assert requests["preview"]["scope"] == {"kind": "comments", "commentKeys": [
                {"workRef": WORK_REF, "commentExternalId": "comment-1"},
                {"workRef": WORK_REF, "commentExternalId": "comment-3"}]}
            assert requests["preview"]["workRoles"] == [{"contentPublicRef": WORK_REF, "observationRole": "primary"}]
            page.close()

            state["created"] = True
            page = browser.new_page()
            page.route("**/api/local/comment-study/**", api_reply)
            page.goto(f"{base_url}/corpus/comments?domain={DOMAIN_REF}&view=targets&runRef={RUN_REF}")
            page.locator("[data-target-ref]").first.wait_for()
            assert "view=runs" in page.url and "panel=targets" in page.url
            page.locator(".study-run-summary").get_by_text("新表达暂缓 1 条", exact=False).wait_for()
            page.locator(".study-run-summary").get_by_text("请求 1 次", exact=False).wait_for()
            page.get_by_role("tab", name="调用记录").click()
            page.locator(f'[data-request-detail="{REQUEST_REF}"]').click()
            page.locator(".study-request-detail").get_by_text("合成请求快照", exact=False).wait_for()
            page.locator(".study-request-card > p").get_by_text("模型：synthetic-model", exact=True).wait_for()
            state["request_source_state"] = "unavailable"
            page.locator(f'[data-request-detail="{REQUEST_REF}"]').click()
            page.locator(f'[data-request-detail="{REQUEST_REF}"]').click()
            page.locator(".study-request-detail").get_by_text("来源或候选当前不可用", exact=False).wait_for()
            page.locator(".study-request-detail").get_by_text("模型：synthetic-model", exact=True).wait_for()
            assert page.locator(".study-request-detail").get_by_text("合成请求快照", exact=False).count() == 0
            page.get_by_role("button", name="用户问题").first.click()
            page.locator(".study-problem-candidates .study-candidate-card").first.wait_for()
            page.locator(".study-problem-candidates .study-candidate-card").first.get_by_text("关键维度不同", exact=False).wait_for()
            assert page.get_by_text("尚无足够独立依据", exact=False).count() == 0
            page.locator("#problem-candidate-state").select_option("budget_stopped")
            page.locator(".study-problem-candidates .study-candidate-card .study-restricted").get_by_text("机器处理未完成", exact=False).wait_for()
            page.locator(".study-problem-candidates [data-candidate-signal]").get_by_text("查看运行与信号诊断", exact=True).wait_for()
            page.locator("#problem-candidate-state").select_option("pending")
            page.locator(".study-problem-candidates .study-candidate-card").get_by_text("待归并判断", exact=True).wait_for()
            assert page.locator(".study-problem-candidates .study-candidate-card .study-restricted").count() == 0
            page.locator("#problem-candidate-state").select_option("deferred_novel")
            page.locator(".study-problem-candidates .study-candidate-card").first.wait_for()
            page.locator(".study-problem-candidates [data-candidate-signal]").first.click()
            page.locator(f'[data-signal-ref="{SIGNAL_REF}"]').wait_for()
            page.go_back()
            page.locator(".study-problem-candidates .study-candidate-card").first.wait_for()
            page.goto(f"{base_url}/corpus/comments?domain={DOMAIN_REF}&view=runs&runRef={OLDER_RUN_REF}&panel=method")
            page.locator(".study-run-summary").get_by_text("历史调用记录未记录", exact=False).wait_for()
            assert page.locator(".study-run-summary").get_by_text("请求 0 次", exact=False).count() == 0
            page.close()

            for width, height, scale in ((1440, 900, 1), (1024, 768, 1),
                                         (390, 844, 1), (720, 450, 2)):
                viewport_page = browser.new_page(viewport={"width": width, "height": height},
                                                 device_scale_factor=scale)
                viewport_page.route("**/api/local/comment-study/**", api_reply)
                viewport_page.goto(f"{base_url}/corpus/comments?domain={DOMAIN_REF}&view=overview")
                viewport_page.locator("#study-observation-title").wait_for()
                for name, view in (("用户评论", "comments"), ("研究运行", "runs"),
                                   ("用户问题", "problems"), ("总览", "overview")):
                    viewport_page.locator(f'.study-tabs [data-view="{view}"]').click()
                    viewport_page.wait_for_url(f"**view={view}*")
                viewport_page.locator("#open-study-dialog").click()
                viewport_page.locator("#study-dialog[open]").wait_for()
                close = viewport_page.locator("#study-dialog-close")
                assert close.is_visible(), f"study dialog close hidden at {width} px / {scale}x"
                try:
                    close.click(timeout=2000)
                except Exception as error:
                    raise AssertionError(
                        f"study dialog close unreachable at {width}px/{scale}x; "
                        f"close_box={close.bounding_box()!r}; dialog_box={viewport_page.locator('#study-dialog').bounding_box()!r}"
                    ) from error
                viewport_page.locator("#study-dialog[open]").wait_for(state="hidden")
                horizontal = viewport_page.evaluate(
                    """() => ({scroll:document.documentElement.scrollWidth,
                      visible:document.documentElement.clientWidth,
                      offenders:[...document.querySelectorAll('*')].filter(element =>
                        element.getBoundingClientRect().right > innerWidth + 2 &&
                        getComputedStyle(element).position !== 'fixed').slice(0,12).map(element =>
                        [element.tagName,element.id,element.className,
                         Math.round(element.getBoundingClientRect().right)])})""")
                assert horizontal["scroll"] <= horizontal["visible"] + 2, (
                    f"page horizontal overflow at {width} px / {scale}x: {horizontal}")
                viewport_page.close()
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

        page.get_by_role("button", name="编辑方法").click()
        page.locator("#method-name").wait_for(state="visible")
        page.get_by_text("编辑副本已载入", exact=False).wait_for()
        page.locator("#method-name").fill("隔离浏览器方法")
        page.locator("#stage-semantic").fill("仅用于真实 Axum 与隔离 PostgreSQL 浏览器回归")
        page.locator("#save-policy").click()
        try:
            page.get_by_text("已保存并选中新方法版本", exact=False).wait_for()
            assert page.locator("#policy-form").is_hidden()
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
        page.locator("#run-comment-budget").fill("2")
        page.locator("#run-context-character-budget").fill("3500")
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
        assert policy and policy["payload"]["methodName"] == "隔离浏览器方法", (
            f"captured policy name={policy['payload'].get('methodName') if policy else None!r}"
        )
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


def run_live_e2e_readonly(base_url: str, domain_ref: str, run_ref: str,
                          problem_ref: str, proof_token: str) -> None:
    """Walk a seeded positive Problem through the real Axum router and isolated PG."""
    from urllib.parse import urlencode

    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=True)
        page = browser.new_page()
        page.set_default_timeout(15000)
        page_errors: list[str] = []
        writes: list[str] = []
        api_requests: list[str] = []
        api_responses: list[str] = []
        failed_requests: list[str] = []
        page.on("pageerror", lambda error: page_errors.append(str(error)))
        page.on("request", lambda request: writes.append(request.url)
                if request.method != "GET" and "/api/local/comment-study/" in request.url else None)
        page.on("request", lambda request: api_requests.append(f"{request.method} {urlsplit(request.url).path}")
                if "/api/local/comment-study/" in request.url else None)
        page.on("response", lambda response: api_responses.append(
            f"{response.request.method} {urlsplit(response.url).path} {response.status}")
                if "/api/local/comment-study/" in response.url else None)
        page.on("requestfailed", lambda request: failed_requests.append(
            f"{request.method} {urlsplit(request.url).path}: {request.failure}"))
        proof = page.request.get(f"{base_url}/__comment-study-browser-proof")
        assert proof.status == 200 and proof.text() == proof_token, "isolated proof handshake failed"
        target_url = f"{base_url}/api/local/comment-study/targets?{urlencode({'domain': domain_ref, 'runRef': run_ref, 'limit': 100})}"
        target_response = page.request.get(target_url)
        assert target_response.status == 200, f"seeded Run targets returned {target_response.status}"
        target_data = target_response.json()
        target = next((item for item in target_data.get("targets", [])
                       if item.get("commentKey", {}).get("commentExternalId") and item.get("signalCount", 0)), None)
        assert target, "seeded positive Problem has no navigable Target commentKey"
        key = target["commentKey"]
        comment_url = f"{base_url}/corpus/comments?{urlencode({'domain': domain_ref, 'view': 'comments', 'commentWorkRef': key['workRef'], 'commentExternalId': key['commentExternalId']})}"
        navigation = page.goto(comment_url)
        try:
            page.locator("#comment-detail-dialog[open]").wait_for()
        except Exception as error:
            raise AssertionError(
                "Initial comment deep link did not open its detail dialog; "
                f"navigation_status={navigation.status if navigation else None}; "
                f"url={page.url!r}; body={page.locator('body').inner_text()[:600]!r}; "
                f"page_errors={page_errors!r}; api_requests={api_requests[-30:]!r}; "
                f"api_responses={api_responses[-30:]!r}; failed_requests={failed_requests[-30:]!r}"
            ) from error
        page.get_by_role("heading", name="父评论语境").wait_for()
        history = page.locator(f'#comment-detail-body [data-history-run="{run_ref}"]')
        assert history.count() >= 3, "comment history did not link the seeded Run"
        history.filter(has_text="查看本次方法").first.click()
        page.get_by_role("heading", name="本次方法").wait_for()
        assert f"runRef={run_ref}" in page.url and "panel=method" in page.url
        page.locator(".study-method-detail").wait_for()

        page.get_by_role("tab", name="调用记录").click()
        page.get_by_role("heading", name="调用记录").wait_for()
        assert "panel=requests" in page.url
        page.locator("[data-request-detail]").first.click()
        page.locator(".study-request-detail").get_by_text("SYNTHETIC / NOT EVIDENCE", exact=False).wait_for()
        page.get_by_role("tab", name="研究信号").click()
        page.get_by_role("heading", name="研究结果").wait_for()
        problem_link = page.locator(f'[data-signal-problem="{problem_ref}"]').first
        problem_link.wait_for()
        problem_link.click()
        page.locator(f'[data-problem-ref="{problem_ref}"]').wait_for()
        page.locator(".study-problem-detail").wait_for()
        assert f"problemRef={problem_ref}" in page.url
        page.locator("[data-problem-close]").click()
        page.locator(".study-problem-detail").wait_for(state="hidden")
        assert "problemRef=" not in page.url
        page.locator(f'[data-problem-open="{problem_ref}"]').click()
        page.locator(".study-problem-detail").wait_for()
        assert f"problemRef={problem_ref}" in page.url
        page.locator(".study-problem-evidence blockquote").first.wait_for()
        page.locator(".study-problem-evidence [data-evidence-id]").first.click()
        page.locator("#comment-detail-dialog[open]").wait_for()
        assert "commentExternalId=" in page.url
        page.go_back()
        page.locator("#comment-detail-dialog").wait_for(state="hidden")
        page.locator(".study-problem-detail").wait_for()
        assert f"problemRef={problem_ref}" in page.url
        page.go_back()
        page.locator(".study-problem-detail").wait_for(state="hidden")
        assert "problemRef=" not in page.url
        page.go_back()
        page.locator(".study-problem-detail").wait_for()
        assert f"problemRef={problem_ref}" in page.url
        page.go_back()
        page.get_by_role("heading", name="研究结果").wait_for()
        page.get_by_role("tab", name="目标评论").click()
        page.locator(f'[data-target-ref="{target["targetRef"]}"]').first.wait_for()
        page.get_by_role("tab", name="本次方法").click()
        page.get_by_role("heading", name="本次方法").wait_for()
        page.get_by_role("button", name="用户问题").first.click()
        try:
            page.locator(".study-problem-candidates .study-candidate-card").first.wait_for()
        except Exception as error:
            candidate_reply = page.request.get(
                f"{base_url}/api/local/comment-study/problem-candidates?"
                f"{urlencode({'domain': domain_ref, 'limit': 5})}"
            )
            raise AssertionError(
                "Seeded deferred expression did not render; "
                f"candidate_api_status={candidate_reply.status}; "
                f"candidate_api_body={candidate_reply.text()[:900]!r}; "
                f"problem_section={page.locator('#study-tab-result').inner_text()[:900]!r}; "
                f"page_errors={page_errors!r}; api_responses={api_responses[-25:]!r}"
            ) from error
        page.locator("#problem-candidate-state").select_option("deferred_novel")
        candidate = page.locator(".study-problem-candidates .study-candidate-card").first
        candidate.wait_for()
        candidate.locator("[data-candidate-signal]").click()
        page.locator(f'[data-signal-ref]').first.wait_for()
        assert "panel=signals" in page.url
        page.go_back()
        try:
            page.locator(".study-problem-candidates .study-candidate-card").first.wait_for()
        except Exception as error:
            raise AssertionError(
                "Back from deferred Signal did not restore candidate list; "
                f"url={page.url!r}; problem_section={page.locator('#study-tab-result').inner_text()[:1100]!r}; "
                f"page_errors={page_errors!r}; api_responses={api_responses[-25:]!r}"
            ) from error
        page.locator(".study-problem-candidates [data-evidence-id]").first.click()
        page.locator("#comment-detail-dialog[open]").wait_for()
        page.go_back()
        page.locator(".study-problem-candidates .study-candidate-card").first.wait_for()
        assert not writes, f"read-only E2E unexpectedly wrote: {writes!r}"
        assert not page_errors, f"browser errors: {page_errors!r}"
        browser.close()
    print("Comment Study positive Problem Axum/PostgreSQL read-only browser E2E passed")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--api-base-url")
    parser.add_argument("--domain-ref")
    parser.add_argument("--work-ref")
    parser.add_argument("--existing-policy-ref")
    parser.add_argument("--initial-active-policy-ref")
    parser.add_argument("--proof-token")
    parser.add_argument("--e2e-run-ref")
    parser.add_argument("--e2e-problem-ref")
    args = parser.parse_args()
    if args.api_base_url:
        e2e = bool(args.e2e_run_ref or args.e2e_problem_ref)
        required = {"--domain-ref": args.domain_ref, "--proof-token": args.proof_token}
        if not e2e:
            required.update({"--work-ref": args.work_ref,
                             "--existing-policy-ref": args.existing_policy_ref,
                             "--initial-active-policy-ref": args.initial_active_policy_ref})
        missing = [name for name, value in required.items() if not value]
        if missing:
            parser.error("--api-base-url requires " + ", ".join(missing))
        if e2e:
            if not args.e2e_run_ref or not args.e2e_problem_ref:
                parser.error("positive Problem E2E requires both --e2e-run-ref and --e2e-problem-ref")
            run_live_e2e_readonly(args.api_base_url.rstrip("/"), args.domain_ref,
                                  args.e2e_run_ref, args.e2e_problem_ref, args.proof_token)
        else:
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
