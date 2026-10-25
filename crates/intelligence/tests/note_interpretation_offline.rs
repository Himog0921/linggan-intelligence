//! Synthetic only: no database, network, environment configuration or model process.
use linggan_evidence::dossier_input::{
    self, Column, ColumnState, Fragment, Material, ProcessingScope,
};
use linggan_intelligence::note_interpretation_offline::*;
use uuid::Uuid;

fn input() -> Input {
    Input {
        material: Material {
            reading_title: serde_json::json!({"text":null,"basis":"missing","blockRefs":[],"platformSource":null}),
            contract: dossier_input::VERSION.into(),
            fingerprint_contract: dossier_input::FINGERPRINT_VERSION.into(),
            domain_ref: Uuid::from_u128(1),
            work_ref: Uuid::from_u128(2),
            as_of: "2026-10-20T00:00:00Z".into(),
            dossier_revision: "synthetic-dossier".into(),
            material_fingerprint: "synthetic-fingerprint".into(),
            provenance_hash: "synthetic-provenance".into(),
            snapshot_identity: "synthetic-snapshot".into(),
            ready: true,
            not_ready_reasons: vec![],
            columns: ["body", "title", "tags", "ocr", "transcript", "frame_text"]
                .into_iter()
                .map(|field| Column {
                    field: field.into(),
                    state: if field == "body" {
                        ColumnState::Available
                    } else {
                        ColumnState::NotProvided
                    },
                    reasons: vec![],
                })
                .collect(),
            fragments: vec![Fragment {
                citation_id: "S1".into(),
                field: "body".into(),
                logical_position: "native".into(),
                source_ref: Uuid::from_u128(3),
                source_version: "synthetic.v1".into(),
                start: 7,
                end: 14,
                text_hash: "synthetic-hash".into(),
                text: "今天🙂先写作业".into(),
                speaker: "author".into(),
                structure: serde_json::json!({"ref":"native","kind":"paragraph","text":"今天🙂先写作业","speaker":"author"}),
                issues: vec![],
                reading_ref: Some(Uuid::from_u128(30)),
                unit_ref: Some(Uuid::from_u128(31)),
            }],
            layer_version: "synthetic.v3".into(),
            processing_scope: ProcessingScope {
                included: vec![],
                context_only: vec![],
                not_connected: vec![],
            },
        },
        direction_version_ref: Uuid::from_u128(4),
        skill_version_ref: Uuid::from_u128(5),
        direction_refs: vec![Uuid::from_u128(6), Uuid::from_u128(7)],
    }
}

#[test]
fn mock_is_stable_explicit_unknown_and_bound_to_versions() {
    let input = input();
    let a = mock_unknown(&input).unwrap();
    let b = mock_unknown(&input).unwrap();
    assert_eq!(
        serde_json::to_value(&a).unwrap(),
        serde_json::to_value(&b).unwrap()
    );
    assert_eq!(a.origin, Origin::OfflineMock);
    assert_eq!(a.relevance, Relevance::Unknown);
    assert_eq!(a.perspective, Perspective::Unknown);
    assert!(a.primary_direction.is_none());
    validate_output(&input, &a).unwrap();
    for field in ["work", "snapshot", "directions", "skill"] {
        let mut output = mock_unknown(&input).unwrap();
        match field {
            "work" => output.identity.work_ref = Uuid::from_u128(99),
            "snapshot" => output.identity.snapshot_identity = "changed".into(),
            "directions" => output.identity.direction_version_ref = Uuid::from_u128(99),
            _ => output.identity.skill_version_ref = Uuid::from_u128(99),
        }
        assert_eq!(
            validate_output(&input, &output),
            Err(Invalid("offline_output_identity_mismatch"))
        );
    }
}

#[test]
fn partial_material_keeps_available_fragments_and_empty_text_is_explicit() {
    let mut input = input();
    input.material.ready = false;
    input.material.not_ready_reasons = vec!["restricted".into()];
    for state in [
        ColumnState::Waiting,
        ColumnState::NotAcquired,
        ColumnState::Restricted,
    ] {
        for c in &mut input.material.columns {
            if c.field == "body" {
                c.state = state.clone();
            }
        }
        assert!(mock_unknown(&input).is_ok());
    }
    input.material.fragments.clear();
    assert_eq!(
        mock_unknown(&input).unwrap_err(),
        Invalid("offline_material_no_readable_text")
    );
}

#[test]
fn unrelated_unknown_no_suitable_and_secondary_direction_remain_distinct() {
    let input = input();
    let mut output = mock_unknown(&input).unwrap();
    for relevance in [Relevance::NotRelated, Relevance::Unknown] {
        output.relevance = relevance;
        output.primary_direction = None;
        validate_output(&input, &output).unwrap();
        output.primary_direction = Some(input.direction_refs[0]);
        assert!(validate_output(&input, &output).is_err());
    }
    output.relevance = Relevance::Related;
    output.primary_direction = None;
    validate_output(&input, &output).unwrap();
    output.primary_direction = Some(input.direction_refs[0]);
    output.also_related = Some(input.direction_refs[1]);
    validate_output(&input, &output).unwrap();
    output.also_related = output.primary_direction;
    assert_eq!(
        validate_output(&input, &output),
        Err(Invalid("offline_duplicate_direction"))
    );
    output.also_related = Some(Uuid::from_u128(99));
    assert!(validate_output(&input, &output).is_err());
}

#[test]
fn keyword_offsets_use_unicode_scalars_and_absolute_fragment_positions() {
    let input = input();
    let mut output = mock_unknown(&input).unwrap();
    output.keywords = vec![Keyword {
        text: "🙂先".into(),
        citation_id: "S1".into(),
        start: 9,
        end: 11,
    }];
    validate_output(&input, &output).unwrap();
    output.keywords[0].end = 12;
    assert!(validate_output(&input, &output).is_err());
    output.keywords[0].start = usize::MAX;
    output.keywords[0].end = usize::MAX;
    assert!(validate_output(&input, &output).is_err());
    output.keywords[0].start = 0;
    output.keywords[0].end = 2;
    assert!(validate_output(&input, &output).is_err());
}

#[test]
fn partial_field_keeps_good_citations_while_missing_ambiguous_and_ineligible_are_rejected() {
    let mut input = input();
    let mut output = mock_unknown(&input).unwrap();
    output.keywords = vec![Keyword {
        text: "今天".into(),
        citation_id: "S1".into(),
        start: 7,
        end: 9,
    }];
    validate_output(&input, &output).unwrap();
    for field in ["asr", "image_body", "author_name", "comment"] {
        input.material.fragments[0].field = field.into();
        assert!(validate_output(&input, &output).is_err());
    }
    input.material.fragments[0].field = "body".into();
    input.material.columns[0].state = ColumnState::Restricted;
    validate_output(&input, &output).unwrap(); // aggregate coverage cannot invalidate an admitted block
    input.material.columns[0].state = ColumnState::Available;
    input
        .material
        .fragments
        .push(input.material.fragments[0].clone());
    assert!(validate_output(&input, &output).is_err());
    input.material.fragments.clear();
    assert!(validate_output(&input, &output).is_err());
}

#[test]
fn reason_limit_is_two_hundred_characters_and_clearing_is_explicit() {
    let input = input();
    let mut output = mock_unknown(&input).unwrap();
    output.reason = Some("🙂".repeat(200));
    validate_output(&input, &output).unwrap();
    output.reason = Some("🙂".repeat(201));
    assert!(validate_output(&input, &output).is_err());
    output.reason = None;
    validate_output(&input, &output).unwrap();
}

#[test]
fn output_cannot_claim_provider_origin_or_hide_unknown_fields() {
    let input = input();
    let mut value = serde_json::to_value(mock_unknown(&input).unwrap()).unwrap();
    value["origin"] = "provider".into();
    assert!(serde_json::from_value::<Output>(value.clone()).is_err());
    value["origin"] = "offline_mock".into();
    value["published"] = true.into();
    assert!(serde_json::from_value::<Output>(value).is_err());
}

#[test]
fn preview_preserves_all_canonical_fields_order_roles_structure_and_missing_states() {
    let mut input = input();
    for (i, field) in ["ocr", "transcript", "frame_text"].into_iter().enumerate() {
        let mut fragment = input.material.fragments[0].clone();
        fragment.field = field.into();
        fragment.citation_id = format!("media-{i}");
        fragment.logical_position = format!("media-block-{i}");
        fragment.text = format!("{}末尾仍保留🙂", "完整图片音频帧文字".repeat(3000));
        fragment.end = fragment.start + fragment.text.chars().count();
        fragment.speaker = "unconfirmed".into();
        fragment.structure = serde_json::json!({"ref":fragment.logical_position,"kind":"paragraph","text":fragment.text,"speaker":"unconfirmed"});
        input.material.fragments.push(fragment);
        input
            .material
            .columns
            .iter_mut()
            .find(|c| c.field == field)
            .unwrap()
            .state = ColumnState::Available;
    }
    input
        .material
        .columns
        .iter_mut()
        .find(|c| c.field == "tags")
        .unwrap()
        .state = ColumnState::NotProvided;
    let preview = prepare_input(&input).unwrap();
    assert_eq!(preview.columns, input.material.columns);
    assert_eq!(preview.fragments, input.material.fragments);
    assert_eq!(preview.fragments[1].speaker, "unconfirmed");
    assert!(preview.fragments[3].text.ends_with("末尾仍保留🙂"));
    let encoded = serde_json::to_string(&preview).unwrap();
    assert_eq!(
        encoded,
        serde_json::to_string(&prepare_input(&input).unwrap()).unwrap()
    );
}

#[test]
fn preview_rejects_missing_duplicate_and_contradictory_columns() {
    let mut input = input();
    input.material.columns[0].state = ColumnState::Restricted;
    assert!(prepare_input(&input).is_ok()); // individual restricted sources are omitted by the shared reader
    input.material.columns[0].state = ColumnState::Available;
    input
        .material
        .columns
        .push(input.material.columns[0].clone());
    assert!(prepare_input(&input).is_err());
    input.material.columns.pop();
    input.material.columns.retain(|c| c.field != "body");
    assert!(prepare_input(&input).is_err());
}

#[test]
fn five_literal_keywords_are_accepted_and_a_sixth_is_rejected() {
    let input = input();
    let mut output = mock_unknown(&input).unwrap();
    let six: Vec<_> = "今天🙂先写作"
        .chars()
        .enumerate()
        .map(|(i, c)| Keyword {
            text: c.to_string(),
            citation_id: "S1".into(),
            start: 7 + i,
            end: 8 + i,
        })
        .collect();
    output.keywords = six[..5].to_vec();
    validate_output(&input, &output).unwrap();
    output.keywords = six;
    assert_eq!(
        validate_output(&input, &output),
        Err(Invalid("offline_too_many_keywords"))
    );
}

fn catalogue(count: usize) -> CatalogueDraft {
    CatalogueDraft {
        origin: Origin::OfflineMock,
        version_ref: Uuid::from_u128(100),
        groups: vec![DirectionGroup {
            group_ref: Uuid::from_u128(101),
            name: "合成生活大类".into(),
        }],
        sample_work_refs: vec![Uuid::from_u128(102), Uuid::from_u128(103)],
        directions: (0..count)
            .map(|i| DirectionDraft {
                direction_ref: Uuid::from_u128(1000 + i as u128),
                group_ref: Uuid::from_u128(101),
                name: format!("合成方向{i}"),
                definition: "仅测试结构，不是正式方向定义".into(),
                example_work_refs: vec![Uuid::from_u128(102), Uuid::from_u128(103)],
            })
            .collect(),
    }
}

#[test]
fn catalogue_is_two_levels_with_twenty_to_forty_directions() {
    for count in [20, 40] {
        catalogue(count).validate().unwrap();
    }
    for count in [0, 19, 41] {
        assert!(catalogue(count).validate().is_err());
    }
    let mut draft = catalogue(20);
    draft.directions[0].group_ref = Uuid::from_u128(999);
    assert!(draft.validate().is_err());
    draft.directions[0].group_ref = draft.groups[0].group_ref;
    draft.directions[0].definition = " ".into();
    assert!(draft.validate().is_err());
}

#[test]
fn examples_are_two_or_three_distinct_members_of_the_declared_sample() {
    let mut draft = catalogue(20);
    draft.directions[0].example_work_refs = vec![draft.sample_work_refs[0]];
    assert!(draft.validate().is_err());
    draft.directions[0].example_work_refs = vec![draft.sample_work_refs[0]; 2];
    assert!(draft.validate().is_err());
    draft.directions[0].example_work_refs = vec![draft.sample_work_refs[0], Uuid::from_u128(999)];
    assert!(draft.validate().is_err());
    draft.sample_work_refs.push(Uuid::from_u128(104));
    draft.directions[0].example_work_refs = draft.sample_work_refs.clone();
    draft.validate().unwrap();
}

#[test]
fn catalogue_membership_and_version_build_the_same_input_identity() {
    let source = input();
    let draft = catalogue(20);
    let prepared =
        Input::from_catalogue_draft(source.material, source.skill_version_ref, &draft).unwrap();
    assert_eq!(prepared.direction_version_ref, draft.version_ref);
    assert_eq!(
        prepared.direction_refs,
        draft
            .directions
            .iter()
            .map(|d| d.direction_ref)
            .collect::<Vec<_>>()
    );
    let mut output = mock_unknown(&prepared).unwrap();
    output.relevance = Relevance::Related;
    output.primary_direction = Some(draft.directions[0].direction_ref);
    validate_output(&prepared, &output).unwrap();
    let mut changed = draft.clone();
    changed.directions[1].direction_ref = changed.directions[0].direction_ref;
    assert!(
        Input::from_catalogue_draft(prepared.material, source.skill_version_ref, &changed).is_err()
    );
}

#[test]
fn explicit_sample_freeze_is_order_independent_and_binds_input_and_provenance() {
    let first = input();
    let mut second = first.clone();
    second.material.work_ref = Uuid::from_u128(90);
    let baseline =
        freeze_sample("synthetic-20261020-v1", &[first.clone(), second.clone()]).unwrap();
    let reversed =
        freeze_sample("synthetic-20261020-v1", &[second.clone(), first.clone()]).unwrap();
    assert_eq!(baseline, reversed);
    assert_eq!(baseline.entries.len(), 2);
    assert_eq!(baseline.manifest_sha256.len(), 64);
    for field in [
        "literal",
        "provenance",
        "snapshot",
        "direction_members",
        "skill",
    ] {
        let mut a = first.clone();
        let mut b = second.clone();
        match field {
            "literal" => {
                let fragment = &mut a.material.fragments[0];
                fragment.text = "今天🙂先做家务".into();
                fragment.end = fragment.start + fragment.text.chars().count();
                fragment.text_hash = linggan_evidence::creator_discovery::hash(&fragment.text);
                fragment.structure["text"] = serde_json::json!(fragment.text);
            }
            "provenance" => a.material.provenance_hash = "different-source".into(),
            "snapshot" => a.material.snapshot_identity = "different-snapshot".into(),
            "direction_members" => {
                a.direction_refs[0] = Uuid::from_u128(91);
                b.direction_refs = a.direction_refs.clone();
            }
            _ => {
                a.skill_version_ref = Uuid::from_u128(92);
                b.skill_version_ref = a.skill_version_ref;
            }
        }
        let changed = freeze_sample("synthetic-20261020-v1", &[a, b]).unwrap();
        assert_ne!(changed.manifest_sha256, baseline.manifest_sha256, "{field}");
    }
    assert_ne!(
        freeze_sample("synthetic-v2", &[first, second])
            .unwrap()
            .manifest_sha256,
        baseline.manifest_sha256
    );
}

#[test]
fn sample_does_not_silently_drop_or_mix_invalid_members() {
    let first = input();
    assert_eq!(
        freeze_sample("v1", &[]),
        Err(Invalid("offline_sample_empty"))
    );
    assert!(freeze_sample(" ", &[first.clone()]).is_err());
    assert!(freeze_sample("v1", &[first.clone(), first.clone()]).is_err());
    for field in [
        "domain",
        "as_of",
        "directions",
        "skill",
        "restricted",
        "nil",
    ] {
        let mut second = first.clone();
        second.material.work_ref = Uuid::from_u128(90);
        match field {
            "domain" => second.material.domain_ref = Uuid::from_u128(99),
            "as_of" => second.material.as_of = "2026-10-21T00:00:00Z".into(),
            "directions" => {
                second.direction_refs.pop();
            }
            "skill" => second.skill_version_ref = Uuid::from_u128(99),
            "restricted" => second.material.fragments.clear(),
            _ => second.material.work_ref = Uuid::nil(),
        }
        assert!(
            freeze_sample("v1", &[first.clone(), second]).is_err(),
            "{field}"
        );
    }
}

#[test]
fn catalogue_examples_bind_the_exact_frozen_sample_not_just_a_declared_list() {
    let source = input();
    let mut draft = catalogue(20);
    let inputs: Vec<_> = draft
        .sample_work_refs
        .iter()
        .map(|work_ref| {
            let mut material = source.material.clone();
            material.work_ref = *work_ref;
            Input::from_catalogue_draft(material, source.skill_version_ref, &draft).unwrap()
        })
        .collect();
    let frozen = freeze_sample("synthetic-v1", &inputs).unwrap();
    draft.validate_sample(&frozen).unwrap();
    draft.sample_work_refs.push(Uuid::from_u128(999));
    assert_eq!(
        draft.validate_sample(&frozen),
        Err(Invalid("offline_catalogue_sample_mismatch"))
    );
    draft.sample_work_refs.pop();
    draft.version_ref = Uuid::from_u128(999);
    assert!(draft.validate_sample(&frozen).is_err());
}

fn choice(input: &Input, primary: Uuid) -> ManualJudgment {
    ManualJudgment {
        origin: Origin::OfflineMock,
        identity: input.identity(),
        relevance: Relevance::Related,
        primary_direction: Some(primary),
        also_related: None,
        reason: None,
    }
}

#[test]
fn manual_classification_survives_skill_changes_and_preserves_cleared_reason() {
    let old = input();
    let judgment = choice(&old, old.direction_refs[0]);
    let mut current = old.clone();
    current.skill_version_ref = Uuid::from_u128(999);
    let machine = mock_unknown(&current).unwrap();
    let result = project_judgment(&current, &machine, Some((&old, &judgment))).unwrap();
    assert!(result.manual_applied && !result.needs_review);
    assert_eq!(
        result.effective.primary_direction,
        judgment.primary_direction
    );
    assert_eq!(result.effective.identity, current.identity());
    assert_eq!(result.effective.reason, None);
    assert_eq!(result.judgment.unwrap().identity, old.identity());
    assert_eq!(result.machine.relevance, Relevance::Unknown);
}

#[test]
fn disappearing_direction_shows_machine_result_and_keeps_original_for_review() {
    let old = input();
    let judgment = choice(&old, old.direction_refs[0]);
    let mut current = old.clone();
    current.direction_version_ref = Uuid::from_u128(999);
    current.direction_refs.remove(0);
    let machine = mock_unknown(&current).unwrap();
    let result = project_judgment(&current, &machine, Some((&old, &judgment))).unwrap();
    assert!(result.needs_review && !result.manual_applied);
    assert_eq!(result.effective.relevance, Relevance::Unknown);
    assert_eq!(
        result.judgment.unwrap().primary_direction,
        judgment.primary_direction
    );
    let mut changed_source = current.clone();
    changed_source.material.snapshot_identity = "changed".into();
    assert!(
        project_judgment(
            &changed_source,
            &mock_unknown(&changed_source).unwrap(),
            Some((&old, &judgment))
        )
        .is_err()
    );
    let mut forged = judgment.clone();
    forged.identity.skill_version_ref = Uuid::new_v4();
    assert!(project_judgment(&current, &machine, Some((&old, &forged))).is_err());
}

#[test]
fn main_direction_statistics_deduplicate_keywords_and_keep_unknowns_separate() {
    let a = input();
    let mut b = a.clone();
    b.material.work_ref = Uuid::from_u128(91);
    let mut c = a.clone();
    c.material.work_ref = Uuid::from_u128(92);
    let mut output = mock_unknown(&a).unwrap();
    output.relevance = Relevance::Related;
    output.primary_direction = Some(a.direction_refs[0]);
    output.also_related = Some(a.direction_refs[1]);
    output.keywords = vec![
        Keyword {
            text: "今天".into(),
            citation_id: "S1".into(),
            start: 7,
            end: 9
        };
        2
    ];
    let mut second = output.clone();
    second.identity = b.identity();
    let third = mock_unknown(&c).unwrap();
    let notes = [
        NoteStatistics {
            input: &a,
            output: &output,
            likes: Some(1000),
            published_week: Some("2026-W40"),
        },
        NoteStatistics {
            input: &b,
            output: &second,
            likes: None,
            published_week: None,
        },
        NoteStatistics {
            input: &c,
            output: &third,
            likes: Some(6200),
            published_week: None,
        },
    ];
    let result = summarize(&notes).unwrap();
    assert_eq!(
        (
            result.total,
            result.unknown,
            result.unknown_likes,
            result.unknown_week
        ),
        (3, 1, 1, 2)
    );
    assert_eq!(
        result.directions.len(),
        1,
        "secondary direction never adds a group"
    );
    let group = &result.directions[0];
    assert_eq!(
        (
            group.count,
            group.over1000,
            group.over5000,
            group.unknown_likes,
            group.unknown_week
        ),
        (2, 0, 0, 1, 1)
    );
    assert_eq!(
        group.keywords[0].count, 2,
        "count notes, not duplicate keyword occurrences"
    );
    assert_eq!(group.weeks["2026-W40"], 1);
    assert!(
        summarize(&[
            NoteStatistics {
                input: &a,
                output: &output,
                likes: None,
                published_week: None
            },
            NoteStatistics {
                input: &a,
                output: &output,
                likes: None,
                published_week: None
            }
        ])
        .is_err()
    );
}

#[test]
fn fixed_fixture_is_reproducible_and_version_comparison_has_no_omitted_old_direction() {
    use fixture::{PreviewRequest, Scenario};
    let request = PreviewRequest {
        scenario: Scenario::Baseline,
        judgments: vec![],
    };
    let a = fixture::preview(&request).unwrap();
    assert_eq!(
        serde_json::to_value(&a).unwrap(),
        serde_json::to_value(fixture::preview(&request).unwrap()).unwrap()
    );
    let b = fixture::preview(&PreviewRequest {
        scenario: Scenario::Candidate,
        judgments: vec![],
    })
    .unwrap();
    assert_ne!(a.sample.manifest_sha256, b.sample.manifest_sha256);
    assert_eq!(a.summary.directions.len(), 20);
    assert_eq!(
        (
            a.summary.unknown,
            a.summary.not_related,
            a.summary.no_suitable_direction
        ),
        (1, 1, 1)
    );
    validate_comparison(&a.catalogue, &b.catalogue, &b.comparison).unwrap();
    let mut incomplete = b.comparison.clone();
    incomplete.pop();
    assert!(validate_comparison(&a.catalogue, &b.catalogue, &incomplete).is_err());
    let mut duplicate = b.comparison.clone();
    duplicate.push(duplicate[0].clone());
    assert!(validate_comparison(&a.catalogue, &b.catalogue, &duplicate).is_err());
}

#[test]
fn preview_rejects_content_role_and_reference_repairs() {
    for change in ["text", "speaker", "ref", "shape"] {
        let mut input = input();
        let structure = &mut input.material.fragments[0].structure;
        match change {
            "text" => structure["text"] = "different content".into(),
            "speaker" => structure["speaker"] = "other".into(),
            "ref" => structure["ref"] = "another block".into(),
            _ => structure["items"] = serde_json::json!(["second content shape"]),
        }
        assert!(matches!(
            prepare_input(&input),
            Err(Invalid("offline_input_structure_invalid"))
        ));
    }
}
