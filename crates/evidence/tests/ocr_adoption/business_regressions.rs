//! Synthetic geometry and words; private source text is never a CI fixture.
use super::*;

fn avatars(i: &mut ImageInput) {
    for n in 0..3 {
        let y = 100. + n as f64 * 200.;
        add(i, "image", "avatar", [10., y, 60., y + 50.]);
        add(
            i,
            "paragraph_title",
            "合成昵称",
            [65., y + 5., 190., y + 25.],
        );
    }
}

#[test]
fn checklist_icons_are_not_enough_to_exclude_supported_prose() {
    let mut i = input();
    i.image_role = ImageRole::Interior;
    i.sequence_ordinal = Some(2);
    for n in 0..3 {
        let y = 550. + n as f64 * 150.;
        add(&mut i, "image", "V", [170., y, 260., y + 85.]);
        add(
            &mut i,
            "text",
            "清晰的合成要点",
            [280., y + 15., 650., y + 80.],
        );
    }
    let a = assess(&i).unwrap();
    let p = project_work(&[a]).unwrap();
    assert_eq!(p.image_text.matches("清晰的合成要点").count(), 3);
}

#[test]
fn conflicting_vl_crop_keeps_a_matching_line_fallback_unconfirmed() {
    let mut i = input();
    add(
        &mut i,
        "text",
        "合成文字word^{o}之后",
        [100., 200., 900., 230.],
    );
    i.anchors.lines[0].text = "合成文字wordα之后".into();
    i.rechecks
        .push(crop(&i, 0, RecheckKind::LineCrop, "合成文字wordα之后"));
    i.rechecks
        .push(crop(&i, 0, RecheckKind::VlCrop, "合成文字word之后"));
    let a = assess(&i).unwrap();
    assert_eq!(a.regions[0].adoption_state, AdoptionState::Pending);
    assert!(a.regions[0].usable_fragments.is_empty());
    assert!(!a.regions[0].fallback_candidates.is_empty());
}

#[test]
fn screenshot_controls_and_inline_ui_are_removed_by_source_ranges() {
    let mut i = input();
    i.image_role = ImageRole::Interior;
    i.sequence_ordinal = Some(2);
    avatars(&mut i);
    let original = "回复 合成昵称：这段原文保持原样。8天前 北京 回复";
    let mixed = add(&mut i, "text", original, [65., 750., 850., 810.]);
    add(&mut i, "text", "作者赞过", [65., 830., 250., 860.]);
    add(&mut i, "text", "8天前 北京 回复", [65., 890., 400., 920.]);
    let a = assess(&i).unwrap();
    let r = a.regions.iter().find(|r| r.block_ref == mixed).unwrap();
    assert_eq!(
        serde_json::to_value(r.adoption_state).unwrap(),
        "adopted_partial_block"
    );
    assert_eq!(r.usable_fragments.len(), 1);
    let f = &r.usable_fragments[0];
    assert_eq!(
        original
            .chars()
            .skip(f.start)
            .take(f.end - f.start)
            .collect::<String>(),
        f.text
    );
    assert_eq!(f.bbox, [65., 750., 850., 810.]);
    let p = project_work(&[a]).unwrap();
    assert_eq!(
        p.image_text,
        "〔部分内容未采用〕\n这段原文保持原样。\n〔部分内容未采用〕"
    );
    assert!(p.images[0].pending_blocks.contains(&mixed));
    assert_eq!(i.layout.blocks[6].text, original);
}

#[test]
fn unsupported_emoji_is_not_certified_by_alphanumeric_normalization() {
    let mut i = input();
    i.image_role = ImageRole::Interior;
    i.sequence_ordinal = Some(2);
    avatars(&mut i);
    add(&mut i, "text", "作者赞过", [65., 750., 250., 780.]);
    add(
        &mut i,
        "text",
        "原文甲🥦，原文乙。",
        [65., 800., 850., 830.],
    );
    i.anchors.lines.last_mut().unwrap().text = "原文甲，原文乙。".into();
    let a = assess(&i).unwrap();
    assert!(
        a.regions
            .last()
            .unwrap()
            .usable_fragments
            .iter()
            .all(|f| !f.text.contains('🥦'))
    );
    let p = project_work(&[a]).unwrap();
    assert!(p.image_text.contains("原文甲"));
    assert!(p.image_text.contains("原文乙"));
    assert!(!p.images[0].pending_blocks.is_empty());
}

#[test]
fn adjacent_smaller_title_tail_joins_but_distant_body_does_not() {
    let mut i = input();
    add(
        &mut i,
        "paragraph_title",
        "周末安排\n先迈一步",
        [70., 300., 430., 540.],
    );
    add(&mut i, "text", "可以慢慢来", [70., 588., 510., 666.]);
    add(
        &mut i,
        "text",
        "以下是具体的行动建议",
        [100., 794., 600., 903.],
    );
    i.anchors.lines[0].text = "周末安排".into();
    i.anchors.lines[0].bbox = [70., 300., 430., 410.];
    i.anchors.lines.push(Line {
        line_ref: "synthetic-second-title-row".into(),
        reading_order: 99,
        text: "先迈一步".into(),
        confidence: 0.99,
        bbox: [70., 420., 430., 540.],
    });
    let a = assess(&i).unwrap();
    assert_eq!(
        a.cover_title.text.as_deref(),
        Some("周末安排\n先迈一步\n可以慢慢来")
    );
    assert_eq!(a.cover_title.fragments.len(), 2);
    assert_eq!(
        project_work(&[a]).unwrap().image_text,
        "以下是具体的行动建议"
    );
}

#[test]
fn long_disagreement_is_not_relaxed_to_a_fuzzy_whole_block_acceptance() {
    let mut i = input();
    let raw = "合成的第一句话。合成的第二句话。合成的第三句话。";
    add(&mut i, "text", raw, [80., 200., 920., 600.]);
    i.anchors.lines[0].text = "合成的第一句话。合成的错误用词。".into();
    i.rechecks.push(crop(&i, 0, RecheckKind::VlCrop, raw));
    let a = assess(&i).unwrap();
    assert_eq!(
        a.regions[0].adoption_state,
        AdoptionState::AdoptedPartialBlock
    );
    assert_eq!(a.regions[0].usable_fragments[0].text, "合成的第一句话。");
    assert_eq!(a.cover_title.state, TitleState::Uncertain);
    assert_eq!(i.layout.blocks[0].text, raw);
}

#[test]
fn one_disputed_word_does_not_erase_other_literal_clauses() {
    let mut i = input();
    i.image_role = ImageRole::Interior;
    i.sequence_ordinal = Some(2);
    let raw = "第一组合成要点、争议好词、第二组合成要点。其余完整说明应该保留。";
    let line = raw.replace("争议好词", "争议坏词");
    add(&mut i, "text", raw, [100., 200., 900., 500.]);
    i.anchors.lines[0].text = line.clone();
    i.rechecks.push(crop(&i, 0, RecheckKind::LineCrop, &line));
    i.rechecks.push(crop(&i, 0, RecheckKind::VlCrop, raw));
    let a = assess(&i).unwrap();
    let r = &a.regions[0];
    assert_eq!(r.adoption_state, AdoptionState::AdoptedPartialBlock);
    for f in &r.usable_fragments {
        assert_eq!(
            raw.chars()
                .skip(f.start)
                .take(f.end - f.start)
                .collect::<String>(),
            f.text
        );
        assert_eq!(f.bbox, i.layout.blocks[0].bbox);
    }
    let p = project_work(&[a]).unwrap();
    assert!(p.image_text.contains("第一组合成要点"));
    assert!(p.image_text.contains("第二组合成要点"));
    assert!(p.image_text.contains("其余完整说明应该保留"));
    assert!(!p.image_text.contains("争议"));
    assert_eq!(p.images[0].pending_blocks.len(), 1);
}

#[test]
fn unsupported_formula_and_orphan_variation_selector_do_not_survive_projection() {
    let mut i = input();
    i.image_role = ImageRole::Interior;
    i.sequence_ordinal = Some(2);
    let raw = "合成文字前半句 $ ^{R} $ 后半句仍然完整保留🏔️。";
    add(&mut i, "text", raw, [100., 200., 900., 300.]);
    i.anchors.lines[0].text = "合成文字前半句后半句仍然完整保留。".into();
    i.rechecks.push(crop(&i, 0, RecheckKind::VlCrop, raw));
    let a = assess(&i).unwrap();
    assert!(a.regions[0].reasons.contains(&Reason::UnverifiedSymbol));
    let p = project_work(&[a]).unwrap();
    assert!(p.image_text.contains("合成文字前半句"));
    assert!(p.image_text.contains("后半句仍然完整保留"));
    assert!(
        !p.image_text
            .chars()
            .any(|c| matches!(c, '$' | '^' | 'R' | '\u{fe0f}' | '🏔'))
    );
    assert_eq!(i.layout.blocks[0].text, raw);
}

#[test]
fn scoped_rechecks_cannot_certify_a_whole_block_or_a_different_crop() {
    let mut i = input();
    let raw = "已有依据的完整句子。没有依据的另外一句。";
    add(&mut i, "text", raw, [100., 200., 900., 600.]);
    i.anchors.lines[0].text = "行文字存在分歧".into();
    for kind in [RecheckKind::LineCrop, RecheckKind::VlCrop] {
        let mut c = crop(&i, 0, kind, "已有依据的完整句子。");
        c.crop_bbox = [100., 200., 900., 350.];
        i.rechecks.push(c);
    }
    let a = assess(&i).unwrap();
    assert_eq!(
        a.regions[0].adoption_state,
        AdoptionState::AdoptedPartialBlock
    );
    assert_eq!(a.regions[0].usable_fragments.len(), 1);
    assert_eq!(
        a.regions[0].usable_fragments[0].text,
        "已有依据的完整句子。"
    );
    assert_eq!(a.cover_title.state, TitleState::Uncertain);
    i.rechecks[1].crop_bbox = [100., 450., 900., 600.];
    assert!(assess(&i).unwrap().regions[0].usable_fragments.is_empty());
    i.rechecks[1].crop_bbox = i.rechecks[0].crop_bbox;
    i.rechecks[1].finish_reason = Some("length".into());
    assert!(assess(&i).unwrap().regions[0].usable_fragments.is_empty());
}

#[test]
fn aggregate_content_stays_pending_without_scoped_support() {
    let mut i = input();
    add(
        &mut i,
        "content",
        "第一条合成评论。\n第二条合成评论。",
        [100., 200., 900., 600.],
    );
    let a = assess(&i).unwrap();
    assert_eq!(a.regions[0].adoption_state, AdoptionState::Pending);
    assert!(a.regions[0].usable_fragments.is_empty());
}

#[test]
fn partial_source_ranges_do_not_leave_orphan_latex_or_remove_supported_prose() {
    let mut i = input();
    i.image_role = ImageRole::Interior;
    i.sequence_ordinal = Some(2);
    let raw =
        "合成前半句很完整 $ \\underline{\\text{这段合成正文应该保留}} $。争议好词、结尾合成要点。";
    let line = "合成前半句很完整这段合成正文应该保留。争议坏词、结尾合成要点。";
    add(&mut i, "text", raw, [100., 200., 900., 600.]);
    i.anchors.lines[0].text = line.into();
    i.rechecks.push(crop(&i, 0, RecheckKind::LineCrop, line));
    i.rechecks.push(crop(&i, 0, RecheckKind::VlCrop, line));
    let a = assess(&i).unwrap();
    let p = project_work(&[a]).unwrap();
    assert!(p.image_text.contains("这段合成正文应该保留"));
    assert!(p.image_text.contains("结尾合成要点"));
    assert!(
        !p.image_text
            .chars()
            .any(|c| matches!(c, '$' | '\\' | '{' | '}'))
    );
}

#[test]
fn aggregate_comments_keep_scoped_bodies_and_exclude_dates_and_counters() {
    let mut i = input();
    i.image_role = ImageRole::Interior;
    i.sequence_ordinal = Some(2);
    let raw = "昵称甲\n这是第一条合成评论正文2028-01-02回复\n昵称乙\n这是第二条合成评论正文2028-01-02回复";
    add(&mut i, "content", raw, [100., 200., 900., 600.]);
    i.anchors.lines[0].text = "没有可靠的全块读法".into();
    for (n, text) in [
        "这是第一条合成评论正文2028-01-02回复",
        "这是第二条合成评论正文2028-01-02回复",
    ]
    .iter()
    .enumerate()
    {
        for kind in [RecheckKind::LineCrop, RecheckKind::VlCrop] {
            let mut c = crop(&i, 0, kind, text);
            c.crop_bbox = [100., 220. + n as f64 * 180., 900., 340. + n as f64 * 180.];
            i.rechecks.push(c);
        }
    }
    let a = assess(&i).unwrap();
    assert_eq!(
        a.regions[0].adoption_state,
        AdoptionState::AdoptedPartialBlock
    );
    let p = project_work(&[a]).unwrap();
    assert_eq!(
        p.image_text,
        "〔部分内容未采用〕\n这是第一条合成评论正文\n〔部分内容未采用〕\n这是第二条合成评论正文\n〔部分内容未采用〕"
    );
    assert_eq!(p.source_relation, "image_transcription_not_author_claim");
    assert_eq!(p.fragments.len(), 2);
    assert_eq!(i.layout.blocks[0].text, raw);
}

#[test]
fn date_in_a_card_is_not_enough_to_establish_a_screenshot() {
    let mut i = input();
    i.image_role = ImageRole::Interior;
    i.sequence_ordinal = Some(2);
    avatars(&mut i);
    add(
        &mut i,
        "text",
        "2028年共同学习计划",
        [65., 750., 600., 780.],
    );
    let p = project_work(&[assess(&i).unwrap()]).unwrap();
    assert!(p.image_text.contains("2028年共同学习计划"));
    assert!(p.image_text.contains("合成昵称"));
}

#[test]
fn navigation_clock_header_is_not_narrative_even_when_model_box_is_oversized() {
    let mut i = input();
    add(&mut i, "doc_title", "<昨天09:18杂字", [1., 20., 999., 600.]);
    i.anchors.lines.clear();
    for (n, (text, y)) in [("昨天", 25.), ("09:18", 60.)].into_iter().enumerate() {
        i.anchors.lines.push(Line {
            line_ref: n.to_string(),
            reading_order: n as u32,
            text: text.into(),
            bbox: [440., y, 540., y + 30.],
            confidence: 0.99,
        });
    }
    let a = assess(&i).unwrap();
    assert_eq!(a.regions[0].role, Role::LayoutMetadata);
    assert!(a.regions[0].usable_fragments.is_empty());
    assert!(project_work(&[a]).unwrap().image_text.is_empty());
    assert_eq!(i.layout.blocks[0].text, "<昨天09:18杂字");
    // A calendar on a poster without navigation is not screenshot metadata.
    i.layout.blocks[0].text = "昨天09:18".into();
    let a = assess(&i).unwrap();
    assert_ne!(a.regions[0].role, Role::LayoutMetadata);
}

#[test]
fn source_gaps_are_typed_and_never_part_of_cited_source_fragments() {
    let mut i = input();
    i.image_role = ImageRole::Interior;
    i.sequence_ordinal = Some(2);
    add(
        &mut i,
        "text",
        "甲段原文争议乙段原文尾部",
        [100., 200., 900., 500.],
    );
    let mut a = assess(&i).unwrap();
    let r = &mut a.regions[0];
    r.adoption_state = AdoptionState::AdoptedPartialBlock;
    r.usable_fragments = [(0, 4, "甲段原文"), (6, 10, "乙段原文")]
        .into_iter()
        .map(|(start, end, text)| Fragment {
            source_kind: SourceKind::Block,
            source_ref: r.block_ref.clone(),
            start,
            end,
            text: text.into(),
            bbox: r.bbox,
        })
        .collect();
    let p = project_work(&[a.clone()]).unwrap();
    assert_eq!(
        p.image_text,
        "甲段原文\n〔部分内容未采用〕\n乙段原文\n〔部分内容未采用〕"
    );
    assert_eq!(
        p.images[0]
            .segments
            .iter()
            .filter(|s| matches!(s, WorkSegment::Gap { .. }))
            .count(),
        2
    );
    for f in &p.fragments {
        assert_eq!(
            p.image_text
                .chars()
                .skip(f.start)
                .take(f.end - f.start)
                .collect::<String>(),
            f.source.text
        );
        assert!(!f.source.text.contains("〔"));
    }
    // Existing decisions stay readable without inventing an old raw-text length.
    a.rule_version = "ocr-adoption-v6".into();
    a.regions[0].source_char_count = None;
    let p = project_work(&[a]).unwrap();
    assert!(matches!(
        p.images[0].segments.last(),
        Some(WorkSegment::Gap {
            source_end: None,
            ..
        })
    ));
}
