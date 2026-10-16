import { ApiError, isRecord, requestJson } from "../../shared/api";

const friendlyNames: Record<string, string> = {
  topic_map_not_connected: "图谱数据库尚未连接",
  topic_map_schema_not_ready: "图谱数据结构尚未就绪",
  research_not_configured: "尚未保存研究模型与预算设置",
  collection_not_enabled: "尚未明确开启本领域的有界补采",
  collection_admission_rejected: "现有采集授权或执行范围未满足准入条件",
  material_not_found: "材料不在当前领域可读范围内",
  move_children_before_structural_replacement:
    "旧主题仍有子主题，请先调整子主题归属再替换结构",
  invalid_structure_cardinality: "合并与拆分的主题数量不符合本次调整方式",
  invalid_structure_destination: "每个新方向需要名称、定义和显式分配的实际作品",
  invalid_structure_member: "作品分配需要独立身份与具体理由",
  structure_source_restricted: "部分来源材料当前不可用，不能重新分配",
  request_identity_conflict: "本次请求身份与已保存内容冲突，请刷新后重新检查",
  topic_map_conflict: "定义或规则版本已变化，请刷新当前版本",
  model_configuration_missing: "所选模型配置尚未取得",
  research_not_found: "该研究运行尚未读取",
  domain_required: "先选择明确研究领域",
  invalid_research_budget: "研究预算须填写有效的明确上限",
};
export async function topicRequest<V>(
  path: string,
  parse: (value: unknown) => V,
  options: { method?: "POST"; body?: string } | undefined,
  signal: AbortSignal,
) {
  try {
    return parse(
      await requestJson(path, {
        method: options?.method || "GET",
        ...(options?.body ? { body: JSON.parse(options.body) as unknown } : {}),
        signal,
      }),
    );
  } catch (error) {
    if (error instanceof ApiError && error.failure.kind === "http") {
      const body = isRecord(error.responseBody) ? error.responseBody : {};
      const code =
        [body.message, body.detail, body.reason, body.code, body.error].find(
          (v): v is string => typeof v === "string",
        ) || error.failure.code;
      const message =
        friendlyNames[code || ""] ||
        (code
          ? `服务未完成此操作：${code}`
          : `读取未成功（${error.failure.status}）`);
      throw Object.assign(error, {
        message,
        status: error.failure.status,
        code,
      });
    }
    throw error;
  }
}
