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
    core_fields["summary"]["properties"]["text"]["description"] = json!(
        "用中立语言概括整篇用途，并引用支持摘要的知识单元。书名、歌名等确切作品名只有在所列单元引用的证据逐字包含时才写作《…》。主题、活动和转述用普通陈述，不标为作品名或原话。缺少证据时概括已支持的用途。只由发布者声称的产品范围、效果、期限、售后条款等保留归因。画面样例只能支持其中能看到的内容。"
    );
    core_fields["contentType"]["description"] = json!(
        "按整篇的主要发布目的选择已有类型。作者在等读者给答案或推荐时选 question。标题是问句但作者已回答时，按回答的内容归类。帖内教读者完成一件事，可跟做的操作、话术或示范为主时选 practical_guide。并列项目供查用、没有必须遵循的操作顺序时选 checklist。讲真人亲历和事件经过为主选 personal_story，其中提到的做法可以进标签。分析具体案例用 case_analysis，解释现象用 explanation，表达取舍或倡议用 opinion。主要在推介产品、服务或报名时用 product_intro，参与挑战本身为主用 interactive_challenge。只有两种目的都独立且难分主次才用 mixed。"
    );
    core_fields["mainTopicUnitId"]["description"] = json!(
        "必填。先为整篇的主要话题或发布目的写一条有依据的知识单元，其 unitType 必须是 discussion。mainTopicUnitId 填这条单元的 id。输出前按 id 找回该单元，确认 unitType 是 discussion。"
    );
    core_fields["knowledgeUnits"]["description"] = json!(format!(
        "按内容作用提炼单元：audience_context最多{}条、author_position最多{}条、provision最多{}条、reader_value最多{}条。相关内容可合并表述，保留关键方法、数字、否定和条件，不为分类凑单元。仅由作者声称的内容使用 author_claim，并在 statement 中注明归因。",
        core["audienceUnitIds"]["maxItems"],
        core["authorPositionUnitIds"]["maxItems"],
        core["provisionUnitIds"]["maxItems"],
        core["readerValueUnitIds"]["maxItems"],
    ));
    core_fields["knowledgeUnits"]["items"]["properties"]["unitType"]["description"] = json!(
        "仅使用本字段的 enum。整篇在讲什么或作者发布的目的用 discussion，例如作者推介一套学习资料。具体提供的方法、解释、信息和案例用 provision，例如资料附练习答案。作者主张不等于作者立场。明确立场用 author_position，受众与处境用 audience_context。assertionKind 独立填写。整篇主议题由作者主张时，unitType 填 discussion，assertionKind 填 author_claim，在陈述中注明归因。"
    );
    core_fields["knowledgeUnits"]["items"]["properties"]["statement"]["description"] = json!(
        "让这条知识单元单独阅读也能理解。说清主体、所指对象与做法，保留原文有实质影响的归属、语气、个人经验、引述、否定和限定。作者在猜测、推荐、描述亲历或转述他人时，在陈述中保留这种表达。无需判断作者说得是否严谨，不补写原帖没有的限定、归因或结论。"
    );
    core_fields["knowledgeUnits"]["items"]["properties"]["evidenceRefs"]["description"] = json!(
        "这里仅填证据条目 ID（例如 ev1），不能填材料编号（例如 s1、image_1）。引用直接支撑本单元的细节。书名用《…》。逐字引语标为原话「…」或原文「…」，口播原话「…」引用口播。图片或画面中识读的引句用图片原文「…」。主题与术语可用普通引号。书名及逐字引语应逐字出现在本单元引用的原文或逐图识读引句中。同一细节重复出现时选一个能支撑它的片段。口播细节引用 transcript；整体标题或画面标题不能单独支撑更具体的内容。每个关键数字、否定、条件和先后顺序都要由所列依据支持。"
    );
    core_fields["candidateLabels"]["description"] = json!(
        "每条候选标签必须同时填写 dimension、label、retrievalUse、unitIds。dimension 只能从本字段的 enum 选择；author_position 是知识单元类型，不能作为标签维度。没有合适维度时省略该候选标签，知识单元仍保留。unitIds 关联 knowledgeUnits 中实际存在的单元 ID，不能遗漏或为空。同一项资源的内容、呈现形式和获取方式对应不同检索用途时分别保留。仅在资源、维度、检索用途都相同时合并重复标签，保留双方能支撑该用途的单元，超出引用上限时分别保留。示例仅演示结构，使用本篇实际标签与 ID。"
    );
    core_fields["candidateLabels"]["items"]["properties"]["unitIds"]["description"] = json!(
        "必填，列出能支撑这条标签的已有知识单元。涉及资源时，核对标签、检索用途和单元证据说的是同一项资源。同帖提到多个资源时，不能借另一项资源的证据。资源名称、内容和领取提示在不同片段时，仅在原文明确把它们连在一起时，引用各自对应的知识单元。同一单元确实支撑两个用途时可以共用；不要把分开的引句拼成一条证据。"
    );
    core_fields["candidateLabels"]["items"]["properties"]["retrievalUse"]["description"] = json!(
        "说明读者在本帖能找到什么。分别写清帖内可见的内容、只展示的示例或目录、只提到的资源和需要另行获取的资源。原帖明确说明评论、私信、报名或购买时，将获取方式写在 participation 标签，并保留资源实际名称。没有入口时只写提及。帖内给出的方法仍可写为 offered_value，不把外部完整资料混入帖内供给。"
    );
    core_fields["candidateLabels"]["items"]["examples"] = json!([{
        "dimension":"offered_value","label":"分步练习","retrievalUse":"查找分步练习方法","unitIds":["u3"]
    }]);
    let text = &card["evidence"]["items"]["oneOf"][0]["properties"];
    let mut text_fields = fields(text, &["id", "inputCitationId", "quote"]);
    text_fields["quote"]["description"] = json!(
        "从对应 inputCitationId 的 text 中逐字复制一段连续且唯一出现的原文。保留大小写、空格、换行和标点，不改写、拼接或补字。口播转写中人名分写也按原文复制，例如「Li Lei」不能合写为「LiLei」。原文截断或引号不成对时，可选足以支持结论的内部连续短语。输出前核对 quote 在该 text 中逐字匹配且只出现一次。概括写在 statement。本次直接从图片识读的文字使用 vision。"
    );
    text_fields["inputCitationId"]["pattern"] = json!("^s[1-9][0-9]*$");
    text_fields["inputCitationId"]["description"] =
        json!("填写 material.texts 中实际出现的片段编号（例如 s3）。");
    text_fields["id"]["pattern"] = json!("^ev[A-Za-z0-9_-]*$");
    text_fields["quote"]["examples"] = json!(["分步练习"]);
    let vision = &card["evidence"]["items"]["oneOf"][1]["properties"];
    let unit_id = core["knowledgeUnits"]["items"]["properties"]["id"].clone();
    let evidence_id = text_fields["id"].clone();
    core_fields["knowledgeUnits"]["items"]["properties"]["evidenceRefs"]["items"] =
        evidence_id.clone();
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
    vision_fields["id"] = evidence_id.clone();
    vision_fields["speakerRole"]["type"] = json!("string");
    let mut contribution = object(json!({
        "inputCitationId":vision["inputCitationId"],
        "status":{"type":"string","enum":["used","no_new_information","unclear"]},
        "summary":{"type":"string","minLength":1,"maxLength":500,"pattern":"\\S"},
        "unitIds":array(unit_id,0,12),"evidenceRefs":array(evidence_id,0,8)
    }));
    contribution["properties"]["evidenceRefs"]["description"] = json!(
        "只填 evidenceCandidates.vision 中 inputCitationId 与本图片相同的证据条目 ID（例如 ev2）；不引用 text 依据或其他图片。关联整篇知识用 unitIds，不把该单元的全部依据复制到此处。"
    );
    let mut schema = object(json!({
        "core":object(core_fields),
        "imageContributions":{"type":"array","items":contribution},
        "imageReadings":{"type":"array","items":object(json!({
            "inputCitationId":vision["inputCitationId"],
            "state":{"type":"string","enum":["readable","no_text","unreadable","deferred"]},
            "text":{"type":"string","maxLength":30000,"description":"按原图阅读顺序整理本图所有可辨认的实质文字，连起排版断行，保留标题、段落、清单和表格。只记录本图可辨认文字，不根据上下文补字、改字或推断看不清的内容；看不清的字标［无法辨认］。跨页句子只记录本图可见部分。保留作者观点、数字和限定条件，不写摘要，不添加新观点。readable 填整理后的 Markdown；no_text 表示确实没有文字，unreadable 表示有字但无法读取，deferred 表示本次输出预算不足以完整记录该图；这些状态的 text 均为空。"}
        })),"description":"material.images 中的每张图片正好一条。无图时返回空数组。"} ,
        "evidenceCandidates":object(json!({
            "text":array(object(text_fields),0,64),
            "vision":array(object(vision_fields),0,64)
        }))
    }));
    schema["description"] = json!(
        "仅返回一个完整的 JSON 对象。参考 examples 中的完整结构，内容和编号以本次材料为准。顶层只有 core、imageContributions、imageReadings、evidenceCandidates，证据候选与 core 同级。各层不添加额外字段或说明。字符串正确转义，括号成对闭合，对象结束后不再追加内容。"
    );
    schema["examples"] = json!([{
        "core":{
            "summary":{"text":"作者分享核对材料顺序的方法。","unitIds":["u1"]},
            "contentType":"practical_guide",
            "mainTopicUnitId":"u1",
            "knowledgeUnits":[{"id":"u1","unitType":"discussion","assertionKind":"author_claim",
                "statement":"作者建议先核对顺序，再摆出材料。","speakerRole":"author","evidenceRefs":["ev1"]}],
            "candidateLabels":[]
        },
        "imageContributions":[],
        "imageReadings":[],
        "evidenceCandidates":{"text":[{"id":"ev1","inputCitationId":"s1",
            "quote":"先核对顺序，再摆出材料"}],"vision":[]}
    }]);
    schema
}

/// Reserve most of the model output for the core and reasoning; do not emit a
/// truncated image reading as though it were complete. This is an instruction
/// and schema bound, not a guarantee that a provider respects its token cap.
pub(crate) fn response_schema_for_images(image_count: usize, output_tokens: usize) -> Value {
    let mut schema = response_schema();
    if image_count > 0 {
        let max_chars = (output_tokens / 3 / image_count).min(3_000);
        let text = &mut schema["properties"]["imageReadings"]["items"]["properties"]["text"];
        text["maxLength"] = json!(max_chars);
        let original = text["description"].as_str().unwrap_or("").to_owned();
        text["description"] = json!(format!(
            "{original} 本次每图最多 {max_chars} 字；若无法完整记录整图，不得截断冒充完整，请用 deferred 且 text 为空。"
        ));
        schema["description"] = json!(
            "先完成 core、evidenceCandidates 和 imageContributions，最后填写 imageReadings；每张图片遵守本次字数上限，放不下时标 deferred，必须结束为完整 JSON。"
        );
    }
    schema
}

#[cfg(test)]
mod image_budget_tests {
    use super::*;
    #[test]
    fn multi_image_readings_leave_output_room_and_can_be_deferred() {
        for image_count in [1, 4, 13] {
            let schema = response_schema_for_images(image_count, 40_000);
            let text = &schema["properties"]["imageReadings"]["items"]["properties"]["text"];
            let max_chars = text["maxLength"].as_u64().unwrap() as usize;
            assert!(max_chars * image_count <= 40_000 / 3);
            assert!(text["description"].as_str().unwrap().contains("deferred"));
            assert!(
                schema["properties"]["imageReadings"]["items"]["properties"]["state"]["enum"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("deferred"))
            );
        }
    }
}
