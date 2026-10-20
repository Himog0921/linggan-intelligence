#!/usr/bin/env python3
"""Check project-local LIDS alignment records; no business or network I/O."""

import argparse
import hashlib
from collections import Counter
from copy import deepcopy
import json
from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[1]
CATALOG = ROOT / "docs/design/lids/review-alignment.json"
VIEW = ROOT / "docs/design/lids/review-alignment.md"
TYPE_SOURCE = ROOT / "docs/design/lids/typography.json"
FOCUS_SOURCE = ROOT / "docs/design/lids/focus.json"
MATERIAL_SOURCE = ROOT / "docs/design/lids/control-material.json"
SHELL_SOURCE = ROOT / "docs/design/lids/shell-layout.json"
LEDGER_SOURCE = ROOT / "docs/design/lids/ledger-layout.json"
COMPOSITION_SOURCE = ROOT / "docs/design/lids/composition-layout.json"
OVERLAY_SOURCE = ROOT / "docs/design/lids/overlay-policy.json"
COMPONENT_SOURCE = ROOT / "docs/design/lids/component-reuse.json"
TYPE_VIEW = ROOT / "docs/design/lids/typography.md"
GROUPS = {
    "policies": (24, "规则"),
    "regions": (18, "区域"),
    "patterns": (11, "页型候选"),
    "components": (48, "组件候选"),
    "businessBlocks": (41, "业务构件候选"),
    "interactions": (32, "交互候选"),
    "pages": (17, "页面与模式"),
    "actions": (14, "动作预设候选"),
    "reviewDecisions": (8, "原审阅决定"),
}
STATES = {
    "confirmed": "已确认原则",
    "existing": "沿用既有规则",
    "partial": "原则采用／细节待审",
    "candidate": "候选待审",
}
RULE_MODULES = [
    "system", "typography", "tokens", "shell-zones", "regions", "patterns",
    "primitives", "materials", "data-boundaries", "interaction-contracts", "language-policy",
]


def catalog_errors(data):
    errors = []
    if data.get("id") != "LIDS-ALIGN-001":
        errors.append("unexpected alignment ID")
    if data.get("status") != "alignment_record_not_rule_source":
        errors.append("catalog must remain an alignment record, not a second rule source")
    for field in ["businessCodeChanged", "runtimeMigrated", "fontsMigrated", "allCandidatesApproved"]:
        if data.get("scope", {}).get(field) is not False:
            errors.append(f"unauthorized scope claim: {field}")
    if not re.fullmatch(r"[a-f0-9]{64}", data.get("source", {}).get("registrySha256", "")):
        errors.append("source registry fingerprint missing")
    if not re.fullmatch(r"[a-f0-9]{40}", data.get("baseline", "")):
        errors.append("baseline commit missing")
    evidence = {item["id"] for item in data.get("userEvidence", [])} | {"existing_lids"}
    groups = data.get("groups", {})
    if set(groups) != set(GROUPS):
        errors.append("inventory group coverage changed")
    seen = set()
    for key, (count, _) in GROUPS.items():
        rows = groups.get(key, [])
        if len(rows) != count:
            errors.append(f"{key}: expected {count} entries, got {len(rows)}")
        for row in rows:
            item_id = row.get("id", "")
            if not item_id or item_id in seen:
                errors.append(f"empty or duplicate ID: {item_id}")
            seen.add(item_id)
            state = row.get("state")
            if state not in STATES:
                errors.append(f"{item_id}: unknown state")
            if not row.get("basis") or not set(row["basis"]) <= evidence:
                errors.append(f"{item_id}: confirmation source missing or unknown")
            target = row.get("normativeTarget", "")
            if not target.startswith("docs/") or ".." in Path(target).parts or not (ROOT / target).is_file():
                errors.append(f"{item_id}: invalid normative target")
            pending = row.get("reviewItems", [])
            if not all(re.fullmatch(r"Q(0[1-9]|1[0-2])", q) for q in pending):
                errors.append(f"{item_id}: unknown review item")
            if state in ["partial", "candidate"] and not pending and row.get("validationStatus") != "not_adopted_by_reuse_classification":
                errors.append(f"{item_id}: pending detail lacks a review reference")
            if state == "confirmed" and pending:
                errors.append(f"{item_id}: confirmed label hides unresolved detail")
            if key not in ["policies", "regions"] and state in ["confirmed", "existing"] and not (key == "reviewDecisions" and item_id == "D06" and "U11" in row.get("basis", []) and target == "docs/design/lids/typography.md"):
                errors.append(f"{item_id}: candidate promoted without independent acceptance")
            if key == "components" and row.get("implementationStatus") != "not_promoted_by_alignment":
                errors.append(f"{item_id}: component certification is not part of this delivery")
            if key == "actions" and row.get("writeAuthorized") is not False:
                errors.append(f"{item_id}: action authorization claim")
    modes = sum(len(p.get("candidateRequirement", {}).get("modes", [])) for p in groups.get("pages", []))
    if modes != 26:
        errors.append(f"expected 26 page modes, got {modes}")
    adoption = data.get("typographyAdoption", {})
    if adoption.get("status") != "accepted_B_Q01_Q02_runtime_not_migrated" or "U09" not in adoption.get("basis", []):
        errors.append("B typography adoption lacks the explicit user decision")
    if adoption.get("resolved") != ["Q01", "Q02", "Q03", "Q11"] or adoption.get("pending") != []:
        errors.append("typography adoption changed the review boundary")
    if adoption.get("normativeSource") != "docs/design/lids/typography.json" or adoption.get("sha256") != hashlib.sha256(TYPE_SOURCE.read_bytes()).hexdigest():
        errors.append("typography adoption drifted from the single parameter source")
    if "typographyReviewCandidate" in data:
        errors.append("obsolete typography candidate cannot coexist as a current standard")
    for rows in groups.values():
        for row in rows:
            if set(row.get("reviewItems", [])) & {"Q01", "Q02", "Q03", "Q04", "Q05", "Q06", "Q07", "Q08", "Q09", "Q10", "Q11", "Q12"}:
                errors.append(f"{row.get('id')}: resolved typography review reopened")
    voice = data.get("voiceReview", {})
    if voice.get("status") != "accepted_sans_runtime_not_migrated" or voice.get("adoptedFamily") != "OPPO Sans 4.0" or "U11" not in voice.get("basis", []) or voice.get("serifExampleAdopted") is not False:
        errors.append("Q03 must keep the explicit OPPO Sans decision and historical Serif boundary")
    focus = data.get("focusReview", {})
    if focus.get("status") != "accepted_C_runtime_not_migrated" or focus.get("selected") != "C" or "U12" not in focus.get("basis", []) or focus.get("businessRequests") is not False or focus.get("BRoleTuplesChanged") is not False or focus.get("tertiaryHost") != "Z02":
        errors.append("Q04 must preserve the explicit C adoption and unchanged boundaries")
    if focus.get("normativeSource") != "docs/design/lids/focus.json" or focus.get("sha256") != hashlib.sha256(FOCUS_SOURCE.read_bytes()).hexdigest():
        errors.append("Q04 must read the single accepted focus source")
    material = data.get("materialReview", {})
    if material.get("status") != "accepted_M2_runtime_not_migrated" or material.get("selected") != "M2" or "U13" not in material.get("basis", []) or material.get("businessRequests") is not False or material.get("runtimeMigrated") is not False or material.get("localExceptionsUnchanged") != ["M-02E", "M-06", "V8_workbench"]:
        errors.append("Q05 must retain explicit M2 adoption, existing exceptions and runtime boundary")
    if material.get("normativeSource") != "docs/design/lids/control-material.json" or material.get("sha256") != hashlib.sha256(MATERIAL_SOURCE.read_bytes()).hexdigest():
        errors.append("Q05 must read the single accepted M2 material source")
    shell = data.get("shellReview", {})
    if shell.get("status") != "accepted_A_runtime_not_migrated" or shell.get("selected") != "A" or "U15" not in shell.get("basis", []) or shell.get("manualSidebarCollapse") is not True or shell.get("domainHost") != "Z02" or shell.get("tertiaryHost") != "Z02" or shell.get("businessRequests") is not False or shell.get("runtimeMigrated") is not False or shell.get("BRoleTuplesChanged") is not False or shell.get("acceptedMaterialsChanged") is not False:
        errors.append("Q06 must retain A adoption, manual icon collapse, Z02 ownership and fixed boundaries")
    if shell.get("normativeSource") != "docs/design/lids/shell-layout.json" or shell.get("sha256") != hashlib.sha256(SHELL_SOURCE.read_bytes()).hexdigest():
        errors.append("Q06 shell source/mirror adoption drift")
    ledger = data.get("ledgerReview", {})
    if ledger.get("status") != "accepted_L1_runtime_not_migrated" or ledger.get("selected") != "L1" or "U16" not in ledger.get("basis", []) or ledger.get("rowHeight") != 44 or ledger.get("nameColumnWidth") != 144 or ledger.get("fieldCustomization") is not True or ledger.get("singleTaskSingleRow") is not True or any(ledger.get(k) is not False for k in ["fontShrinking", "rowWrapping", "automaticFieldHiding", "businessRequests", "runtimeMigrated"]):
        errors.append("Q07 must retain L1/44px/name144px, customization, one row and fixed boundaries")
    if ledger.get("normativeSource") != "docs/design/lids/ledger-layout.json" or ledger.get("sha256") != hashlib.sha256(LEDGER_SOURCE.read_bytes()).hexdigest():
        errors.append("Q07 single accepted ledger source drift")
    composition = data.get("compositionReview", {})
    if composition.get("status") != "accepted_B_search_C_functional_runtime_not_migrated" or "U18" not in composition.get("basis", []) or composition.get("selected") != {"search":"B","functional":"C","entry":"A"} or composition.get("preserveProblemCards") is not True or composition.get("domainHost") != "Z02" or composition.get("tertiaryHost") != "Z02" or any(composition.get(k) is not False for k in ["businessRequests", "runtimeMigrated", "componentCertificationChanged", "pageContractsApproved"]) or composition.get("localExceptionsUnchanged") != ["M-02E", "M-06", "V8_workbench"]:
        errors.append("Q08 must retain B search/C functional, preserved cards, Z02 and no business authorization")
    if composition.get("normativeSource") != "docs/design/lids/composition-layout.json" or composition.get("sha256") != hashlib.sha256(COMPOSITION_SOURCE.read_bytes()).hexdigest():
        errors.append("Q08 accepted composition source drift")
    if any("Q08" in row.get("reviewItems", []) for rows in groups.values() for row in rows):
        errors.append("accepted Q08 visual composition cannot remain pending")
    overlay = data.get("overlayReview", {})
    if overlay.get("status") != "accepted_S1_runtime_not_migrated" or overlay.get("selected") != "S1" or "U20" not in overlay.get("basis", []) or overlay.get("domainHost") != "Z02" or overlay.get("tertiaryHost") != "Z02" or overlay.get("unresolvedFormsProtected") is not True or overlay.get("originalRequestTracking") is not True or any(overlay.get(k) is not False for k in ["businessRequests", "runtimeMigrated", "fixedStandardsChanged", "problemCardsChanged", "componentCertificationChanged", "stoppedCanResume"]):
        errors.append("Q09 must retain accepted S1, draft protection, tracking, terminal stop, fixed cards/Z02 and no business authority")
    if overlay.get("normativeSource") != "docs/design/lids/overlay-policy.json" or overlay.get("sha256") != hashlib.sha256(OVERLAY_SOURCE.read_bytes()).hexdigest():
        errors.append("Q09 accepted S1 source drift")
    components = data.get("componentReview", {})
    if components.get("status") != "accepted_four_classes_not_component_certification" or "U21" not in components.get("basis", []) or components.get("inventory") != 48 or components.get("gateCount") != 7 or components.get("formalCMPsGranted") != 0 or components.get("tertiaryHost") != "Z02" or components.get("domainHost") != "Z02" or any(components.get(k) is not False for k in ["businessRequests", "runtimeMigrated", "fixedStandardsChanged", "problemCardsChanged"]):
        errors.append("Q10 retains four accepted classes, seven gates and no automatic CMP or business change")
    if components.get("normativeSource") != "docs/design/lids/component-reuse.json" or components.get("sha256") != hashlib.sha256(COMPONENT_SOURCE.read_bytes()).hexdigest():
        errors.append("Q10 accepted reuse source drift")
    fonts = data.get("fontReview", {})
    if fonts.get("status") != "accepted_delivery_runtime_not_migrated" or "U22" not in fonts.get("basis", []) or fonts.get("selectedFallback") != "F2" or fonts.get("automaticApplication") is not True or fonts.get("ordinaryAlignment") != "right" or fonts.get("decimalColumnAlignment") != "decimal_point" or fonts.get("fixedFonts") != ["OPPO Sans 4.0", "Albert Sans", "Alumni Sans", "Manrope"] or fonts.get("tertiaryHost") != "Z02" or fonts.get("domainHost") != "Z02" or any(fonts.get(k) is not False for k in ["BRoleValuesChanged", "sourceFontAssetsChanged", "businessRequests", "runtimeMigrated", "problemCardsChanged"]):
        errors.append("Q11 requires adopted auto/F2/numeric policy with fixed B/card/Z02 and no business/asset change")
    if fonts.get("normativeSource") != "docs/design/lids/typography.json" or fonts.get("sha256") != hashlib.sha256(TYPE_SOURCE.read_bytes()).hexdigest():
        errors.append("Q11 single typography source drift")
    exceptions = data.get("materialExceptionsReview", {})
    if exceptions.get("status") != "accepted_materials_runtime_not_migrated" or "U23" not in exceptions.get("basis", []) or exceptions.get("selected") != "E1" or exceptions.get("stageField") != "plain" or exceptions.get("domainHost") != "Z02" or exceptions.get("tertiaryHost") != "Z02" or any(exceptions.get(k) is not False for k in ["businessRequests", "runtimeMigrated", "fontAssetsChanged", "problemCardsChanged", "fixedStandardsChanged"]):
        errors.append("Q12 requires E1/plain with registered exceptions and fixed standards")
    media_path = ROOT / "docs/design/lids/media-material.json"
    media = json.loads(media_path.read_text())
    if exceptions.get("sha256") != hashlib.sha256(media_path.read_bytes()).hexdigest() or media.get("decision") != {"selected": "E1", "stageField": "plain", "evidence": "U23"} or media.get("runtimeMigrated") is not False:
        errors.append("Q12 accepted material source drift")
    if voice.get("fixedNumericSource") != "docs/design/lids/typography.json" or voice.get("sourceText") != "synthetic_not_real_evidence":
        errors.append("Q03 must preserve B and use declared synthetic material")
    navigation = next((row for row in groups.get("regions", []) if row.get("id") == "Z04-N"), {})
    if navigation.get("candidateRequirement", {}).get("physicalHost") != "Z02" or "U10" not in navigation.get("basis", []):
        errors.append("tertiary page navigation must use the confirmed breadcrumb host")
    return errors


def typography_errors(standard):
    errors = []
    if standard.get("id") != "LIDS-TYPE-001-B" or standard.get("status") != "accepted" or standard.get("runtimeMigrated") is not False:
        errors.append("B specification and runtime migration states must remain separate")
    roles = standard.get("roles", {})
    expected = {"aux", "label", "control", "input", "body", "reading", "section", "object", "page", "data", "metric", "metricLarge", "display", "displayCompact", "displaySample"}
    if set(roles) != expected or set(standard.get("roleLabels", {})) != expected:
        errors.append("B typography role coverage changed")
    for role, values in roles.items():
        if set(values) != {"size", "weight", "lineHeight"}:
            errors.append(f"{role}: typography requires a complete role tuple")
            continue
        if not all(type(v) in [int, float] for v in values.values()):
            errors.append(f"{role}: numeric role values required")
            continue
        if values["size"] < 11 or values["weight"] not in [300, 400, 500, 600, 700] or not .9 <= values["lineHeight"] <= 2.2:
            errors.append(f"{role}: invalid B typography values")
    if standard.get("families", {}).get("evidence") != standard.get("families", {}).get("chinese") or standard.get("voiceDecision", {}).get("evidence") != "U11" or "Q03" in standard.get("pendingReviewItems", []):
        errors.append("Q03 accepted evidence family drift")
    delivery = standard.get("deliveryDecision", {})
    if delivery.get("id") != "ALIGN-FONT-012" or delivery.get("evidence") != "U22" or delivery.get("status") != "accepted" or delivery.get("automaticApplication") is not True or delivery.get("fontDisplay") != "swap" or delivery.get("fallback") != "F2" or delivery.get("numeric") != {"features": ["tabular-nums", "lining-nums"], "ordinaryAlignment": "right", "decimalColumnAlignment": "decimal_point", "preserveOriginalScale": True, "unknownAsZero": False} or "Q11" in standard.get("pendingReviewItems", []):
        errors.append("Q11 delivery policy drift")
    return errors


def render_typography(standard):
    lines = ["<!-- TYPE-B:BEGIN -->", "", "| 角色 | 字号px | 字重 | 行高 |", "| --- | --- | --- | --- |"]
    for role, value in standard["roles"].items():
        lines.append(f"| {standard['roleLabels'][role]} `{role}` | {value['size']} | {value['weight']} | {value['lineHeight']} |")
    lines.extend(["", "<!-- TYPE-B:END -->"])
    return "\n".join(lines)


def render(data):
    def cell(value):
        return str(value).replace("|", "／").replace("\n", " ")

    lines = [
        "# LIDS-ALIGN-001 · 审阅稿逐项对齐表", "",
        "> 状态: 权威当前", "> 最后核对: 2026-10-20",
        "> 适用范围: 设计采用追踪与候选导航；不是另一个设计规则源",
        "> 事实来源: review-alignment.json及其中记录的用户确认、审阅源SHA和代码基线",
        "> 冲突时以谁为准: 用户最新确认及引用的正式LIDS分册；候选要求不自动生效", "",
        "由 `python3 scripts/verify-lids-alignment.py --render` 从 [清单源](review-alignment.json)生成；表格不手改。", "",
        "先查当前任务的ID，再读对应规则模块；只在需要审阅候选细节时打开JSON中的candidateRequirement。", "",
        "## 采用口径", "",
        "- 已确认原则：按正式落点执行；仅确认列出的范围。",
        "- 沿用既有规则：语义继续有效，具体适用依据仍需PAGE/数据合同。",
        "- 原则采用／细节待审：已确认部分见正式分册，JSON候选参数不作为默认。",
        "- 候选待审：只用于讨论与盘点，不代表批准、组件交付或业务能力。", "",
        "字体角色与完整B字号、字重、行高已确认，Q01/Q02已关闭，Q03原声已统一OPPO Sans；唯一参数源为[typography.json](typography.json)。Q04已按ALIGN-FOCUS-005采用C；Q05已按ALIGN-MATERIAL-006采用M2，唯一参数源[control-material.json](control-material.json)；Q06已按ALIGN-SHELL-007采用A及手动图标折叠；Q07已按ALIGN-LEDGER-008采用L1/44px/名称144px；Q08按ALIGN-COMPOSITION-009采用含搜索的列表B/其他功能C并保留用户问题卡片；Q09已采用S1，Q10四类/七门槛已采用，Q11已采用自动/F2/数值对齐；Q12按ALIGN-MEDIA-013采用E1与白场舞台。",
        "业务代码、字体资产和运行时Token未改；分支文档入库不等于主线或常驻runtime同步。", "",
        f"原审阅注册表 SHA256：`{data['source']['registrySha256']}`。",
        f"本次代码基线：`{data['baseline']}`。完整确认摘要见清单userEvidence。", "",
        "## 已确认的跨模块规则", "",
        "| 规则 | 正式落点 | 未确认部分 |", "| --- | --- | --- |",
        "| 无边框V2，连续白场、黑结构、橙重音；v8仍为粗稿 | [system](system.md) | Q05控件材质已采用M2；E1/白场舞台已采用 |",
        "| OPPO / Albert / Alumni / Manrope及B字级整体采用 | [typography](typography.md) | 原声OPPO已确认／生产接入Q11 |",
        "| 领域保留面包屑位置，位置与范围分责 | [shell-zones](shell-zones.md) | A容量及图标折叠已采用；组件认证Q10 |",
        "| 三级页面Tab统一归Z02面包屑模块，主区不重复 | [shell-zones](shell-zones.md) | A容量／收纳已采用；对象内部Tab与筛选各自就近 |",
        "| 区域可定位、标题去重、列表紧凑计数 | [regions](regions.md) | Q08组合及动作位置已采用；组件认证Q10 |",
        "| A连续账本，一任务一行 | [regions](regions.md) | 44px/名称144px已采用；B外部组合已采用；组件认证Q10 |",
        "| FIND保留装饰结构，输入/FIND聚焦不加外圈 | [primitives](primitives.md) | C焦点、M2材质已确认；组件认证Q10 |",
        "| 按钮、领域、面包屑、侧栏沿真实页面纹理方向 | [materials](materials.md) | M2参数已确认；E1局部例外/白场舞台已采用 |",
        "| Agent入口渐进展开，不自动推广候选或扩大授权 | [治理协议](../../governance/ui-design-execution.md) | 业务接入与主线集成独立 |", "",
    ]
    for key, (count, name) in GROUPS.items():
        lines.extend([f"## {name} · {count}项", "", "| ID | 名称 | 状态 | 正式规则落点 | 依据 | 待审 |", "| --- | --- | --- | --- | --- | --- |"])
        for row in data["groups"][key]:
            target = Path(row["normativeTarget"])
            relative = "../../" + str(target.relative_to("docs"))
            lines.append("| " + " | ".join([
                row["id"], cell(row["name"]), STATES[row["state"]],
                f"[{target.stem}]({relative})", ", ".join(row["basis"]),
                ", ".join(row["reviewItems"]) or "—",
            ]) + " |")
        lines.append("")
    lines.extend(["## 覆盖与边界", "", "共213项，P类含26模式；候选的结构、状态、交互引用保存在清单，可按ID读取。",
                  "本表完整性检查不验证真实DOM/CSS、视觉质量、数据来源、行为正确性、权限或业务结果；[待审清单](review-items.md)和[决定台账](decisions.md)分别记录待决与正式采用。", ""])
    return "\n".join(lines)


def routing_errors():
    errors = []
    route = "docs/governance/ui-design-execution.md"
    for file in ["AGENTS.md", "CLAUDE.md", "codx.md"]:
        text = (ROOT / file).read_text()
        if route not in text:
            errors.append(f"{file}: missing project UI protocol route")
    protocol = (ROOT / route).read_text()
    for level in ["L0", "L1", "L2", "L3"]:
        if level not in protocol:
            errors.append(f"missing disclosure level: {level}")
    for module in RULE_MODULES:
        if f"../design/lids/{module}.md" not in protocol:
            errors.append(f"missing module trigger route: {module}")
    pending = (ROOT / "docs/design/lids/review-items.md").read_text()
    for number in range(1, 13):
        if f"Q{number:02}" not in pending:
            errors.append(f"pending review Q{number:02} missing")
    if "codx.md)" not in (ROOT / "scripts/check-project-governance.sh").read_text():
        errors.append("root alias not admitted by project governance")
    return errors


def self_test(data):
    mutations = [
        ("false-runtime-migration", lambda d: d["scope"].update(runtimeMigrated=True)),
        ("false-all-approved", lambda d: d["scope"].update(allCandidatesApproved=True)),
        ("component-promoted", lambda d: d["groups"]["components"][0].update(state="confirmed", reviewItems=[])),
        ("unknown-evidence", lambda d: d["groups"]["policies"][0].update(basis=["invented"])),
        ("lost-review-boundary", lambda d: d["groups"]["patterns"][0].update(reviewItems=[], validationStatus="")),
        ("confirmed-with-pending", lambda d: d["groups"]["policies"][0].update(reviewItems=["Q01"])),
        ("duplicate-id", lambda d: d["groups"]["regions"][0].update(id="R01")),
        ("lost-typography-adoption", lambda d: d["typographyAdoption"].update(status="candidate")),
        ("reopened-resolved-review", lambda d: d["groups"]["components"][0]["reviewItems"].append("Q01")),
        ("second-typography-source", lambda d: d.update(typographyReviewCandidate={"status": "current"})),
        ("reopen-Q03-or-adopt-serif", lambda d: d["voiceReview"].update(status="pending_user_review", serifExampleAdopted=True)),
        ("reopened-Q04", lambda d: d["focusReview"].update(status="pending_user_review", selected="A")),
        ("reopened-Q05", lambda d: d["materialReview"].update(status="pending_user_review", selected="M1")),
        ("reopened-Q06", lambda d: d["shellReview"].update(status="pending_user_review", selected="B")),
        ("Q06-lost-sidebar-collapse", lambda d: d["shellReview"].update(manualSidebarCollapse=False)),
        ("reopened-Q07", lambda d: d["ledgerReview"].update(status="pending_user_review", selected="L2")),
        ("Q07-row-height-drift", lambda d: d["ledgerReview"].update(rowHeight=48)),
        ("reopened-Q08", lambda d: d["compositionReview"].update(status="pending_user_review")),
        ("Q08-search-wrong-recipe", lambda d: d["compositionReview"]["selected"].update(search="A")),
        ("Q08-functional-wrong-recipe", lambda d: d["compositionReview"]["selected"].update(functional="B")),
        ("Q08-problem-cards-lost", lambda d: d["compositionReview"].update(preserveProblemCards=False)),
        ("reopened-Q09", lambda d: d["overlayReview"].update(status="pending_user_review",selected="S2")),
        ("Q09-changed-problem-cards", lambda d: d["overlayReview"].update(problemCardsChanged=True)),
        ("Q09-business-authorization", lambda d: d["overlayReview"].update(businessRequests=True)),
        ("Q09-lost-draft-protection", lambda d: d["overlayReview"].update(unresolvedFormsProtected=False)),
        ("Q09-lost-original-request", lambda d: d["overlayReview"].update(originalRequestTracking=False)),
        ("Q09-restored-stopped-run", lambda d: d["overlayReview"].update(stoppedCanResume=True)),
        ("Q10-reopened", lambda d: d["componentReview"].update(status="pending_user_review")),
        ("Q10-false-CMPs", lambda d: d["componentReview"].update(formalCMPsGranted=48)),
        ("Q10-changed-cards", lambda d: d["componentReview"].update(problemCardsChanged=True)),
        ("Q10-lowered-gates", lambda d: d["componentReview"].update(gateCount=6)),
        ("Q11-reopened", lambda d: d["fontReview"].update(status="pending_user_review")),
        ("Q11-wrong-fallback", lambda d: d["fontReview"].update(selectedFallback="F1")),
        ("Q11-manual-only", lambda d: d["fontReview"].update(automaticApplication=False)),
        ("Q11-decimal-right", lambda d: d["fontReview"].update(decimalColumnAlignment="right")),
        ("Q12-reopened", lambda d: d["materialExceptionsReview"].update(status="pending_user_review")),
        ("Q12-measurement-grid", lambda d: d["materialExceptionsReview"].update(stageField="existing")),
        ("Q12-business-writes", lambda d: d["materialExceptionsReview"].update(businessRequests=True)),
        ("Q11-changed-font-assets", lambda d: d["fontReview"].update(sourceFontAssetsChanged=True)),
        ("Q11-changed-B", lambda d: d["fontReview"].update(BRoleValuesChanged=True)),
        ("Q08-moved-tertiary", lambda d: d["compositionReview"].update(tertiaryHost="Z04")),
        ("Q07-wrap-or-shrink", lambda d: d["ledgerReview"].update(rowWrapping=True, fontShrinking=True)),
        ("Q06-moved-domain", lambda d: d["shellReview"].update(domainHost="Z04")),
        ("tertiary-tabs-back-in-workspace", lambda d: next(row for row in d["groups"]["regions"] if row["id"] == "Z04-N")["candidateRequirement"].update(physicalHost="Z04")),
        ("action-authorized", lambda d: d["groups"]["actions"][0].update(writeAuthorized=True)),
        ("missing-page", lambda d: d["groups"]["pages"].pop()),
    ]
    for name, mutate in mutations:
        bad = deepcopy(data)
        mutate(bad)
        if not catalog_errors(bad):
            raise AssertionError(f"negative case unexpectedly accepted: {name}")
    return len(mutations)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--render", action="store_true", help="regenerate only the registered alignment Markdown")
    parser.add_argument("--check", action="store_true", help="verify inventory, route coverage and derived Markdown")
    parser.add_argument("--self-test", action="store_true", help="reject in-memory negative declarations")
    parser.add_argument("--item", metavar="ID", help="disclose one alignment item without loading the full catalog into agent context")
    parser.add_argument("--list", choices=list(GROUPS), help="disclose one group's IDs, names and adoption states")
    args = parser.parse_args()
    if (args.item or args.list) and (args.render or args.check or args.self_test):
        parser.error("query options are read-only; run checks or rendering separately")
    if args.item and args.list:
        parser.error("choose one item or one group")
    data = json.loads(CATALOG.read_text())
    standard = json.loads(TYPE_SOURCE.read_text())
    focus_source = json.loads(FOCUS_SOURCE.read_text())
    errors = catalog_errors(data) + typography_errors(standard) + routing_errors()
    if focus_source.get("status") != "accepted" or focus_source.get("decision", {}).get("selected") != "C" or focus_source.get("decision", {}).get("evidence") != "U12" or focus_source.get("runtimeMigrated") is not False or focus_source.get("externalPerimeter") is not False or focus_source.get("fontRolesChanged") is not False:
        errors.append("accepted focus source drifted from C or changed authorization")
    material_source = json.loads(MATERIAL_SOURCE.read_text())
    if material_source.get("status") != "accepted" or material_source.get("decision", {}).get("selected") != "M2" or material_source.get("decision", {}).get("evidence") != "U13" or material_source.get("runtimeMigrated") is not False or material_source.get("componentCertificationChanged") is not False or material_source.get("localExceptionsUnchanged") != ["M-02E", "M-06", "V8_workbench"]:
        errors.append("accepted M2 material source changed decision or authorization")
    shell_source = json.loads(SHELL_SOURCE.read_text())
    if shell_source.get("status") != "accepted" or shell_source.get("decision", {}).get("selected") != "A" or shell_source.get("decision", {}).get("evidence") != "U15" or shell_source.get("sidebar", {}).get("manualToggle") is not True or shell_source.get("runtimeMigrated") is not False or "Q06" in standard.get("pendingReviewItems", []):
        errors.append("accepted A shell and pending boundary drift")
    ledger_source = json.loads(LEDGER_SOURCE.read_text())
    if ledger_source.get("status") != "accepted" or ledger_source.get("decision", {}).get("selected") != "L1" or ledger_source.get("decision", {}).get("evidence") != "U16" or ledger_source.get("decision", {}).get("rowHeight") != 44 or ledger_source.get("widths", {}).get("name") != 144 or ledger_source.get("runtimeMigrated") is not False or ledger_source.get("businessFieldProjectionApproved") is not False or "Q07" in standard.get("pendingReviewItems", []):
        errors.append("accepted L1 ledger and pending projection boundary drift")
    composition_source = json.loads(COMPOSITION_SOURCE.read_text())
    if composition_source.get("status") != "accepted" or composition_source.get("decision", {}).get("evidence") != "U18" or composition_source.get("decision", {}).get("search") != "B" or composition_source.get("decision", {}).get("functional") != "C" or composition_source.get("preservedCards", {}).get("tableReplacementAllowed") is not False or len(composition_source.get("modes", {})) != 26 or any(composition_source.get(k) is not False for k in ["runtimeMigrated", "businessRequests", "componentCertificationChanged", "pageContractsApproved"]) or "Q08" in standard.get("pendingReviewItems", []):
        errors.append("accepted Q08 composition/card or pending boundary drift")
    overlay_source = json.loads(OVERLAY_SOURCE.read_text())
    if overlay_source.get("status") != "accepted" or overlay_source.get("decision", {}).get("selected") != "S1" or overlay_source.get("decision", {}).get("evidence") != "U20" or overlay_source.get("policy", {}).get("dirtyLeave") != "ask_continue_discard_or_save_then_leave" or overlay_source.get("policy", {}).get("pendingLeave") != "explicit_detach_with_original_request_tracking" or overlay_source.get("longTask", {}).get("stoppedCanResume") is not False or overlay_source.get("runtimeMigrated") is not False or "Q09" in standard.get("pendingReviewItems", []):
        errors.append("accepted Q09 S1 interaction source or pending boundary drift")
    component_source = json.loads(COMPONENT_SOURCE.read_text())
    if component_source.get("status") != "accepted" or component_source.get("decision", {}).get("evidence") != "U21" or set(component_source.get("routes", {})) != {"shared", "adapter", "business", "gap"} or len(component_source.get("gates", [])) != 7 or len(component_source.get("components", [])) != 48 or component_source.get("formalCMPsGranted") != 0 or component_source.get("runtimeMigrated") is not False or any(c.get("certified") is not False for c in component_source.get("components", [])) or "Q10" in standard.get("pendingReviewItems", []):
        errors.append("accepted Q10 four classes/seven gates or certification boundary drift")
    if "Q05" in standard.get("pendingReviewItems", []):
        errors.append("accepted Q05 material review cannot remain pending")
    if errors:
        for error in errors:
            print("LIDS alignment error:", error)
        return 1
    if args.item:
        item = next((row for rows in data["groups"].values() for row in rows if row["id"] == args.item), None)
        if item is None:
            print("LIDS alignment error: unknown item", args.item)
            return 1
        print(json.dumps({"authority": "Read normativeTarget first; candidateRequirement is not an implementation default.",
                          "item": item}, ensure_ascii=False, indent=2))
        return 0
    if args.list:
        print(json.dumps([{key: row[key] for key in ["id", "name", "state", "normativeTarget", "reviewItems"]}
                          for row in data["groups"][args.list]], ensure_ascii=False, indent=2))
        return 0
    expected = render(data)
    original_type_view = TYPE_VIEW.read_text()
    expected_type_view = re.sub(r"<!-- TYPE-B:BEGIN -->.*?<!-- TYPE-B:END -->", lambda _: render_typography(standard), original_type_view, flags=re.S)
    if "<!-- TYPE-B:BEGIN -->" not in original_type_view:
        print("LIDS alignment error: missing generated typography table boundary")
        return 1
    if args.render:
        VIEW.write_text(expected)
        TYPE_VIEW.write_text(expected_type_view)
    if args.check or not args.render:
        if not VIEW.exists() or VIEW.read_text() != expected:
            print("LIDS alignment error: derived view drift; use --render")
            return 1
        if TYPE_VIEW.read_text() != expected_type_view:
            print("LIDS alignment error: typography table drift; use --render")
            return 1
    negative_cases = self_test(data) if args.self_test else 0
    counts = Counter(row["state"] for rows in data["groups"].values() for row in rows)
    print(json.dumps({"result": "passed", "entries": sum(counts.values()), "states": dict(counts),
                      "pageModes": 26, "negativeCases": negative_cases,
                      "scope": "project document routing and declaration integrity only"}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
