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
    assert_eq!(p.image_text, "这段原文保持原样。");
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
    assert_eq!(a.regions[0].adoption_state, AdoptionState::Pending);
    assert!(a.regions[0].usable_fragments.is_empty());
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
