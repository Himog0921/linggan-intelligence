import { c as cls } from "./styles";
import { badge } from "./topicElements";
import { esc, known, labelState, list, number } from "./topicModel";
import type * as T from "./types";
export function researchPhases(run: { phases?: T.Phases }) {
  if (!run.phases) return "";
  const p = run.phases;
  return (
    <>
      <div className={cls("lgi-tm-research-phases")}>
        <p>
          <strong>{"提炼讨论"}</strong>
          {[
            " · 进行中 ",
            number(p.extracting),
            " · 待处理 ",
            number(p.extractQueued),
          ].join("")}
        </p>
        <p>
          <strong>{"判断归属"}</strong>
          {[
            " · 进行中 ",
            number(p.resolving),
            " · 待处理 ",
            number(p.resolveQueued),
          ].join("")}
        </p>
        <p>
          <strong>{"比较讨论"}</strong>
          {[
            " · 进行中 ",
            number(p.comparing),
            " · 待处理 ",
            number(p.compareQueued),
          ].join("")}
        </p>
      </div>
    </>
  );
}
export function validationDetails(diagnostic: T.Diagnostic | null | undefined) {
  if (!diagnostic || typeof diagnostic !== "object") return "";
  const rules: Record<string, string> = {
    required_property: "缺少必填字段",
    unknown_property: "含有合同外字段",
    type: "字段类型错误",
    type_or_format: "字段类型或格式错误",
    enum: "字段值不属于允许范围",
    min_length: "字段长度低于下限",
    max_length: "字段长度超过上限",
    min_items: "条目数量低于下限",
    max_items: "条目数量超过上限",
    fragment_index: "引用索引不在本次材料中",
    contract: "输出合同版本错误",
    json: "JSON 格式错误",
    minimum: "数值低于下限",
  };
  const rule =
    typeof diagnostic.rule === "string" && Object.hasOwn(rules, diagnostic.rule)
      ? rules[diagnostic.rule]
      : null;
  const fields = new Set([
    "contract",
    "outcome",
    "discussions",
    "scenes",
    "journey",
    "responseMatches",
    "angles",
    "productOpportunities",
    "limitations",
    "label",
    "statement",
    "definition",
    "inclusionCriteria",
    "exclusionCriteria",
    "speakerRole",
    "evidenceRole",
    "rationale",
    "topicRef",
    "evidence",
    "fragmentId",
    "quote",
    "checks",
    "checkId",
    "verdict",
    "start",
    "end",
    "mainStage",
    "involvedStages",
    "overlays",
    "path",
    "status",
    "unanswered",
    "title",
    "answerTask",
    "need",
    "hypothesis",
    "verificationQuestion",
    "alternativeExplanation",
    "decisions",
    "unitId",
    "matches",
    "definitionRef",
    "reason",
    "proposedTopic",
    "relations",
    "relation",
    "domainFit",
    "domainReason",
    "abstractionReason",
    "parent",
    "kind",
    "proposal",
  ]);
  const path = diagnostic.schemaPath;
  const safePath =
    typeof path === "string" &&
    path.length <= 300 &&
    (path === "" ||
      (/^[A-Za-z][A-Za-z0-9]*(?:\[\d{1,6}\]|\.[A-Za-z][A-Za-z0-9]*)*$/.test(
        path,
      ) &&
        path
          .split(/[.[\]]/)
          .filter(Boolean)
          .every((part) => /^\d{1,6}$/.test(part) || fields.has(part))));
  const counts = (
    [
      ["candidateCount", "召回候选"],
      ["unitCount", "本次讨论"],
      ["decisionCount", "归属决定"],
    ] as const
  )
    .filter(
      ([key]) =>
        typeof diagnostic[key] === "number" &&
        Number.isSafeInteger(diagnostic[key]) &&
        diagnostic[key] >= 0 &&
        diagnostic[key] <= 64,
    )
    .map(([key, label]) => `${label} ${number(diagnostic[key])}`);
  const semantic = list(diagnostic.semanticChecks)
    .filter(
      (c) =>
        typeof c?.checkId === "string" &&
        /^[a-z][a-z0-9.]{0,31}$/.test(c.checkId) &&
        ["fail", "uncertain"].includes(c.verdict) &&
        typeof c.rule === "string" &&
        /^semantic_[a-z_]{1,50}$/.test(c.rule),
    )
    .slice(0, 96)
    .map(
      (c) =>
        `${esc(c.checkId)} · ${c.verdict === "fail" ? "未通过" : "未确定"} · ${esc(c.rule)}`,
    );
  if (semantic.length)
    return (
      <>
        <p className={cls("lgi-tm-note")}>
          {["语义检查：", semantic.join("；")].join("")}
        </p>
      </>
    );
  if (!rule && !counts.length) return "";
  return (
    <>
      <p className={cls("lgi-tm-note")}>
        {[
          rule
            ? `格式规则：${esc(rule)}${safePath ? ` · 字段位置：${esc(path || "根对象")}` : ""}`
            : "",
          counts.length ? `${rule ? " · " : ""}${counts.join(" · ")}` : "",
        ].join("")}
      </p>
    </>
  );
}
export function researchTaskIssue(issue: T.TaskIssue) {
  const phases: Record<string, string> = {
    extract: "提炼讨论",
    resolve: "判断归属",
    compare: "比较讨论",
  };
  const phase = Object.hasOwn(phases, issue.phase)
    ? phases[issue.phase]
    : "阶段未知";
  const status =
    issue.state === "unknown_dispatch"
      ? "派发结果未知"
      : issue.state === "failed"
        ? "处理失败"
        : "任务状态未知";
  const reason =
    typeof issue.reason === "string" && issue.reason ? issue.reason : null;
  const reasons: Record<string, string> = {
    rule_feedback_unavailable:
      "未取得与当前冻结输入及阶段一致的规则反馈，未发送纠偏请求。",
    journey_stage_invalid: "旅程主阶段不在允许范围。",
    journey_path_invalid: "旅程路径不在允许范围。",
    journey_involved_stage_count: "涉及旅程阶段数量超过上限。",
    journey_involved_stage_invalid: "涉及阶段不在允许范围。",
    journey_overlay_count: "旅程覆盖事件数量超过上限。",
    journey_overlay_invalid: "旅程覆盖事件不在允许范围。",
    journey_rationale_invalid: "旅程依据说明必须是有效非空短文本。",
    journey_evidence_missing: "已给出明确旅程阶段，但没有原文引用依据。",
    journey_evidence_invalid: "旅程引用未通过来源或原文位置校验。",
    model_input_limit: "输入超过模型配置上限，请检查输入范围与模型设置。",
    invalid_output: "模型返回内容未通过研究结果校验，历史记录未保留具体规则。",
    output_json_invalid: "模型返回内容不是有效 JSON。",
    output_schema_invalid: "模型返回字段不符合严格研究格式。",
    output_contract_mismatch: "模型返回的研究合同版本不匹配。",
    invalid_evidence: "研究引用未通过来源或原文位置校验。",
    output_validation_failed: "模型返回内容未通过研究接纳规则。",
    response_too_large: "模型流或最终文本超过受控响应大小限制。",
    invalid_response: "模型响应流不完整或格式不可用。",
    output_limit: "模型输出达到配置上限，结果未完整接纳。",
    output_text_missing: "模型没有返回可接纳的文本结果。",
    provider_response_rejected: "模型响应未通过受控传输校验。",
    provider_timeout: "模型请求超时。",
    provider_network_error: "模型请求发生网络错误。",
    provider_stream_interrupted: "模型响应流中断。",
    provider_terminal_missing: "模型响应缺少完整结束回执。",
    provider_content_filtered: "模型服务对响应执行了内容过滤。",
    unexpected_content: "模型返回了不允许的工具或非文本内容。",
    secret_echo_rejected: "模型输出触发了凭据回显保护。",
    authentication_failed: "模型连接认证失败。",
    provider_rate_limited: "模型服务限制了请求速率。",
    provider_unavailable: "模型服务暂时不可用。",
    provider_request_rejected: "模型服务拒绝了请求参数。",
    provider_endpoint_not_found: "模型连接地址不可用。",
    provider_redirect_rejected: "模型请求被不允许的重定向拒绝。",
    provider_failed: "模型服务未完成请求。",
    endpoint_rejected: "模型连接地址不符合受控连接要求。",
    invalid_request: "模型请求不符合传输合同。",
    invalid_research_phase: "研究任务阶段不符合合同。",
    invalid_output_structure: "研究输出的结构、状态或条目数量不符合合同。",
    invalid_unicode_fragment: "引用片段的 Unicode 位置不符合来源合同。",
    invalid_journey_evidence: "旅程判断的引用不符合来源或阶段规则。",
    invalid_discussion_evidence: "讨论的角色、引用或原文位置不符合规则。",
    invalid_response_evidence: "回应判断缺少合格引用或超出范围。",
    invalid_action_evidence: "角度或机会判断缺少合格引用或边界。",
    duplicate_angle_task: "模型重复返回同一角度任务。",
    duplicate_journey_stage: "模型重复返回同一旅程阶段。",
    invalid_resolution_candidates: "归属候选定义不符合本次检索范围。",
    duplicate_resolution_candidate: "归属候选重复。",
    invalid_resolution_structure: "归属输出的结构不符合合同。",
    invalid_resolution_units: "归属输出与本次讨论身份不匹配。",
    invalid_resolution_decision: "归属决定的理由或字段不符合规则。",
    invalid_resolution_match: "归属匹配不属于本次候选定义。",
    invalid_resolution_relation: "主题关系不符合候选范围或边界。",
    missing_proposed_topic: "新主题决定没有附带候选定义。",
    invalid_new_topic_boundary:
      "新主题未通过边界校验；这是历史笼统原因，未保存具体违反规则。",
    invalid_resolution_status: "归属状态不符合严格合同。",
    request_not_started: "请求未开始执行，当前任务已停止尝试。",
  };
  Object.assign(reasons, {
    semantic_review_pending: "提案等待独立语义复核，尚未接纳或计入有效覆盖。",
    semantic_review_unavailable:
      "待复核提案与原调用、来源或定义版本不一致，未发送复核请求。",
    semantic_review_uncertain: "独立复核仍无法确定，未接纳主题或计入有效覆盖。",
    semantic_review_contract_invalid: "独立复核返回的合同版本不正确。",
    semantic_review_coverage_invalid:
      "独立复核漏项、重复或返回了未请求的检查。",
    semantic_review_check_invalid: "独立复核判断或短理由不符合合同。",
    semantic_outcome_unsupported: "分析、无信号或材料不足的结论与原文不符。",
    semantic_discussion_unsupported:
      "引用未支持讨论的完整主张、表达者身份或计划与实际效果边界。",
    semantic_discussion_non_atomic: "讨论混合了可独立研究的不同问题或障碍。",
    semantic_scene_unsupported: "场景描述缺少完整原文支持。",
    semantic_journey_unsupported: "旅程阶段、路径或经历判断缺少原文支持。",
    semantic_response_unsupported: "回应匹配判断缺少双方材料支持。",
    semantic_angle_unsupported: "内容角度缺少证据依据或与其他角度重复。",
    semantic_opportunity_unsupported: "机会假设的需求或推论超过材料支持范围。",
    semantic_assignment_boundary: "归属未同时满足主题纳入条件和排除边界。",
    semantic_identity_relation: "同义主题未复用或不同障碍被混为同一主题。",
    semantic_concept_boundary:
      "概念定义拼接现象、扩大断言或把偶然属性固定成边界。",
    semantic_parent_containment:
      "子主题范围并非完全包含于父主题，相关或共现不足以建立父子关系。",
    invalid_citation_quote: "原文短引句为空、只有空白或超过长度限制。",
    citation_quote_not_found: "短引句未在指定冻结片段中逐字找到。",
    citation_quote_ambiguous: "短引句在指定冻结片段中出现多次，无法唯一定位。",
    invalid_citation_fragment_index: "引用片段编号不在当前冻结输入范围。",
    citation_missing: "缺少必需的原文引用。",
    citation_count_exceeded: "单项引用数量超过上限。",
    citation_fragment_not_found: "引用片段不属于冻结材料。",
    citation_range_invalid: "引用起止位置不在冻结片段有效范围。",
    citation_length_exceeded: "单条引用长度超过上限。",
    discussion_label_invalid: "讨论名称为空或超过长度限制。",
    discussion_topic_not_allowed: "提炼阶段不能自行指定主题归属。",
    discussion_statement_invalid: "讨论陈述为空或超过长度限制。",
    discussion_definition_invalid: "讨论定义为空或超过长度限制。",
    discussion_inclusion_invalid: "讨论纳入条件缺失、为空或超过数量长度限制。",
    discussion_exclusion_invalid: "讨论排除条件缺失、为空或超过数量长度限制。",
    discussion_criteria_duplicate: "讨论纳入和排除条件存在重复。",
    discussion_rationale_invalid: "讨论依据为空或超过长度限制。",
    discussion_speaker_role_invalid: "表达者角色不在允许范围。",
    discussion_evidence_role_invalid: "证据角色不在允许范围。",
    discussion_source_role_mismatch: "表达者身份或证据角色与引用来源不符。",
    scene_label_invalid: "场景名称为空或超过长度限制。",
    response_status_invalid:
      "回应程度不在允许的直接回应、部分回应、未回应、不适用或未知范围。",
    response_unanswered_count_exceeded: "尚未回应的问题数量超过上限。",
    response_unanswered_text_invalid: "尚未回应的问题为空或超过长度限制。",
    response_author_evidence_missing:
      "回应判断缺少合格的作者原文，作品上下文不能冒充作者回应。",
    response_commenter_evidence_missing:
      "回应判断缺少合格的主评论原文，父评论不能冒充当前评论者经历。",
    angle_label_invalid: "内容角度名称为空或超过长度限制。",
    angle_title_invalid: "内容角度标题为空或超过长度限制。",
    angle_answer_task_invalid: "内容角度回答任务为空或超过长度限制。",
    opportunity_need_invalid: "机会所对应的需求为空或超过长度限制。",
    opportunity_hypothesis_invalid: "机会假设为空或超过长度限制。",
    opportunity_verification_question_invalid:
      "机会验证问题为空或超过长度限制。",
    opportunity_alternative_explanation_invalid:
      "机会替代解释为空或超过长度限制。",

    resolution_candidates_not_array: "归属候选必须是数组。",
    resolution_candidate_count_exceeded: "本次归属候选超过 64 个。",
    resolution_candidate_topic_invalid: "候选缺少有效主题身份。",
    resolution_candidate_definition_invalid: "候选缺少有效定义版本。",
    resolution_contract_mismatch: "归属输出合同版本不匹配。",
    resolution_unit_coverage_mismatch: "归属决定未完整覆盖本次讨论。",
    resolution_unit_count_exceeded: "归属讨论数量超过合同上限。",
    resolution_unit_identity_duplicate: "请求内讨论身份重复。",
    resolution_unit_identity_invalid: "请求内讨论身份为空或过长。",
    resolution_unit_not_requested: "模型返回了本次未请求的讨论。",
    resolution_unit_decision_duplicate: "同一讨论被重复判断。",
    resolution_decision_reason_invalid: "归属理由为空或超过 500 字符。",
    resolution_match_count_exceeded: "单条讨论的匹配超过 4 项。",
    resolution_relation_count_exceeded: "主题关系超过 64 项。",
    resolution_match_not_candidate: "匹配主题不在本次候选中。",
    resolution_match_version_mismatch: "匹配的定义版本与本次候选不一致。",
    resolution_match_duplicate: "匹配主题重复。",
    resolution_match_reason_invalid: "匹配理由为空或超过 500 字符。",
    resolution_relation_not_candidate: "关系主题不在本次候选中。",
    resolution_relation_version_mismatch: "关系的定义版本与本次候选不一致。",
    resolution_relation_kind_invalid: "主题关系类型不属于允许的六种关系。",
    resolution_relation_reason_invalid: "主题关系理由为空或超过 500 字符。",
    resolution_relation_duplicate: "同一候选出现重复关系。",
    resolution_match_relation_conflict: "已匹配主题同时被判断为不同或不确定。",
    resolution_matched_without_match: "归属状态为已匹配，但没有匹配主题。",
    resolution_matched_has_proposal: "已匹配决定不能同时提出新主题。",
    resolution_new_has_match: "新主题决定不能同时含有匹配主题。",
    resolution_unassigned_has_match: "未确定或领域外决定不能同时匹配主题。",
    resolution_unassigned_has_proposal:
      "未确定或领域外决定不能同时提出新主题。",
    resolution_status_unknown: "归属状态不属于严格合同。",
    new_topic_candidate_coverage_missing: "新主题未逐一比较全部召回候选。",
    new_topic_equivalent_requires_match:
      "判断为同义主题时，必须明确匹配已有主题。",
    new_topic_definition_copies_statement:
      "新主题定义直接复述了一条具体讨论，缺少可复用边界。",
    new_topic_label_invalid: "新主题名称为空或超过 120 字符。",
    new_topic_definition_invalid: "新主题定义为空或超过 1,000 字符。",
    new_topic_domain_not_in_scope: "新主题没有被明确判断为当前领域内。",
    new_topic_domain_reason_invalid: "新主题领域依据为空或超过 500 字符。",
    new_topic_abstraction_reason_invalid: "新主题抽象依据为空或超过 500 字符。",
    parent_topic_label_invalid: "父主题名称为空或超过 120 字符。",
    parent_topic_definition_invalid: "父主题定义为空或超过 1,000 字符。",
    parent_topic_domain_not_in_scope: "父主题没有被明确判断为当前领域内。",
    parent_topic_domain_reason_invalid: "父主题领域依据为空或超过 500 字符。",
    parent_topic_abstraction_reason_invalid:
      "父主题抽象依据为空或超过 500 字符。",
    new_topic_criteria_duplicate: "新主题纳入或排除条件重复，包含跨组重复。",
    parent_topic_criteria_duplicate: "父主题纳入或排除条件重复，包含跨组重复。",
    topic_parent_reason_invalid: "父子关系依据为空或超过 500 字符。",
    topic_parent_root_has_reference: "根主题不能同时引用或提出父主题。",
    topic_parent_existing_reference_missing:
      "现有父主题缺少主题身份或定义版本。",
    topic_parent_existing_has_proposal: "现有父主题不能同时附带新父主题提案。",
    topic_parent_not_candidate: "父主题不在本次候选中。",
    topic_parent_version_mismatch: "父主题定义版本与本次候选不一致。",
    topic_parent_containment_missing:
      "现有父主题缺少明确的范围包含关系，相关或共现不能作为父子关系。",
    topic_parent_proposed_has_reference: "新父主题提案不能同时引用现有父主题。",
    topic_parent_proposal_missing: "提出父主题时没有附带完整父主题定义。",
    topic_parent_same_label: "父主题与子主题名称相同。",
    topic_parent_same_concept: "父主题与子主题定义边界归一化后相同。",
    topic_parent_proposal_matches_candidate:
      "父主题提案与已有候选边界相同，必须明确引用现有父主题版本并判断范围包含。",
    topic_parent_kind_unknown: "父主题类型必须是根、现有或新提案。",
    new_topic_inclusion_empty: "新主题缺少纳入条件。",
    new_topic_inclusion_count_exceeded: "新主题纳入条件超过 8 项。",
    new_topic_inclusion_item_empty: "新主题存在空白纳入条件。",
    new_topic_inclusion_item_too_long: "新主题单条纳入条件超过 300 字符。",
    new_topic_exclusion_empty: "新主题缺少排除条件。",
    new_topic_exclusion_count_exceeded: "新主题排除条件超过 8 项。",
    new_topic_exclusion_item_empty: "新主题存在空白排除条件。",
    new_topic_exclusion_item_too_long: "新主题单条排除条件超过 300 字符。",
    parent_topic_inclusion_empty: "父主题缺少纳入条件。",
    parent_topic_inclusion_count_exceeded: "父主题纳入条件超过 8 项。",
    parent_topic_inclusion_item_empty: "父主题存在空白纳入条件。",
    parent_topic_inclusion_item_too_long: "父主题单条纳入条件超过 300 字符。",
    parent_topic_exclusion_empty: "父主题缺少排除条件。",
    parent_topic_exclusion_count_exceeded: "父主题排除条件超过 8 项。",
    parent_topic_exclusion_item_empty: "父主题存在空白排除条件。",
    parent_topic_exclusion_item_too_long: "父主题单条排除条件超过 300 字符。",
  });
  const explanation =
    issue.state === "unknown_dispatch"
      ? "无法确认请求的执行结果，不会自动重发。"
      : reason && Object.hasOwn(reasons, reason)
        ? reasons[reason]
        : reason
          ? "原因尚未识别，请核对任务记录。"
          : "未记录具体原因，请核对任务记录。";
  return (
    <>
      <li>
        <strong>{[esc(phase), " · ", esc(status)].join("")}</strong>
        <p>
          {esc(explanation)}
          {reason ? (
            <>
              {" "}
              <span className={cls("lgi-tm-num")}>
                {["原因码：", esc(reason)].join("")}
              </span>
            </>
          ) : (
            ""
          )}
        </p>
        {validationDetails(issue.validationDiagnostic)}
      </li>
    </>
  );
}
export function researchRunDetails(run: T.Run) {
  const issues = list(run.taskIssues).slice(0, 20);
  if (issues.length)
    return (
      <>
        <div className={cls("lgi-tm-note")}>
          <p>
            {[
              "失败与派发未知任务 · 显示 ",
              number(issues.length),
              " / ",
              number(researchIssueCount(run)),
              " 项",
            ].join("")}
          </p>
          <ul>{issues.map(researchTaskIssue)}</ul>
        </div>
      </>
    );
  if ((researchIssueCount(run) ?? 0) > 0)
    return (
      <>
        <p className={cls("lgi-tm-note")}>
          {"存在失败或派发未知任务，具体原因尚未取得，请刷新进度。"}
        </p>
      </>
    );
  if (run.lastReason === "comparison_queued") return "";
  const reasons: Record<string, string> = {
    daily_budget_paused: "日额度不足，等待额度恢复。",
    run_budget_exhausted: "本次研究额度已用尽。",
    paused: "研究已暂停。",
    stopped: "研究已明确停止。",
    method_superseded: "旧方法已被替代，保留历史账本并停止未派发任务。",
  };
  return run.lastReason ? (
    <>
      <p>
        {esc(
          Object.hasOwn(reasons, run.lastReason)
            ? reasons[run.lastReason]
            : `最近运行回执：${run.lastReason}`,
        )}
      </p>
    </>
  ) : (
    ""
  );
}

export function researchIssueCount(run: T.Run) {
  return known(run.failedCount) && known(run.unknownDispatchCount)
    ? run.failedCount + run.unknownDispatchCount
    : undefined;
}
export function researchCounts(counts: T.Counts | null | undefined) {
  return (
    <>
      <p>
        {[
          "待处理 ",
          number(counts?.queuedCount),
          " · 进行中 ",
          number(counts?.runningCount),
          " · 已接纳结果 ",
          number(counts?.succeededCount),
        ].join("")}
      </p>
      <p>
        {[
          "无研究信号 ",
          number(counts?.noSignalCount),
          " · 材料不足 ",
          number(counts?.insufficientCount),
          " · 失败 ",
          number(counts?.failedCount),
          " · 派发未知 ",
          number(counts?.unknownDispatchCount),
        ].join("")}
      </p>
      <p>
        {[
          "初次材料已提炼窗口 ",
          number(counts?.extractedWindowCount),
          " · 已过时 ",
          number(counts?.staleCount),
          " · 已停止 ",
          number(counts?.stoppedCount),
        ].join("")}
      </p>
    </>
  );
}
export function researchCoverageCounts(counts: T.Counts | null | undefined) {
  return (
    <>
      <div className={cls("lgi-tm-callout")}>
        <p>
          {[
            "初次材料处理：待处理 ",
            number(counts?.initialQueuedCount),
            " 个窗口 · 已处理 ",
            number(counts?.completedInitialWindowCount),
            " 个独立窗口 · 涉及 ",
            number(counts?.coveredWorkCount),
            " 篇独立作品（至少处理一窗）",
          ].join("")}
        </p>
        <p>
          {[
            "定义重评：待处理 ",
            number(counts?.reassessmentQueuedCount),
            " · 已完成 ",
            number(counts?.reassessmentCompletedCount),
            " 次",
          ].join("")}
        </p>
        <p className={cls("lgi-tm-note")}>
          {
            "窗口按方法、配置及输入身份去重，历史方法或重新分组可能重叠；处理过一窗不代表全文完成或人数。"
          }
        </p>
      </div>
    </>
  );
}

export function researchRun(run: T.Run) {
  const state = `旧记录：${run.state === "daily_budget_paused" ? "当时因日额度暂停" : run.state === "run_budget_exhausted" ? "当时研究额度已用尽" : labelState(run.state)}`;
  const trigger =
    Object.assign({
      historical: "历史回填",
      incremental: "新材料增量",
      on_demand: "按需研究",
    } as Record<string, string>)[run.trigger || ""] || run.trigger;
  return (
    <>
      <article className={cls("lgi-tm-run")}>
        <div>
          <h4>
            {esc(state)} {badge(trigger)}
          </h4>
          <p>{["方法版本：", esc(run.methodVersion || "未记录")].join("")}</p>
          {researchCoverageCounts(run)}
          {researchCounts(run)}
          {researchPhases(run)}
          {researchRunDetails(run)}
          <p>{["创建于 ", esc(run.createdAt)].join("")}</p>
          <small className={cls("lgi-tm-num")}>{esc(run.runRef)}</small>
        </div>

      </article>
    </>
  );
}
export function currentResearchSummary(
  p: T.ResearchProgress | null | undefined,
) {
  const counts = p?.currentMethodSummary,
    limits = p?.processingLimits;
  const missing = (
    <>
      <p className={cls("lgi-tm-note")}>
        {"当前方法进展尚未读取，请刷新；历史账本不作为当前进展。"}
      </p>
    </>
  );
  return (
    <>
      <section className={cls("lgi-tm-section")}>
        <h3>{"当前方法有效进展"}</h3>
        <p>{["方法版本：", esc(p?.methodVersion || "未读取")].join("")}</p>
        {counts ? (
          <>
            {researchCoverageCounts(counts)}
            {researchCounts(counts)}
            <p>
              {[
                "等待规则纠偏 ",
                number(counts.correctiveQueuedCount),
                " · 纠偏调用中 ",
                number(counts.correctiveRunningCount),
              ].join("")}
            </p>
            <p>
              {[
                "等待语义复核 ",
                number(counts.reviewQueuedCount),
                " · 语义复核中 ",
                number(counts.reviewRunningCount),
              ].join("")}
            </p>
            {researchPhases({ phases: counts.phases })}
          </>
        ) : (
          missing
        )}
        <p className={cls("lgi-tm-note")}>
          {
            "有效窗口处理只计已接纳、无研究信号或材料不足；失败、派发未知、旧方法退役均不计入。当前方法须完成引用与归属的独立语义复核后接纳；机器复核通过仍是候选，旧方法结果不自动取得新质量标识。"
          }
        </p>
        {limits ? (
          <>
            <p>
              {[
                "自动研究每批最多 ",
                number(limits.automaticBatchWindows),
                " 个窗口 · 完整单篇笔记 · 每窗最多 ",
                number(limits.primaryCommentsPerWindow),
                " 条主要评论",
              ].join("")}
            </p>
            <p className={cls("lgi-tm-note")}>
              {
                "批次是逐窗派发的排队范围，不是一次发送全部材料；当前队列不代表全部尚未研究材料。"
              }
            </p>
          </>
        ) : (
          ""
        )}
      </section>
    </>
  );
}
export function researchSummary(p: T.ResearchProgress | null | undefined) {
  return (
    <>
      <section className={cls("lgi-tm-section")}>
        <h3>{"全领域任务账本"}</h3>
        <p>
          {[
            "累计 ",
            number(p?.summary?.totalRunCount),
            " 个批次 · ",
            number(p?.summary?.totalTaskCount),
            " 个任务",
          ].join("")}
        </p>
        {researchCoverageCounts(p?.summary)}
        {researchCounts(p?.summary)}
        {researchPhases({ phases: p?.summary?.phases })}
        <p className={cls("lgi-tm-note")}>
          {
            "包含全部历史方法与已停止任务，不据累计分母计算当前覆盖率。初次窗口按方法、模型配置与输入身份去重，历史方法或重新分组可能覆盖同一原文；无信号、材料不足表示窗口已处理，不表示形成主题。定义重评复用已有讨论，不增加初次窗口；实际字符覆盖请核对作品阅读器。任务接纳、模型调用、讨论数量和主题质量分别判断。历史账本不代表当前可用主题或研究质量。"
          }
        </p>
      </section>
    </>
  );
}
