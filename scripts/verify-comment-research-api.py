#!/usr/bin/env python3
"""Exercise the isolated SYNTHETIC preview only; never print source text or credentials."""
import json
import sys
import uuid
from urllib.parse import urlencode, urlparse
from urllib.request import Request, urlopen
from urllib.error import HTTPError

base = sys.argv[1] if len(sys.argv) == 2 else ""
parsed = urlparse(base)
if parsed.scheme != "http" or parsed.hostname != "127.0.0.1" or not parsed.port or parsed.path:
    raise SystemExit("Supply the loopback origin printed by preview-comment-research.sh")


def read(path, body=None, origin=None):
    headers = {"Content-Type": "application/json"} if body is not None else {}
    if origin:
        headers["Origin"] = origin
    request = Request(base + path, headers=headers,
                      data=json.dumps(body).encode() if body is not None else None)
    try:
        with urlopen(request) as response:
            assert response.headers["Cache-Control"] == "no-store"
            return response.status, json.load(response)
    except HTTPError as error:
        return error.code, None


path = "/api/local/comment-research"
status, data = read(path)
assert status == 200 and data["page"]["total"] == 24
assert len(data["page"]["items"]) == 20 and data["modelConnected"] is False
assert all("SYNTHETIC / NOT EVIDENCE" in item["body"] for item in data["page"]["items"]), "Refuse non-synthetic data"
assert all(work["title"].startswith("合成材料") for work in data["works"])
cursor = data["page"]["nextCursor"]
_, second = read(path + "?" + urlencode({"text": "", "cursor": cursor}))
assert second["page"]["total"] == 24 and len(second["page"]["items"]) == 4
first_ids = {item["sourceRef"] for item in data["page"]["items"]}
assert not first_ids.intersection(item["sourceRef"] for item in second["page"]["items"])
source_ref = data["page"]["items"][0]["sourceRef"]
_, detail = read(path + "/sources/" + source_ref)
assert detail["work"]["body"]["value"].startswith("SYNTHETIC / NOT EVIDENCE")
assert "author_external_id" not in json.dumps(detail)
asset = {"assetRef": str(uuid.uuid4()), "sourceRef": source_ref, "startChar": 0,
         "endChar": 9, "sourceSha256": detail["sourceSha256"], "reason": "合成 API 保存证明", "collectionRef": None}
assert read(path + "/assets", asset)[0] == 200
assert read(path + "/assets", asset)[0] == 200
conflict = dict(asset, reason="不同载荷不能复用保存标识")
assert read(path + "/assets", conflict)[0] == 409
assert read(path + "/assets", dict(asset, assetRef=str(uuid.uuid4()), endChar=99999))[0] == 400
query = {"queryRef": str(uuid.uuid4()), "name": "合成 API 查询", "text": "奖励表", "workRef": None}
assert read(path + "/queries", query)[0] == 200
assert read(path + "/queries", query, origin="https://foreign.example")[0] == 403
assert read(path + "/queries", query, origin=base)[0] == 200
_, queries = read(path + "/queries")
assert any(item["queryRef"] == query["queryRef"] for item in queries["items"])
_, groups = read(path + "/groups")
assert groups["groupingRule"] == "EXACT_LABEL_EQUALITY"
assert groups["items"][0]["origin"] == "human"
legacy = "/api/local/work-resources/" + detail["source"]["workRef"] + "/comments"
assert read(legacy)[0] == 200
assert read(legacy, origin="https://foreign.example")[0] == 403
collection = {"collectionRef": str(uuid.uuid4()), "name": "合成整理集合"}
assert read(path + "/collections", collection)[0] == 200
edit = {"revisionRef": str(uuid.uuid4()), "assetRef": asset["assetRef"], "expectedRevision": 0,
        "reason": "合成修订理由", "collectionRef": collection["collectionRef"], "withdrawn": False, "changeReason": "补充研究缘由"}
assert read(path + "/assets/revisions", edit, origin="https://foreign.example")[0] == 403
assert read(path + "/assets/revisions", edit)[0] == 200
assert read(path + "/assets/revisions", dict(edit, revisionRef=str(uuid.uuid4())))[0] == 409
_, organized = read(path + "/assets?" + urlencode({"collectionRef": collection["collectionRef"]}))
assert organized["total"] == 1 and organized["items"][0]["reason"] == "合成修订理由"
withdraw = dict(edit, revisionRef=str(uuid.uuid4()), expectedRevision=1, reason=None,
                collectionRef=None, withdrawn=True, changeReason="合成误收存撤销")
assert read(path + "/assets/revisions", withdraw)[0] == 200
assert read(path + "/assets/revisions", edit)[1]["state"] == "WITHDRAWN"
assert read(path + "/assets", asset)[1]["state"] == "WITHDRAWN"
_, history = read(path + "/assets/" + asset["assetRef"] + "/history")
assert len(history["items"]) == 3 and history["items"][0]["withdrawn"] is True
_, organized = read(path + "/assets?" + urlencode({"collectionRef": collection["collectionRef"]}))
assert organized["total"] == 0
query_edit = {"revisionRef": str(uuid.uuid4()), "queryRef": query["queryRef"], "expectedRevision": 0,
              "name": "合成改名查询", "text": "合成", "workRef": detail["source"]["workRef"], "deleted": False}
assert read(path + "/queries/revisions", query_edit)[0] == 200
assert read(path + "/queries/revisions", dict(query_edit, revisionRef=str(uuid.uuid4())))[0] == 409
_, queries = read(path + "/queries")
updated = next(q for q in queries["items"] if q["queryRef"] == query["queryRef"])
assert updated["name"] == "合成改名查询" and updated["text"] == "合成" and updated["workRef"] == detail["source"]["workRef"]
delete = dict(query_edit, revisionRef=str(uuid.uuid4()), expectedRevision=1, deleted=True)
assert read(path + "/queries/revisions", delete)[0] == 200
assert read(path + "/queries/revisions", query_edit)[1]["state"] == "DELETED"
assert read(path + "/queries", query)[1]["state"] == "DELETED"
_, queries = read(path + "/queries")
assert not any(q["queryRef"] == query["queryRef"] for q in queries["items"])
print("PASS: R1 asset/query revisions, collection organization, withdrawal/deletion and safe replay; synthetic API source/work context, pagination, exact asset persistence/replay/conflict, saved query, groups, old channel and Origin/no-store boundary")
