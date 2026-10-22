//! Synthetic source observations only. No original private OCR text or images.
use linggan_evidence::ocr_adoption::*;

#[path = "ocr_adoption/business_regressions.rs"]
mod business_regressions;

fn image() -> ImageIdentity {
    ImageIdentity {
        work_ref: uuid::Uuid::from_u128(1),
        image_ref: "synthetic:cover:1".into(),
        blob_sha256: content_hash(b"synthetic image"),
        width: 1000,
        height: 1400,
    }
}
fn input() -> ImageInput {
    let image = image();
    ImageInput {
        schema_version: 1,
        image: image.clone(),
        image_role: ImageRole::FirstImage,
        sequence_ordinal: Some(1),
        layout: SavedLayout {
            image: image.clone(),
            pipeline: Pipeline {
                version: "synthetic-vl".into(),
                raw_sha256: content_hash(b"saved layout"),
            },
            completed: true,
            blocks: vec![],
            terminations: vec![],
            require_termination: false,
        },
        rechecks: vec![],
        anchors: SavedAnchors {
            image,
            pipeline: Pipeline {
                version: "synthetic-lines".into(),
                raw_sha256: content_hash(b"saved lines"),
            },
            lines: vec![],
        },
    }
}
fn add(input: &mut ImageInput, label: &str, text: &str, bbox: [f64; 4]) -> String {
    let n = input.layout.blocks.len() as u32;
    let id = format!("block:{n}");
    input.layout.blocks.push(Block {
        block_ref: id.clone(),
        reading_order: n,
        label: label.into(),
        text: text.into(),
        bbox,
    });
    input.anchors.lines.push(Line {
        line_ref: format!("line:{n}"),
        reading_order: n,
        text: text.into(),
        confidence: 0.99,
        bbox,
    });
    id
}
fn finish(input: &mut ImageInput, index: usize, reason: Option<&str>) {
    let b = &input.layout.blocks[index];
    input.layout.terminations.push(Termination {
        block_ref: b.block_ref.clone(),
        output_sha256: content_hash(b.text.as_bytes()),
        finish_reason: reason.map(str::to_owned),
        ..Default::default()
    });
}
fn title(input: &ImageInput) -> CoverTitle {
    assess(input).unwrap().cover_title
}

#[test]
fn title_uses_text_blocks_and_keeps_platform_title_separate() {
    let mut i = input();
    add(&mut i, "text", "安静的家庭", [100., 180., 850., 270.]);
    add(
        &mut i,
        "text",
        "如何一起度过周末？",
        [100., 330., 850., 420.],
    );
    let result = assess(&i).unwrap();
    assert_eq!(result.cover_title.state, TitleState::Usable);
    assert_eq!(
        result.cover_title.text.as_deref(),
        Some("安静的家庭\n如何一起度过周末？")
    );
    assert_eq!(result.cover_title.fragments.len(), 2);
    assert_eq!(
        display_title(Some("平台的原始标题"), &result).text,
        Some("平台的原始标题")
    );
    assert_eq!(
        display_title(None, &result).text,
        result.cover_title.text.as_deref()
    );
}

#[test]
fn distant_small_caption_does_not_join_large_title() {
    let mut i = input();
    add(
        &mut i,
        "paragraph_title",
        "完整封面标题",
        [80., 150., 920., 290.],
    );
    add(
        &mut i,
        "paragraph_title",
        "一行视频字幕",
        [300., 1120., 700., 1190.],
    );
    let t = title(&i);
    assert_eq!(t.state, TitleState::Usable);
    assert_eq!(t.text.as_deref(), Some("完整封面标题"));
}

#[test]
fn low_title_can_be_the_only_real_title() {
    let mut i = input();
    add(
        &mut i,
        "doc_title",
        "从下方开始的完整标题",
        [50., 1120., 950., 1220.],
    );
    assert_eq!(title(&i).state, TitleState::Usable);
}

#[test]
fn no_title_is_a_result_not_a_generated_summary() {
    let mut i = input();
    add(
        &mut i,
        "text",
        "这是一段普通小字正文，保留来源，不给它拟标题。",
        [50., 200., 950., 230.],
    );
    let a = assess(&i).unwrap();
    assert_eq!(a.cover_title.state, TitleState::None);
    assert!(a.cover_title.text.is_none());
    assert_eq!(a.regions[0].usable_fragments.len(), 1);
}

#[test]
fn interior_heading_and_unknown_sequence_do_not_create_cover_title() {
    let mut i = input();
    add(
        &mut i,
        "doc_title",
        "内页章节标题",
        [100., 200., 900., 300.],
    );
    i.image_role = ImageRole::Interior;
    i.sequence_ordinal = Some(2);
    assert_eq!(title(&i).state, TitleState::NotCover);
    i.image_role = ImageRole::Unknown;
    i.sequence_ordinal = None;
    let t = title(&i);
    assert_eq!(t.state, TitleState::Uncertain);
    assert!(t.text.is_none());
    assert_eq!(t.candidates.len(), 1);
}

#[test]
fn competing_groups_remain_uncertain() {
    let mut i = input();
    add(
        &mut i,
        "paragraph_title",
        "左上方章节标题",
        [50., 100., 450., 200.],
    );
    add(
        &mut i,
        "paragraph_title",
        "右下方章节标题",
        [550., 1000., 950., 1100.],
    );
    assert_eq!(title(&i).state, TitleState::Uncertain);
    assert!(title(&i).reasons.contains(&Reason::CompetingTitleGroups));
}

#[test]
fn title_model_label_alone_and_ui_username_are_insufficient() {
    let mut i = input();
    add(
        &mut i,
        "paragraph_title",
        "某个用户的昵称",
        [100., 100., 320., 125.],
    );
    assert_eq!(title(&i).state, TitleState::None);
}

#[test]
fn anomalous_region_is_blocked_without_discarding_good_neighbor_or_raw() {
    let mut i = input();
    add(
        &mut i,
        "text",
        "这段已保存的文字有来源",
        [50., 200., 950., 230.],
    );
    let bad = format!("后续内容{}", "1".repeat(4039));
    add(&mut i, "text", &bad, [50., 600., 950., 800.]);
    i.anchors.lines[1].text = "后续内容".into();
    finish(&mut i, 1, Some("length"));
    let before = serde_json::to_vec(&i).unwrap();
    let a = assess(&i).unwrap();
    assert_eq!(a.regions[0].quality, Quality::Supported);
    let b = &a.regions[1];
    assert_eq!(b.quality, Quality::Rejected);
    assert!(b.reasons.contains(&Reason::OutputTruncated));
    assert!(b.reasons.contains(&Reason::ExcessiveRepetition));
    assert!(b.usable_fragments.is_empty());
    assert_eq!(b.adoption_state, AdoptionState::Pending);
    assert_eq!(b.fallback_candidates[0].text, "后续内容");
    assert_eq!(b.fallback_candidates[0].source_kind, SourceKind::Line);
    assert_eq!(before, serde_json::to_vec(&i).unwrap());
}

#[test]
fn only_exactly_bound_termination_can_reject_a_region() {
    let mut i = input();
    add(&mut i, "text", "被截断的保存文字", [50., 200., 950., 230.]);
    finish(&mut i, 0, Some("length"));
    assert_eq!(assess(&i).unwrap().regions[0].quality, Quality::Rejected);
    i.layout.terminations[0].output_sha256 = content_hash(b"different output");
    assert_eq!(assess(&i).unwrap_err(), InputError::Receipt);
}

#[test]
fn missing_finish_reason_remains_missing_but_anchor_support_is_independent() {
    let mut i = input();
    add(
        &mut i,
        "text",
        "有逐行支撑的保存文字",
        [50., 200., 950., 230.],
    );
    let a = assess(&i).unwrap();
    assert!(a.regions[0].finish_reason.is_none());
    assert!(a.regions[0].reasons.contains(&Reason::FinishReasonMissing));
    assert_eq!(a.regions[0].quality, Quality::Supported);
}

#[test]
fn interrupted_and_filtered_generations_are_not_successful_text() {
    for reason in ["content_filter", "error", "cancelled", "tool_calls"] {
        let mut i = input();
        add(&mut i, "text", "不完整的生成文字", [50., 200., 950., 230.]);
        finish(&mut i, 0, Some(reason));
        assert_eq!(assess(&i).unwrap().regions[0].quality, Quality::Rejected);
    }
}

#[test]
fn unsupported_image_background_cannot_become_a_fact_even_if_anchors_agree() {
    let mut i = input();
    add(
        &mut i,
        "image",
        "背景机构 地址：未知路99号 电话：01012345678",
        [10., 10., 400., 400.],
    );
    let a = assess(&i).unwrap();
    assert_eq!(a.regions[0].quality, Quality::NeedsReview);
    assert_eq!(a.regions[0].role, Role::Background);
    assert!(a.regions[0].usable_fragments.is_empty());
    assert!(a.cover_title.text.is_none());
}

#[test]
fn silent_rewrite_and_parenthetical_additions_fall_back_without_editing_raw() {
    for (vl, raw) in [
        ("内在学习动力", "内在学习习动力"),
        ("拍拍手（拍手打）", "拍拍手"),
    ] {
        let mut i = input();
        add(&mut i, "text", vl, [50., 200., 950., 230.]);
        i.anchors.lines[0].text = raw.into();
        let a = assess(&i).unwrap();
        assert_eq!(a.regions[0].quality, Quality::NeedsReview);
        assert!(a.regions[0].usable_fragments.is_empty());
        assert_eq!(a.regions[0].fallback_candidates[0].text, raw);
        assert_eq!(i.layout.blocks[0].text, vl);
    }
}

#[test]
fn ordinary_repetitions_numbers_and_multiple_languages_are_not_deleted() {
    for text in [
        "人人人人学会慢慢来",
        "202610211234567890123456789",
        "11111111",
        "こんにちは家族 hello family",
    ] {
        let mut i = input();
        add(&mut i, "text", text, [50., 200., 950., 260.]);
        let r = &assess(&i).unwrap().regions[0];
        assert_eq!(r.quality, Quality::Supported, "{text}");
        assert!(!r.usable_fragments.is_empty(), "{text}");
    }
}

#[test]
fn repeated_cycles_and_thin_binding_decoration_are_flagged() {
    let mut i = input();
    add(
        &mut i,
        "text",
        &"合成异常重复".repeat(20),
        [50., 200., 950., 260.],
    );
    add(
        &mut i,
        "paragraph_title",
        "XXXXXXXXX",
        [50., 500., 950., 515.],
    );
    i.anchors.lines.pop();
    let a = assess(&i).unwrap();
    assert_eq!(a.regions[0].quality, Quality::Rejected);
    assert!(a.regions[1].reasons.contains(&Reason::DecorationLike));
}

#[test]
fn html_is_data_and_table_structure_is_not_adopted_as_proven_relationships() {
    let mut i = input();
    add(
        &mut i,
        "table",
        "<table><tr><td>普通材料</td></tr></table>",
        [50., 200., 950., 600.],
    );
    i.anchors.lines[0].text = "普通材料".into();
    let a = assess(&i).unwrap();
    assert_eq!(a.regions[0].role, Role::StructuredText);
    assert!(a.regions[0].usable_fragments.is_empty());
    assert_eq!(a.regions[0].fallback_candidates[0].text, "普通材料");
    assert_eq!(a.cover_title.state, TitleState::None);
}

#[test]
fn fragments_reconstruct_scalars_not_utf16_and_own_exact_source_boxes() {
    let mut i = input();
    add(
        &mut i,
        "text",
        "家庭🙂 e\u{301}\n共同表达",
        [50., 200., 950., 300.],
    );
    let a = assess(&i).unwrap();
    let f = &a.regions[0].usable_fragments[0];
    assert_eq!(f.end, i.layout.blocks[0].text.chars().count());
    assert_ne!(f.end, i.layout.blocks[0].text.encode_utf16().count());
    let raw: String = i.layout.blocks[0]
        .text
        .chars()
        .skip(f.start)
        .take(f.end - f.start)
        .collect();
    assert_eq!(raw, f.text);
    assert_eq!(f.bbox, i.layout.blocks[0].bbox);
}

#[test]
fn identical_blob_from_another_work_or_image_cannot_supply_anchors() {
    let mut i = input();
    i.anchors.image.work_ref = uuid::Uuid::from_u128(2);
    assert_eq!(assess(&i).unwrap_err(), InputError::Identity);
    i.anchors.image = i.image.clone();
    i.anchors.image.image_ref = "other:slot".into();
    assert_eq!(assess(&i).unwrap_err(), InputError::Identity);
}

#[test]
fn bad_geometry_references_hashes_and_source_position_fail_closed() {
    let mut i = input();
    add(&mut i, "text", "合成测试材料", [50., 200., 950., 300.]);
    let mut bad = i.clone();
    bad.layout.blocks[0].bbox[2] = 1001.;
    assert_eq!(assess(&bad).unwrap_err(), InputError::Geometry);
    let mut bad = i.clone();
    bad.anchors.lines[0].confidence = f64::NAN;
    assert_eq!(assess(&bad).unwrap_err(), InputError::Geometry);
    let mut bad = i.clone();
    bad.layout.blocks.push(bad.layout.blocks[0].clone());
    assert_eq!(assess(&bad).unwrap_err(), InputError::Reference);
    let mut bad = i.clone();
    bad.layout.pipeline.raw_sha256 = "not a hash".into();
    assert_eq!(assess(&bad).unwrap_err(), InputError::Source);
    let mut bad = i.clone();
    bad.sequence_ordinal = None;
    assert_eq!(assess(&bad).unwrap_err(), InputError::Identity);
}

#[test]
fn fingerprints_bind_both_raw_sources_rules_and_image_identity_deterministically() {
    let mut i = input();
    add(&mut i, "text", "合成测试材料", [50., 200., 950., 300.]);
    let a = assess(&i).unwrap();
    assert_eq!(a.input_fingerprint, assess(&i).unwrap().input_fingerprint);
    i.anchors.pipeline.raw_sha256 = content_hash(b"new raw lines");
    assert_ne!(a.input_fingerprint, assess(&i).unwrap().input_fingerprint);
}

#[test]
fn failed_execution_is_unavailable_not_no_text_or_successful_fallback() {
    let mut i = input();
    add(
        &mut i,
        "doc_title",
        "有文字但执行未完成",
        [50., 200., 950., 300.],
    );
    i.layout.completed = false;
    let a = assess(&i).unwrap();
    assert_eq!(a.cover_title.state, TitleState::Unavailable);
    assert!(a.regions[0].usable_fragments.is_empty());
}

#[test]
fn thin_repeated_identifier_with_line_support_is_not_assumed_to_be_decoration() {
    let mut i = input();
    add(&mut i, "text", "11111111", [50., 200., 950., 215.]);
    assert_eq!(assess(&i).unwrap().regions[0].quality, Quality::Supported);
}

#[test]
fn a_short_real_title_is_not_discarded_for_lack_of_sentence_length() {
    let mut i = input();
    add(&mut i, "text", "慢行", [150., 200., 850., 350.]);
    assert_eq!(title(&i).text.as_deref(), Some("慢行"));
}

#[test]
fn output_keeps_source_order_and_rejected_region_location_without_a_usable_fragment() {
    let mut i = input();
    add(&mut i, "image", "无行依据的背景", [50., 200., 950., 400.]);
    i.anchors.lines.clear();
    i.image_role = ImageRole::Interior;
    i.sequence_ordinal = Some(5);
    let a = assess(&i).unwrap();
    assert_eq!(a.sequence_ordinal, Some(5));
    assert_eq!(a.image_role, ImageRole::Interior);
    assert!(a.regions[0].usable_fragments.is_empty());
    assert_eq!(a.regions[0].bbox, i.layout.blocks[0].bbox);
    assert_eq!(a.regions[0].reading_order, i.layout.blocks[0].reading_order);
}

#[test]
fn truncated_prominent_title_is_uncertain_instead_of_confirming_no_title() {
    let mut i = input();
    add(
        &mut i,
        "doc_title",
        "没有生成完整的封面标题",
        [50., 200., 950., 350.],
    );
    finish(&mut i, 0, Some("length"));
    let t = title(&i);
    assert_eq!(t.state, TitleState::Uncertain);
    assert!(t.text.is_none());
    assert_eq!(t.candidates.len(), 1);
}

fn crop(i: &ImageInput, block: usize, kind: RecheckKind, text: &str) -> LocalRecheck {
    LocalRecheck {
        block_ref: i.layout.blocks[block].block_ref.clone(),
        image_blob_sha256: i.image.blob_sha256.clone(),
        crop_bbox: i.layout.blocks[block].bbox,
        crop_sha256: content_hash(b"synthetic crop"),
        raw_sha256: content_hash(text.as_bytes()),
        pipeline_version: "synthetic-local-recheck".into(),
        kind,
        text: text.into(),
        finish_reason: if kind == RecheckKind::VlCrop {
            Some("stop".into())
        } else {
            None
        },
    }
}

#[test]
fn crop_ocr_can_support_visible_letters_missed_by_the_initial_ocr_without_a_whitelist() {
    let mut i = input();
    add(
        &mut i,
        "text",
        "怎样安静地度过周末？",
        [100., 200., 900., 300.],
    );
    i.anchors.lines[0].text = "安静地度过周末？".into();
    assert_eq!(title(&i).state, TitleState::Uncertain);
    i.rechecks
        .push(crop(&i, 0, RecheckKind::LineCrop, "怎样安静地度过周末？"));
    assert_eq!(title(&i).state, TitleState::Usable);
}

#[test]
fn fallback_is_pending_until_a_bound_local_reading_agrees() {
    let mut i = input();
    add(&mut i, "text", "不支持的生成内容", [100., 200., 900., 230.]);
    i.anchors.lines[0].text = "原行可核对的内容".into();
    let a = assess(&i).unwrap();
    assert!(a.regions[0].usable_fragments.is_empty());
    assert!(!a.regions[0].fallback_candidates.is_empty());
    i.rechecks
        .push(crop(&i, 0, RecheckKind::LineCrop, "原行可核对的内容"));
    let a = assess(&i).unwrap();
    assert_eq!(
        a.regions[0].adoption_state,
        AdoptionState::AdoptedLineFallback
    );
    assert_eq!(
        a.regions[0].usable_fragments[0].source_kind,
        SourceKind::Line
    );
}

#[test]
fn same_model_repetition_does_not_validate_a_rewrite_or_unanchored_additions() {
    for (original, anchor) in [
        ("原词动力", "原词行动动力"),
        ("没有依据的一大段补写", "原词"),
    ] {
        let mut i = input();
        add(&mut i, "text", original, [100., 200., 900., 300.]);
        i.anchors.lines[0].text = anchor.into();
        i.rechecks.push(crop(&i, 0, RecheckKind::VlCrop, original));
        assert!(assess(&i).unwrap().regions[0].usable_fragments.is_empty());
    }
}

#[test]
fn wrong_image_recheck_or_disconnected_crop_is_rejected() {
    let mut i = input();
    add(&mut i, "text", "用于复核的材料", [100., 200., 900., 300.]);
    let mut c = crop(&i, 0, RecheckKind::LineCrop, "用于复核的材料");
    c.image_blob_sha256 = content_hash(b"different image");
    i.rechecks.push(c);
    assert_eq!(assess(&i).unwrap_err(), InputError::Receipt);
    i.rechecks[0] = crop(&i, 0, RecheckKind::LineCrop, "用于复核的材料");
    i.rechecks[0].crop_bbox = [10., 10., 90., 90.];
    assert_eq!(assess(&i).unwrap_err(), InputError::Receipt);
}

#[test]
fn required_termination_cannot_be_invented_from_whole_image_success() {
    let mut i = input();
    add(
        &mut i,
        "text",
        "有行支持但缺区域结束回执",
        [100., 200., 900., 300.],
    );
    i.layout.require_termination = true;
    let a = assess(&i).unwrap();
    assert_eq!(a.regions[0].quality, Quality::Rejected);
    assert!(a.regions[0].usable_fragments.is_empty());
    finish(&mut i, 0, Some("stop"));
    assert_eq!(
        assess(&i).unwrap().regions[0].adoption_state,
        AdoptionState::AdoptedBlock
    );
}

#[test]
fn inner_pages_form_ordered_body_with_headings_repetitions_and_exact_image_links() {
    let mut first = input();
    first.image_role = ImageRole::Interior;
    first.sequence_ordinal = Some(2);
    add(
        &mut first,
        "paragraph_title",
        "第一段内页标题",
        [100., 200., 900., 300.],
    );
    add(
        &mut first,
        "text",
        "这段正文保留具体表达",
        [100., 400., 900., 430.],
    );
    let a = assess(&first).unwrap();
    let mut second = first.clone();
    second.image.image_ref = "synthetic:image:3".into();
    second.layout.image = second.image.clone();
    second.anchors.image = second.image.clone();
    second.sequence_ordinal = Some(3);
    let b = assess(&second).unwrap();
    let body = project_work(&[b, a]).unwrap();
    assert_eq!(body.image_text.matches("第一段内页标题").count(), 2);
    assert_eq!(body.images[0].ordinal, Some(2));
    assert!(body.cover_title.is_none());
    for f in &body.fragments {
        let text: String = body
            .image_text
            .chars()
            .skip(f.start)
            .take(f.end - f.start)
            .collect();
        assert_eq!(text, f.source.text);
    }
}

#[test]
fn colored_same_row_words_use_geometry_without_modifying_raw_ordinals() {
    let mut i = input();
    add(&mut i, "text", "怎样安静度过周末", [100., 200., 900., 300.]);
    i.anchors.lines[0].text = "安静".into();
    i.anchors.lines[0].bbox = [300., 210., 450., 290.];
    i.anchors.lines.push(Line {
        line_ref: "prefix".into(),
        reading_order: 1,
        text: "怎样".into(),
        confidence: 0.99,
        bbox: [110., 210., 280., 290.],
    });
    i.anchors.lines.push(Line {
        line_ref: "suffix".into(),
        reading_order: 2,
        text: "度过周末".into(),
        confidence: 0.99,
        bbox: [470., 210., 890., 290.],
    });
    assert_eq!(title(&i).state, TitleState::Usable);
    assert_eq!(i.anchors.lines[0].text, "安静");
}

#[test]
fn upstream_repetition_cannot_be_hidden_by_paddle_cleanup() {
    let mut input = input();
    add(
        &mut input,
        "text",
        "visible words",
        [10.0, 10.0, 900.0, 90.0],
    );
    finish(&mut input, 0, Some("stop"));
    let raw = "1".repeat(4000);
    input.layout.terminations[0].raw_output_sha256 = Some(content_hash(raw.as_bytes()));
    input.layout.terminations[0].raw_text = Some(raw);
    let result = assess(&input).unwrap();
    assert_eq!(result.regions[0].quality, Quality::Rejected);
    assert!(result.regions[0].usable_fragments.is_empty());
}

#[test]
fn screenshot_counter_is_pending_even_when_both_recognizers_agree() {
    let mut input = input();
    add(&mut input, "text", "427", [800.0, 700.0, 850.0, 720.0]);
    add(&mut input, "text", "展开3条回复", [80., 800., 350., 830.]);
    for n in 0..3 {
        input.layout.blocks.push(Block {
            block_ref: format!("icon:{n}"),
            reading_order: n + 2,
            label: "image".into(),
            text: String::new(),
            bbox: [
                10.0,
                100.0 + f64::from(n) * 120.0,
                60.0,
                150.0 + f64::from(n) * 120.0,
            ],
        });
    }
    let a = assess(&input).unwrap();
    assert_eq!(a.regions[0].quality, Quality::Supported);
    assert_eq!(a.regions[0].adoption_state, AdoptionState::Pending);
    assert!(a.regions[0].usable_fragments.is_empty());
    input.layout.blocks.truncate(1);
    assert!(
        !assess(&input).unwrap().regions[0]
            .usable_fragments
            .is_empty(),
        "ordinary numeric material without screenshot context is retained"
    );
}

#[test]
fn avatar_overlap_does_not_exclude_the_comment_below_the_name() {
    let mut i = input();
    i.image_role = ImageRole::Interior;
    i.sequence_ordinal = Some(2);
    for n in 0..3 {
        let y = 100. + n as f64 * 200.;
        add(&mut i, "image", "icon", [10., y, 60., y + 50.]);
        add(
            &mut i,
            "paragraph_title",
            "合成昵称",
            [65., y + 5., 190., y + 25.],
        );
        add(
            &mut i,
            "text",
            "这段材料可以逐字回到原图。",
            [65., y + 35., 650., y + 65.],
        );
    }
    add(&mut i, "text", "译自法语 显示原文", [65., 750., 400., 780.]);
    add(&mut i, "text", "展开8条回复", [65., 800., 400., 830.]);
    add(&mut i, "text", "2026-09-08 回复", [65., 850., 400., 880.]);
    let a = assess(&i).unwrap();
    for n in 0..3 {
        assert_eq!(a.regions[n * 3 + 1].role, Role::LayoutMetadata);
        assert_eq!(
            a.regions[n * 3 + 2].adoption_state,
            AdoptionState::AdoptedBlock
        );
    }
    let work = project_work(&[a]).unwrap();
    assert_eq!(
        work.image_text
            .matches("这段材料可以逐字回到原图。")
            .count(),
        3
    );
    assert!(!work.image_text.contains("合成昵称"));
    assert!(!work.image_text.contains("显示原文"));
    assert!(!work.image_text.contains("回复"));
}

#[test]
fn equations_are_not_promoted_to_a_cover_title_or_flattened_prose() {
    let mut i = input();
    let equation = "1.05365=12.3\n1.04365=9.8";
    add(&mut i, "text", equation, [100., 180., 900., 400.]);
    let a = assess(&i).unwrap();
    assert_eq!(a.cover_title.state, TitleState::Uncertain);
    assert_eq!(a.cover_title.text, None);
    assert_eq!(a.cover_title.candidates[0].text, equation);
    assert_eq!(a.regions[0].adoption_state, AdoptionState::Pending);
    assert!(a.regions[0].usable_fragments.is_empty());
    assert!(project_work(&[a]).unwrap().image_text.is_empty());
    i.rechecks
        .push(crop(&i, 0, RecheckKind::LineCrop, equation));
    assert!(assess(&i).unwrap().regions[0].usable_fragments.is_empty());
}

#[test]
fn numeric_and_mixed_prose_titles_remain_eligible() {
    for text in [
        "2027年学习计划",
        "每天1%进步=一年后的改变",
        "What does E=mc² mean?",
        "42",
    ] {
        let mut i = input();
        add(&mut i, "doc_title", text, [100., 180., 900., 300.]);
        assert_eq!(title(&i).text.as_deref(), Some(text));
    }
}
