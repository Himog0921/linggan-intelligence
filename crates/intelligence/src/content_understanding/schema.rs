use serde_json::{Map, Value, json};

fn object(properties: Value) -> Value {
    let required: Vec<_> = properties
        .as_object()
        .expect("static schema properties")
        .keys()
        .cloned()
        .collect();
    json!({"type":"object","additionalProperties":false,"required":required,"properties":properties})
}
fn fields(properties: &Value, names: &[&str]) -> Value {
    Value::Object(
        names
            .iter()
            .map(|name| ((*name).to_owned(), properties[*name].clone()))
            .collect::<Map<_, _>>(),
    )
}
fn array(items: Value, min: usize, max: usize) -> Value {
    json!({"type":"array","items":items,"minItems":min,"maxItems":max,"uniqueItems":true})
}

/// Model draft only: identity, exact offsets, hashes and category projections are program-owned.
/// Public storage-contract field definitions supply the shared types, enums and bounds.
/// Cross-reference/source checks and the combined 64-candidate ceiling run in `accept`.
pub fn response_schema() -> Value {
    let storage: Value = serde_json::from_str(include_str!(
        "../../../../docs/data-contracts/content-card.v0.2.schema.json"
    ))
    .expect("checked-in Content Card schema");
    let card = &storage["properties"];
    let core = &card["core"]["properties"];
    let mut core_fields = fields(
        core,
        &[
            "summary",
            "contentType",
            "mainTopicUnitId",
            "knowledgeUnits",
            "candidateLabels",
        ],
    );
    core_fields["knowledgeUnits"]["description"] = json!(format!(
        "按内容作用提炼单元：audience_context最多{}条、author_position最多{}条、provision最多{}条、reader_value最多{}条。相关内容可合并表述，保留关键方法、数字、否定和条件，不为分类凑单元。",
        core["audienceUnitIds"]["maxItems"],
        core["authorPositionUnitIds"]["maxItems"],
        core["provisionUnitIds"]["maxItems"],
        core["readerValueUnitIds"]["maxItems"],
    ));
    core_fields["knowledgeUnits"]["items"]["properties"]["unitType"]["description"] = json!(
        "仅使用本字段的 enum。作者主张不等于作者立场：方法、解释、信息和案例属于 provision，明确立场用 author_position；受众与处境用 audience_context。author_claim、observation、interpretation 属于 assertionKind，不是 unitType。"
    );
    core_fields["candidateLabels"]["description"] = json!(
        "每条候选标签必须同时填写 dimension、label、retrievalUse、unitIds；unitIds 关联 knowledgeUnits 中实际存在的单元 ID，不能遗漏或为空。示例仅演示完整结构，使用本篇实际标签与 ID，不照抄示例内容。"
    );
    core_fields["candidateLabels"]["items"]["properties"]["unitIds"]["description"] = json!(
        "必填，列出支持本标签的已有知识单元 ID；retrievalUse 说明用途，不能代替 unitIds。输出前逐条检查所有标签的关联。"
    );
    core_fields["candidateLabels"]["items"]["examples"] = json!([{
        "dimension":"offered_value","label":"分步练习","retrievalUse":"查找分步练习方法","unitIds":["u3"]
    }]);
    let text = &card["evidence"]["items"]["oneOf"][0]["properties"];
    let mut text_fields = fields(text, &["id", "inputCitationId", "quote"]);
    text_fields["quote"]["description"] = json!(
        "从对应 inputCitationId 的 text 中逐字复制一段连续且唯一出现的原文；保留原字词和标点，不改写、拼接或补全缺失标点。若原文截断或引号不成对，可选内部连续短语，不必引用整个标题。例如合成来源 text 为「合成标题：“分步练习」，可引用「分步练习」，不能引用「合成标题：“分步练习”」，因为后者多了原文没有的右引号。输出前核对 quote 确实是该 text 的原样子串。概括写在 statement，不写进 quote；图片文字使用 vision。"
    );
    text_fields["quote"]["examples"] = json!(["分步练习"]);
    let vision = &card["evidence"]["items"]["oneOf"][1]["properties"];
    let id = text["id"].clone();
    let citation = text["inputCitationId"].clone();
    let mut vision_fields = fields(
        vision,
        &[
            "id",
            "inputCitationId",
            "modelQuote",
            "observation",
            "speakerRole",
        ],
    );
    vision_fields["speakerRole"]["type"] = json!("string");
    let mut contribution = object(json!({
        "inputCitationId":citation,
        "status":{"type":"string","enum":["used","no_new_information","unclear"]},
        "summary":{"type":"string","minLength":1,"maxLength":500,"pattern":"\\S"},
        "unitIds":array(id.clone(),0,12),"evidenceRefs":array(id,0,8)
    }));
    contribution["properties"]["evidenceRefs"]["description"] = json!(
        "只引用 evidenceCandidates.vision 中 inputCitationId 与本图片相同的依据 ID；不引用 text 依据或其他图片。关联整篇知识用 unitIds，不把该单元的全部依据复制到此处。"
    );
    let mut schema = object(json!({
        "core":object(core_fields),
        "imageContributions":{"type":"array","items":contribution},
        "evidenceCandidates":object(json!({
            "text":array(object(text_fields),0,64),
            "vision":array(object(vision_fields),0,64)
        }))
    }));
    schema["description"] = json!(
        "仅返回一个完整的 JSON 对象，顶层只有 core、imageContributions、evidenceCandidates，所有层级仅填写列出的字段，不添加额外字段或说明文字。字符串正确转义，括号成对闭合，对象结束后不再追加内容。"
    );
    schema
}
