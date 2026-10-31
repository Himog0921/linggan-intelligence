use super::*;
use crate::research_dossier::{VERSION, markdown, ordered_refs, refresh_revision, segment};
use uuid::Uuid;

fn dossier(materials: Vec<Value>) -> ResearchDossier {
    let mut value = ResearchDossier {
        contract: VERSION,
        work_ref: Uuid::from_u128(17),
        revision: String::new(),
        material_revision: String::new(),
        as_of: "2026-10-27T00:00:00Z".into(),
        note: json!({"platformTitle":"把一项任务写清楚"}),
        materials,
        reading_view: Value::Null,
        coverage: json!({"imageInventory":{"unknownOrder":false}}),
        attachments: json!({}),
        selection: json!({"mode":"full"}),
        fragments: vec![],
        media: vec![],
        media_inventory: vec![],
        candidate_statuses: vec![],
        metadata: json!({}),
    };
    super::super::rebuild(&mut value);
    refresh_revision(&mut value);
    value
}

fn paragraph(reference: &str, value: &str) -> Value {
    json!({"ref":reference,"kind":"paragraph","text":value,"speaker":"author",
        "bbox":[0.1,0.05,0.8,0.9]})
}

fn image(page: u64, blocks: Vec<Value>) -> Value {
    json!({"ref":format!("image-{page}"),"kind":"image","page":page,
        "purpose":"body_image","sourceState":"present","issues":[],"blocks":blocks})
}

fn groups(value: &ResearchDossier) -> Vec<&Value> {
    value.reading_view["sections"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|s| s["groups"].as_array().unwrap())
        .collect()
}

fn group_text(value: &ResearchDossier, group: &Value) -> String {
    let refs = group["blockRefs"].as_array().unwrap();
    let joiners = group["joiners"].as_array().unwrap();
    let mut text = String::new();
    for (index, reference) in refs.iter().enumerate() {
        if index > 0 {
            text.push_str(joiners[index - 1].as_str().unwrap());
        }
        let block = value
            .materials
            .iter()
            .flat_map(|m| m["blocks"].as_array().unwrap())
            .find(|b| b["ref"] == *reference)
            .unwrap();
        text.push_str(block["text"].as_str().unwrap());
    }
    text
}

#[test]
fn twelve_and_thirteen_images_preserve_every_source_and_fixed_sections() {
    for count in [12, 13] {
        let mut materials = vec![json!({"ref":"audio","kind":"audio","slotKey":"v","blocks":[
            {"ref":"a","kind":"paragraph","text":"完整口播","speaker":"author","time":{"startMs":0,"endMs":900}}]})];
        materials.extend((1..=count).map(|p| {
            image(
                p,
                vec![paragraph(&format!("p{p}"), &format!("第{p}页独立文字。"))],
            )
        }));
        materials[1]["purpose"] = json!("cover");
        let original = materials.clone();
        let d = dossier(materials);
        assert_eq!(
            d.materials, original,
            "the plan does not rewrite canonical text"
        );
        assert_eq!(ordered_refs(&d).len(), count as usize + 1);
        assert_eq!(ordered_refs(&d).first(), Some(&"p1"));
        assert_eq!(ordered_refs(&d).last(), Some(&"a"));
        assert_eq!(d.reading_view["sections"][0]["kind"], "images");
        assert_eq!(d.reading_view["sections"][1]["kind"], "audio");
        assert_eq!(d.reading_view["supplementBlockRefs"], json!(["p1"]));
        assert_eq!(
            d.reading_view["mainBlockRefs"][0], "a",
            "the Works view retains its original primary order"
        );
        let output = markdown(&d);
        for p in 1..=count {
            assert_eq!(output.matches(&format!("第{p}页独立文字。")).count(), 1);
        }
    }
}

#[test]
fn cross_page_sentence_needs_syntax_and_real_boundary_evidence() {
    let original = vec![
        image(1, vec![paragraph("p1", "先写出验收条件，")]),
        image(2, vec![paragraph("p2", "再列明交付时间。")]),
    ];
    let d = dossier(original.clone());
    assert_eq!(groups(&d).len(), 1);
    assert_eq!(groups(&d)[0]["mode"], "paragraph");
    assert_eq!(
        group_text(&d, groups(&d)[0]),
        "先写出验收条件，再列明交付时间。"
    );
    assert_eq!(d.materials, original);
    for (left, right) in [("第一个条件", "第二个条件"), ("已经完成。", "另一个条件")]
    {
        let d = dossier(vec![
            image(1, vec![paragraph("p1", left)]),
            image(2, vec![paragraph("p2", right)]),
        ]);
        assert_eq!(
            groups(&d).len(),
            2,
            "page geometry is not sentence evidence"
        );
    }
    for change in [
        "missing_page",
        "no_bbox",
        "material_issue",
        "block_issue",
        "role",
        "unknown_order",
    ] {
        let mut d = dossier(original.clone());
        match change {
            "missing_page" => d.materials[1]["page"] = json!(3),
            "no_bbox" => {
                d.materials[0]["blocks"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("bbox");
            }
            "material_issue" => {
                d.materials[0]["issues"] = json!([{"code":"structure_unconfirmed"}])
            }
            "block_issue" => d.materials[1]["blocks"][0]["issues"] = json!([{"code":"unclear"}]),
            "role" => d.materials[1]["blocks"][0]["speaker"] = json!("unconfirmed"),
            _ => d.coverage["imageInventory"]["unknownOrder"] = json!(true),
        }
        super::super::rebuild(&mut d);
        assert_eq!(groups(&d).len(), 2, "{change}");
    }
}

#[test]
fn list_numbering_and_table_rows_require_actual_continuation() {
    let list = |r: &str, start| json!({"ref":r,"kind":"list","ordered":true,"start":start,"items":["先定义输出","再列明边界"],"speaker":"author"});
    for (start, expected) in [(3, "list"), (4, "blocks"), (1, "blocks")] {
        let d = dossier(vec![
            image(1, vec![list("l1", 1)]),
            image(2, vec![list("l2", start)]),
        ]);
        assert_eq!(groups(&d)[0]["mode"], expected);
        if expected == "list" {
            let output = markdown(&d);
            for marker in [
                "1. 先定义输出",
                "2. 再列明边界",
                "3. 先定义输出",
                "4. 再列明边界",
            ] {
                assert!(output.contains(marker), "{output}");
            }
        }
    }
    let table = |r: &str, number: &str| json!({"ref":r,"kind":"table","columns":["序号","条件"],"rows":[[number,"完整的条件文字"]],"speaker":"author","bbox":[0.1,0.05,0.8,0.9]});
    for (number, expected) in [("2", "table"), ("1", "blocks"), ("项目二", "blocks")] {
        let d = dossier(vec![
            image(1, vec![table("t1", "1")]),
            image(2, vec![table("t2", number)]),
        ]);
        assert_eq!(groups(&d)[0]["mode"], expected);
        if expected == "table" {
            assert_eq!(markdown(&d).matches("| 序号 | 条件 |").count(), 1);
        }
    }
}

fn tag_materials(body: &str) -> Vec<Value> {
    vec![
        json!({"ref":"body","kind":"body","sourceRef":"same-native-source","blocks":[
        {"ref":"body-tags","kind":"paragraph","text":body,"speaker":"author"}]}),
        json!({"ref":"tags","kind":"tags","sourceRef":"same-native-source","blocks":[
            {"ref":"native-tags","kind":"list","ordered":false,"items":["任务","任务","边界"],"speaker":"author"}]}),
    ]
}

#[test]
fn tags_are_only_a_proven_equivalent_display_with_duplicates_preserved() {
    let original = tag_materials(" #任务[话题]#\n #任务# #边界# ");
    let d = dossier(original.clone());
    assert_eq!(d.materials, original);
    assert_eq!(groups(&d)[0]["mode"], "tags");
    assert_eq!(tags_for_group(&d, groups(&d)[0]), ["任务", "任务", "边界"]);
    assert_eq!(ordered_refs(&d), ["body-tags", "native-tags"]);
    for body in [
        "正文不能被标签隐藏。 #任务# #任务# #边界#",
        "#任务 #任务# #边界#",
        "#任务# #边界#",
        "#[话题]#",
    ] {
        assert_eq!(groups(&dossier(tag_materials(body))).len(), 2, "{body}");
    }
    for change in ["ordered", "source", "role"] {
        let mut input = original.clone();
        match change {
            "ordered" => input[1]["blocks"][0]["ordered"] = json!(true),
            "source" => input[1]["sourceRef"] = json!("another-source"),
            _ => input[1]["blocks"][0]["speaker"] = json!("other"),
        }
        assert_eq!(groups(&dossier(input)).len(), 2, "{change}");
    }
}

#[test]
fn audio_keeps_all_characters_and_separates_roles_materials_and_sentences() {
    let blocks: Vec<_> = (0..36)
        .map(|i| {
            json!({"ref":format!("a{i}"),"kind":"paragraph",
        "text":format!("第{i}段原话含否定和边界。"),"speaker":if i<18{"author"}else{"other"},
        "time":{"startMs":i*8000,"endMs":i*8000+7900}})
        })
        .collect();
    let input = vec![json!({"ref":"audio","kind":"audio","blocks":blocks})];
    let d = dossier(input.clone());
    assert_eq!(d.materials, input);
    assert_eq!(ordered_refs(&d).len(), 36);
    assert_eq!(groups(&d).len(), 12);
    let joined = groups(&d)
        .iter()
        .map(|g| group_text(&d, g))
        .collect::<String>();
    assert_eq!(
        joined,
        blocks
            .iter()
            .map(|b| b["text"].as_str().unwrap())
            .collect::<String>()
    );
    let output = markdown(&d);
    for block in blocks {
        assert_eq!(output.matches(block["text"].as_str().unwrap()).count(), 1);
    }
    let d = dossier(vec![json!({"ref":"audio","kind":"audio","blocks":[
        {"ref":"a0","kind":"paragraph","text":"没有标点的连续","time":{"startMs":0,"endMs":30000}},
        {"ref":"a1","kind":"paragraph","text":"同一句话不能按字数切断","time":{"startMs":30000,"endMs":60000}}]})]);
    assert_eq!(groups(&d).len(), 1);
}

#[test]
fn segment_clips_frozen_groups_joiners_and_single_side_tags() {
    let full = dossier(vec![json!({"ref":"audio","kind":"audio","blocks":[
        {"ref":"a0","kind":"paragraph","text":"One","time":{"startMs":0,"endMs":100}},
        {"ref":"a1","kind":"paragraph","text":"two","time":{"startMs":100,"endMs":200}},
        {"ref":"a2","kind":"paragraph","text":"三","time":{"startMs":200,"endMs":300}}]})]);
    let group_ref = groups(&full)[0]["ref"].clone();
    let mut middle = full.clone();
    let metadata = segment(&mut middle, 1, 1).unwrap();
    assert_eq!(metadata["completeRevision"], full.revision);
    let g = groups(&middle)[0];
    assert_eq!(g["ref"], group_ref);
    assert_eq!(g["joiners"], json!([]));
    assert_eq!(
        g["continuation"],
        json!({"fromPrevious":true,"toNext":true})
    );
    let mut tail = full.clone();
    segment(&mut tail, 1, 2).unwrap();
    assert_eq!(groups(&tail)[0]["joiners"], json!([""]));
    assert_eq!(
        groups(&tail)[0]["continuation"],
        json!({"fromPrevious":true,"toNext":false})
    );
    let mut empty = full;
    segment(&mut empty, 3, 1).unwrap();
    assert_eq!(empty.reading_view["sections"], json!([]));
    assert!(ordered_refs(&empty).is_empty());
    let full = dossier(tag_materials("#任务# #任务# #边界#"));
    for offset in [0, 1] {
        let mut page = full.clone();
        segment(&mut page, offset, 1).unwrap();
        assert_eq!(groups(&page)[0]["mode"], "tags");
        assert_eq!(
            tags_for_group(&page, groups(&page)[0]),
            ["任务", "任务", "边界"]
        );
        assert_eq!(markdown(&page).matches("#任务　#任务　#边界").count(), 1);
    }
}

#[test]
fn actual_missing_pages_are_positioned_once_across_transport_segments() {
    let mut full = dossier(vec![
        image(1, vec![paragraph("p1", "前文完整。")]),
        image(3, vec![paragraph("p3", "这一项继续，")]),
        image(4, vec![paragraph("p4", "保留中间条件，")]),
        image(5, vec![paragraph("p5", "然后结束。")]),
    ]);
    full.coverage["imageInventory"]["missingPages"] = json!([2, 6]);
    super::super::rebuild(&mut full);
    let section = &full.reading_view["sections"][0];
    assert_eq!(section["groups"].as_array().unwrap().len(), 2);
    assert_eq!(
        section["gaps"],
        json!([
            {"beforeGroupRef":section["groups"][1]["ref"],"missingPages":[2]},
            {"beforeGroupRef":null,"missingPages":[6]}
        ])
    );
    let output = markdown(&full);
    assert!(output.find("前文完整").unwrap() < output.find("第 2 页文字待补全").unwrap());
    assert!(output.find("第 2 页文字待补全").unwrap() < output.find("这一项继续").unwrap());
    assert_eq!(output.matches("第 2 页文字待补全").count(), 1);
    let mut recovered = vec![];
    for offset in 0..4 {
        let mut part = full.clone();
        segment(&mut part, offset, 1).unwrap();
        let pages: Vec<_> = part.reading_view["sections"][0]["gaps"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|g| g["missingPages"].as_array().unwrap())
            .cloned()
            .collect();
        assert_eq!(
            pages,
            match offset {
                1 => vec![json!(2)],
                3 => vec![json!(6)],
                _ => vec![],
            }
        );
        recovered.extend(pages);
    }
    assert_eq!(recovered, vec![json!(2), json!(6)]);
    let mut selected = full.clone();
    crate::research_dossier::select(
        &mut selected,
        &crate::research_dossier::Selection {
            image_pages: vec![3, 4, 5],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        selected.reading_view["sections"][0]["gaps"],
        json!([]),
        "no missing pages outside explicit selection"
    );
    full.coverage["imageInventory"]["unknownOrder"] = json!(true);
    super::super::rebuild(&mut full);
    assert_eq!(
        full.reading_view["sections"][0]["gaps"],
        json!([]),
        "unknown order is not guessed"
    );
}

#[test]
fn text_gaps_include_failed_unread_pages_without_changing_source_inventory() {
    let mut materials: Vec<_> = (1..=10)
        .map(|page| {
            image(
                page,
                vec![paragraph(
                    &format!("p{page}"),
                    &format!("第{page}页完整原文。"),
                )],
            )
        })
        .collect();
    let mut failed = image(11, vec![]);
    failed["processing"] = json!({"state":"failed","reason":"synthetic_failure"});
    let mut missing = image(12, vec![]);
    missing["sourceState"] = json!("not_acquired");
    materials.extend([failed, missing]);
    let original = materials.clone();
    let mut full = dossier(materials);
    full.coverage["imageInventory"]["missingPages"] = json!([12]);
    let coverage = full.coverage.clone();
    super::super::rebuild(&mut full);
    assert_eq!(image_text_gap_pages(&full), BTreeSet::from([11, 12]));
    assert_eq!(
        full.reading_view["sections"][0]["gaps"],
        json!([{"beforeGroupRef":null,"missingPages":[11,12]}])
    );
    assert_eq!(full.materials, original);
    assert_eq!(
        full.coverage, coverage,
        "source acquisition missingPages remains [12]"
    );
    let output = markdown(&full);
    assert_eq!(output.matches("第 11、12 页文字待补全").count(), 1);
    assert!(output.contains("第 11 页：处理失败。"));
    let mut recovered = vec![];
    for offset in 0..10 {
        let mut part = full.clone();
        segment(&mut part, offset, 1).unwrap();
        assert_eq!(ordered_refs(&part), [format!("p{}", offset + 1)]);
        let gaps = part.reading_view["sections"][0]["gaps"].as_array().unwrap();
        recovered.extend(
            gaps.iter()
                .flat_map(|gap| gap["missingPages"].as_array().unwrap())
                .cloned(),
        );
        if offset == 9 {
            let failed = part
                .materials
                .iter()
                .find(|unit| unit["page"] == 11)
                .unwrap();
            assert_eq!(failed["sourceState"], "present");
            assert_eq!(failed["processing"]["state"], "failed");
            assert_eq!(failed["blocks"], json!([]));
            assert_eq!(
                part.materials.len(),
                3,
                "selected block plus actual gap metadata only"
            );
            assert_eq!(part.selection["omittedUnits"], 9);
            assert_eq!(markdown(&part).matches("第 11、12 页文字待补全").count(), 1);
        } else {
            assert_eq!(part.materials.len(), 1);
        }
    }
    assert_eq!(recovered, [json!(11), json!(12)]);
    for state in ["known_empty", "not_applicable"] {
        let mut value = full.clone();
        value.materials[10]["sourceState"] = json!(state);
        super::super::rebuild(&mut value);
        assert_eq!(image_text_gap_pages(&value), BTreeSet::from([12]));
        assert_eq!(
            value.reading_view["sections"][0]["gaps"],
            json!([{"beforeGroupRef":null,"missingPages":[12]}])
        );
    }
    let mut readable = full.clone();
    readable.materials[10]["blocks"] = json!([paragraph("p11", "已有正文仍需保留。")]);
    readable.materials[10]["issues"] = json!([{"key":"unclear_word","message":"局部一字待确认"}]);
    super::super::rebuild(&mut readable);
    assert_eq!(image_text_gap_pages(&readable), BTreeSet::from([12]));
    let output = markdown(&readable);
    assert!(output.contains("已有正文仍需保留。") && output.contains("局部一字待确认"));
}

#[test]
fn text_gap_scope_and_unknown_order_do_not_invent_page_positions() {
    let mut missing = image(2, vec![]);
    missing["processing"] = json!({"state":"running"});
    let mut full = dossier(vec![
        image(1, vec![paragraph("p1", "第一段完整。")]),
        missing,
        image(3, vec![paragraph("p3", "第三段完整。")]),
    ]);
    assert_eq!(
        full.reading_view["sections"][0]["gaps"][0]["beforeGroupRef"],
        full.reading_view["sections"][0]["groups"][1]["ref"]
    );
    assert_eq!(
        full.reading_view["sections"][0]["gaps"][0]["missingPages"],
        json!([2])
    );
    full.coverage["imageInventory"]["unknownOrder"] = json!(true);
    super::super::rebuild(&mut full);
    assert_eq!(full.reading_view["sections"][0]["gaps"], json!([]));
    assert!(markdown(&full).contains("第 2 页文字待补全。"));
    let mut selected = full.clone();
    crate::research_dossier::select(
        &mut selected,
        &crate::research_dossier::Selection {
            image_pages: vec![2],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(selected.reading_view["sections"], json!([]));
    assert!(markdown(&selected).contains("第 2 页文字待补全。"));
    full.materials[1]["page"] = Value::Null;
    super::super::rebuild(&mut full);
    assert!(
        image_text_gap_pages(&full).is_empty(),
        "no page number is invented for an observed image"
    );
}

#[test]
fn all_gap_page_keeps_its_selected_diagnostic_and_only_metadata_in_other_segments() {
    let diagnostic =
        json!({"ref":"g2","kind":"gap","text":"这一处无法辨认","speaker":"unconfirmed"});
    let full = dossier(vec![
        image(1, vec![paragraph("p1", "第一段完整。")]),
        image(2, vec![diagnostic.clone()]),
        image(3, vec![paragraph("p3", "第三段完整。")]),
    ]);
    assert_eq!(image_text_gap_pages(&full), BTreeSet::from([2]));
    assert_eq!(full.materials[1]["blocks"], json!([diagnostic]));
    let mut selected = full.clone();
    segment(&mut selected, 1, 1).unwrap();
    assert_eq!(ordered_refs(&selected), ["g2"]);
    assert_eq!(selected.materials[0]["blocks"][0], diagnostic);
    assert_eq!(markdown(&selected).matches("这一处无法辨认").count(), 1);
    let mut following = full.clone();
    segment(&mut following, 2, 1).unwrap();
    assert_eq!(ordered_refs(&following), ["p3"]);
    let context = following
        .materials
        .iter()
        .find(|unit| unit["page"] == 2)
        .unwrap();
    assert_eq!(context["blocks"], json!([]));
    assert_eq!(
        following.reading_view["sections"][0]["gaps"][0]["missingPages"],
        json!([2])
    );
    assert!(!markdown(&following).contains("这一处无法辨认"));
    assert_eq!(full.materials[1]["blocks"][0], diagnostic);
}

#[test]
fn grouping_is_revision_bound_but_a_new_clock_alone_is_not() {
    let mut d = dossier(vec![image(1, vec![paragraph("p", "完整原文。")])]);
    let before = d.revision.clone();
    d.as_of = "2026-10-27T01:00:00Z".into();
    refresh_revision(&mut d);
    assert_eq!(d.revision, before);
    d.reading_view["sections"][0]["groups"][0]["continuation"]["toNext"] = json!(true);
    refresh_revision(&mut d);
    assert_ne!(d.revision, before);
}

#[test]
fn wrapped_lines_on_unknown_page_join_without_crossing_paragraphs_or_selections() {
    let material = json!({"ref":"image","kind":"image","sourceState":"present","page":null,
        "issues":[{"code":"page_order_unknown"}],"blocks":[
        {"ref":"p0","kind":"paragraph","speaker":"unconfirmed","text":"这里是一段跨行的完整","bbox":[0.1,0.14,0.8,0.037]},
        {"ref":"p1","kind":"paragraph","speaker":"unconfirmed","text":"文字。","bbox":[0.1,0.20,0.35,0.035]},
        {"ref":"p2","kind":"paragraph","speaker":"unconfirmed","text":"这是下一段内容。","bbox":[0.1,0.32,0.8,0.037]}]});
    let mut d = dossier(vec![material]);
    let groups = d.reading_view["sections"][0]["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0]["mode"], "paragraph");
    assert_eq!(groups[0]["blockRefs"], json!(["p0", "p1"]));
    crate::research_dossier::select(
        &mut d,
        &crate::research_dossier::Selection {
            block_refs: vec!["p0".into(), "p2".into()],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        d.reading_view["sections"][0]["groups"]
            .as_array()
            .unwrap()
            .iter()
            .all(|g| g["blockRefs"].as_array().unwrap().len() == 1)
    );
}

#[test]
fn wrapped_large_font_prose_is_one_paragraph_but_real_heading_stays_separate() {
    let mut blocks: Vec<Value> = ["第一行大字号正文延续", "第二行正文继续说明", "第三行还在同一个段落", "结束。"]
        .into_iter().enumerate().map(|(i, text)| json!({
            "ref":format!("p{i}"),"kind":if i < 3 {"heading"} else {"paragraph"},
            "text":text,"speaker":"unconfirmed","bbox":[0.1,0.1+i as f64*0.08,if i<3 {0.8} else {0.2},0.06]
        })).collect();
    let d = dossier(vec![image(1, blocks.clone())]);
    assert_eq!(groups(&d).len(), 1);
    assert_eq!(groups(&d)[0]["mode"], "paragraph");
    assert_eq!(groups(&d)[0]["blockRefs"], json!(["p0", "p1", "p2", "p3"]));
    assert_eq!(d.materials[0]["blocks"], json!(blocks));
    blocks[3]["bbox"] = json!([0.1, 0.6, 0.8, 0.03]);
    let d = dossier(vec![image(1, blocks)]);
    assert_eq!(groups(&d).len(), 4);
}

#[test]
fn same_image_wrap_never_merges_speakers_or_recognition_issues() {
    for different_speaker in [false, true] {
        let mut blocks = vec![
            paragraph("a", "这是一段换行的内容"),
            paragraph("b", "后续内容"),
        ];
        blocks[0]["bbox"] = json!([0.1, 0.1, 0.8, 0.04]);
        blocks[1]["bbox"] = json!([0.1, 0.16, 0.8, 0.04]);
        if different_speaker {
            blocks[1]["speaker"] = json!("quoted");
        } else {
            blocks[1]["issues"] = json!([{"code":"uncertain_text"}]);
        }
        let mut material = image(1, blocks);
        material["issues"] = json!([{"code":"page_order_unknown"}]);
        let d = dossier(vec![material]);
        assert_eq!(groups(&d).len(), 2);
    }
}

#[test]
fn tilted_same_line_fragments_read_left_to_right_without_rewriting_blocks() {
    let blocks = vec![
        json!({"ref":"right","kind":"paragraph","text":"继续阅读","bbox":[0.686,0.928,0.181,0.024]}),
        json!({"ref":"middle","kind":"paragraph","text":"列清单，","bbox":[0.583,0.931,0.095,0.028]}),
        json!({"ref":"left","kind":"paragraph","text":"构建结构。","bbox":[0.197,0.937,0.381,0.037]}),
    ];
    let d = dossier(vec![image(1, blocks.clone())]);
    assert_eq!(ordered_refs(&d), vec!["left", "middle", "right"]);
    assert_eq!(groups(&d).len(), 1);
    assert_eq!(group_text(&d, groups(&d)[0]), "构建结构。列清单，继续阅读");
    assert_eq!(d.materials[0]["blocks"], json!(blocks));
    let mut selected = d.clone();
    crate::research_dossier::select(
        &mut selected,
        &crate::research_dossier::Selection {
            block_refs: vec!["left".into(), "right".into()],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(ordered_refs(&selected), vec!["left", "right"]);
    assert_eq!(groups(&selected).len(), 2);
    assert_eq!(selected.materials[0]["blocks"][0], blocks[2]);
    assert_eq!(selected.materials[0]["blocks"][1], blocks[0]);
    let mut columns = blocks;
    columns[2]["bbox"][2] = json!(0.2);
    let d = dossier(vec![image(1, columns)]);
    assert_eq!(
        groups(&d).len(),
        2,
        "separate columns cannot be joined across a wide gap"
    );
}
