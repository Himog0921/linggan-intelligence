use super::*;
use serde_json::Value;
use crate::comment_study_run::bounded_context_manifest;

fn key(work: u128, id: &str) -> CommentKey {
    CommentKey { work_ref: Uuid::from_u128(work), comment_external_id: id.into() }
}
fn command() -> StartStudyRunCommand {
    serde_json::from_value(json!({"requestRef":Uuid::from_u128(100),
        "domainRef":crate::comment_study_source::ADHD_DOMAIN_REF,"policyRef":Uuid::from_u128(11),
        "scope":{"kind":"works","workRefs":[Uuid::from_u128(10)]},"mode":"new_only",
        "limits":{"commentBudget":100,"contextCharacterBudget":6000,"tokenLimit":100000},"reason":null})).unwrap()
}
fn candidate(work: u128, rank: u64) -> SelectionCandidate {
    SelectionCandidate { comment_key:key(work,&format!("comment-{rank}")),
        source_ref:Uuid::from_u128(work*10000+u128::from(rank)),source_rank:rank,
        source_flags:[false;7],in_progress:false,latest:None,input_fingerprint:Some("a".repeat(64)) }
}
fn history(state: StudyTargetState) -> SelectionHistory {
    SelectionHistory { state, input_fingerprint:Some("a".repeat(64)),legacy_stopped_without_live_invocation:false }
}
fn context() -> Value {
    bounded_context_manifest(&json!({"contract":"comment-study.context.v1","workRef":Uuid::from_u128(10),
        "cleanerVersion":"comment-clean.v2","sources":[{"sourceRef":Uuid::from_u128(20),
        "kind":"native_title","slotOrdinal":null,"text":"合成作品","characterCount":4}]}),6000)
}
fn parent(state: &str) -> Value {
    json!({"commentKey":key(10,"parent"),"sourceRef":Uuid::from_u128(30),
        "sourceState":state,"commentText":"孩子做作业需要家长督促","researchText":"not trusted"})
}
fn prepared(raw: &str) -> PreparedStudyInput {
    prepare_study_input(&key(10,"评论-A"),Uuid::from_u128(21),raw,&context(),None).unwrap()
}
fn selections(rows: &[SelectionCandidate], mode: StudySelectionMode) -> StudySelection {
    let mut preview=command().preview(); preview.mode=mode;
    choose_study_targets(&preview,rows).unwrap()
}

#[test]
fn start_matches_approved_example_and_every_root_field_is_required() {
    let fixture=include_str!("../../../../docs/data-contracts/comment-study-productization-001/start-run.example.json");
    assert!(serde_json::from_str::<StartStudyRunCommand>(fixture).unwrap().normalize().is_ok());
    let value=serde_json::to_value(command()).unwrap();
    for field in ["requestRef","domainRef","policyRef","scope","mode","limits","reason"] {
        let mut missing=value.clone(); missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<StartStudyRunCommand>(missing).is_err(),"{field}");
    }
}
#[test]
fn unknown_root_scope_and_budget_fields_are_rejected() {
    for field in ["origin","originRef","scheduledFor","methodHash","force"] {
        let mut value=serde_json::to_value(command()).unwrap(); value[field]=json!(true);
        assert!(serde_json::from_value::<StartStudyRunCommand>(value).is_err());
    }
    for (section,field) in [("scope","commentKeys"),("limits","temperature")] {
        let mut value=serde_json::to_value(command()).unwrap(); value[section][field]=json!([]);
        assert!(serde_json::from_value::<StartStudyRunCommand>(value).is_err());
    }
    let mut preview=serde_json::to_value(command().preview()).unwrap(); preview["requestRef"]=json!(Uuid::new_v4());
    assert!(serde_json::from_value::<SelectionPreviewCommand>(preview).is_err());
}
#[test]
fn both_scope_variants_reject_crossed_fields_and_missing_members() {
    for value in [json!({"kind":"works"}),json!({"kind":"comments"}),
        json!({"kind":"comments","workRefs":[],"commentKeys":[]}),
        json!({"kind":"domain_backlog"})] {
        assert!(serde_json::from_value::<StudyScope>(value).is_err());
    }
}
#[test]
fn all_three_limits_and_scope_sizes_are_enforced() {
    for (comments,context,tokens) in [(0,6000,100000),(3001,6000,100000),(100,0,100000),
        (100,20001,100000),(100,6000,1023),(100,6000,10000001)] {
        let mut c=command(); c.limits=StudyRunLimits { comment_budget:comments,context_character_budget:context,token_limit:tokens };
        assert_eq!(c.normalize().unwrap_err(),StudySelectionError::InvalidLimit);
    }
    for scope in [StudyScope::Works { work_refs:vec![] }, StudyScope::Works { work_refs:vec![Uuid::from_u128(10);101] },
        StudyScope::Comments { comment_keys:vec![key(10,"c");3001] },
        StudyScope::Comments { comment_keys:(1..=101).map(|i|key(i,"c")).collect() }] {
        let mut c=command();c.scope=scope;assert!(c.normalize().is_err());
    }
}
#[test]
fn stable_ids_use_utf8_bytes_and_preserve_literal_whitespace() {
    assert!(key(10,&"中".repeat(170)).validate().is_ok());
    assert!(key(10,&"中".repeat(171)).validate().is_err());
    for id in ["", "  ", "a\0b"] { assert!(key(10,id).validate().is_err()); }
    assert!(key(0,"c").validate().is_err());
    let mut c=command();c.scope=StudyScope::Comments { comment_keys:vec![key(10," c "),key(10,"c")] };
    let normalized=c.normalize().unwrap();
    assert_eq!(normalized.scope,StudyScope::Comments {comment_keys:vec![key(10," c "),key(10,"c")]});
}
#[test]
fn non_new_modes_require_explicit_reason_without_inserting_it_into_input() {
    for mode in [StudySelectionMode::InputChanged,StudySelectionMode::RetryFailed,StudySelectionMode::Reanalyse] {
        for reason in [None,Some(" ".into()),Some("a\0b".into()),Some("中".repeat(501))] {
            let mut c=command();c.mode=mode;c.reason=reason;assert!(c.normalize().is_err());
        }
        let mut c=command();c.mode=mode;c.reason=Some("  明确复核  ".into());
        assert_eq!(c.normalize().unwrap().reason.as_deref(),Some("明确复核"));
    }
    let mut c=command();c.reason=Some("".into());assert!(c.normalize().is_err());
}
#[test]
fn nil_and_other_domain_never_become_defaults() {
    for field in ["requestRef","policyRef","domainRef"] {
        let mut value=serde_json::to_value(command()).unwrap();value[field]=json!(Uuid::nil());
        assert!(serde_json::from_value::<StartStudyRunCommand>(value).unwrap().normalize().is_err());
    }
    assert!(study_domain_lock_key(Uuid::from_u128(9)).is_err());
}
#[test]
fn set_normalization_keeps_start_and_preview_identical() {
    let mut a=command();a.scope=StudyScope::Works { work_refs:vec![Uuid::from_u128(12),Uuid::from_u128(10),Uuid::from_u128(12)] };
    let mut b=a.clone();b.scope=StudyScope::Works { work_refs:vec![Uuid::from_u128(10),Uuid::from_u128(12)] };
    assert_eq!(a.manual_request_hash().unwrap(),b.manual_request_hash().unwrap());
    assert_eq!(a.clone().normalize().unwrap().scope,a.preview().normalize().unwrap().scope);
    a.scope=StudyScope::Comments {comment_keys:vec![key(10,"b"),key(10,"a"),key(10,"b")]};
    b.scope=StudyScope::Comments {comment_keys:vec![key(10,"a"),key(10,"b")]};
    assert_eq!(a.manual_request_hash().unwrap(),b.manual_request_hash().unwrap());
}
#[test]
fn manual_intent_hash_binds_policy_scope_and_every_budget_not_the_receipt_key() {
    let c=command();let original=c.manual_request_hash().unwrap();
    for mut changed in [c.clone(),c.clone(),c.clone(),c.clone(),c.clone()].into_iter().enumerate() {
        match changed.0 {0=>changed.1.limits.comment_budget+=1,1=>changed.1.limits.context_character_budget+=1,
            2=>changed.1.limits.token_limit+=1,3=>changed.1.policy_ref=Uuid::new_v4(),
            _=>changed.1.scope=StudyScope::Works {work_refs:vec![Uuid::from_u128(13)]}}
        assert_ne!(original,changed.1.manual_request_hash().unwrap());
    }
    let mut new_key=c.clone();new_key.request_ref=Uuid::new_v4();
    assert_eq!(original,new_key.manual_request_hash().unwrap());
    let mut intent=c;intent.mode=StudySelectionMode::Reanalyse;intent.reason=Some("复核".into());
    assert_ne!(original,intent.manual_request_hash().unwrap());
}
#[test]
fn domain_lock_key_matches_independent_python_sha256_signed_big_endian() {
    assert_eq!(study_domain_lock_key(command().domain_ref).unwrap(),-4846399579964473942_i64);
}
#[test]
fn second_new_only_selection_from_688_excludes_the_completed_first_100() {
    let mut rows:Vec<_>=(1..=688).map(|n|candidate(10,n)).collect();
    let first=selections(&rows,StudySelectionMode::NewOnly);
    for index in &first.selected_indices { rows[*index].latest=Some(history(StudyTargetState::Succeeded)); }
    let second=selections(&rows,StudySelectionMode::NewOnly);
    assert_eq!(first.target_count,100);assert_eq!(second.target_count,100);
    assert!(first.selected_indices.iter().all(|i|!second.selected_indices.contains(i)));
    assert_eq!(second.exclusion_counts["notSelectedByMode"],100);
    assert_eq!(second.exclusion_counts["budgetNotSelected"],488);
    assert_eq!(second.scope_comment_count,second.target_count+second.exclusion_counts.values().sum::<usize>());
}
#[test]
fn mode_truth_table_preserves_unknown_and_does_not_retry_every_day() {
    use StudySelectionMode::*;use StudyTargetState::*;
    for state in [Succeeded,NoSignal,NeedsContext,Failed,Excluded,Cancelled] {
        let mut row=candidate(10,1);row.latest=Some(history(state));
        assert_eq!(selections(&[row.clone()],NewOnly).target_count,0);
        assert_eq!(selections(&[row.clone()],InputChanged).target_count,0);
        assert_eq!(selections(&[row.clone()],RetryFailed).target_count,usize::from(matches!(state,Failed|Cancelled)));
        assert_eq!(selections(&[row.clone()],Reanalyse).target_count,1);
        row.input_fingerprint=Some("b".repeat(64));
        assert_eq!(selections(&[row.clone()],InputChanged).target_count,1);
        row.latest.as_mut().unwrap().input_fingerprint=None;
        assert_eq!(selections(&[row],InputChanged).target_count,0);
    }
    assert_eq!(selections(&[candidate(10,1)],InputChanged).target_count,0);
    assert_eq!(selections(&[candidate(10,1)],RetryFailed).target_count,0);
}
#[test]
fn authorized_in_progress_wins_over_all_modes_even_after_a_newer_success() {
    for mode in [StudySelectionMode::NewOnly,StudySelectionMode::InputChanged,StudySelectionMode::RetryFailed,StudySelectionMode::Reanalyse] {
        let mut row=candidate(10,1);row.in_progress=true;row.latest=Some(history(StudyTargetState::Succeeded));
        let result=selections(&[row],mode);
        assert_eq!(result.target_count,0);assert_eq!(result.exclusion_counts["inProgress"],1);
    }
}
#[test]
fn legacy_unfinished_requires_proven_stop_and_no_live_invocation() {
    for state in [StudyTargetState::Ready,StudyTargetState::Queued,StudyTargetState::Running] {
        let mut row=candidate(10,1);row.latest=Some(history(state));
        assert_eq!(selections(&[row.clone()],StudySelectionMode::RetryFailed).target_count,0);
        assert_eq!(selections(&[row.clone()],StudySelectionMode::Reanalyse).target_count,0);
        row.latest.as_mut().unwrap().legacy_stopped_without_live_invocation=true;
        assert_eq!(selections(&[row.clone()],StudySelectionMode::RetryFailed).target_count,1);
        assert_eq!(selections(&[row.clone()],StudySelectionMode::InputChanged).target_count,0);
        row.in_progress=true;
        assert_eq!(selections(&[row],StudySelectionMode::RetryFailed).target_count,0);
    }
}
#[test]
fn shared_source_precedence_and_all_ten_count_keys_are_preserved() {
    let mut rows=Vec::new();
    for i in 0..7 { let mut row=candidate(10,i+1);row.source_flags[i as usize..].fill(true);row.in_progress=true;rows.push(row); }
    let result=selections(&rows,StudySelectionMode::Reanalyse);
    assert_eq!(result.exclusion_counts.len(),10);
    for name in EXCLUSIONS.iter().take(7) {assert_eq!(result.exclusion_counts[name],1);}
    assert_eq!(result.exclusion_counts["inProgress"],0);
}
#[test]
fn partial_indexing_does_not_prevent_available_selection() {
    let mut rows:Vec<_>=(1..=100).map(|n|candidate(10,n)).collect();
    for row in &mut rows[60..] {row.source_flags[2]=true;row.input_fingerprint=None;}
    let result=selections(&rows,StudySelectionMode::NewOnly);
    assert_eq!(result.target_count,60);assert_eq!(result.exclusion_counts["indexPending"],40);
    for row in &mut rows[..60] {row.source_flags[2]=true;row.input_fingerprint=None;}
    assert_eq!(selections(&rows,StudySelectionMode::NewOnly).target_count,0);
}
#[test]
fn round_robin_reranks_after_exclusion_and_retains_oldest_per_work() {
    let mut request=command().preview();request.scope=StudyScope::Works { work_refs:vec![Uuid::from_u128(10),Uuid::from_u128(11)] };
    request.limits.comment_budget=3;
    let mut excluded=candidate(10,1);excluded.latest=Some(history(StudyTargetState::NoSignal));
    let rows=vec![candidate(11,3),candidate(10,3),excluded,candidate(11,1),candidate(10,2)];
    let selected=choose_study_targets(&request,&rows).unwrap();
    assert_eq!(selected.selected_indices,vec![4,3,1]);
}
#[test]
fn scope_and_snapshot_corruption_do_not_silently_become_empty_success() {
    let p=command().preview();let row=candidate(10,1);
    assert!(choose_study_targets(&p,&[row.clone(),row.clone()]).is_err());
    assert!(choose_study_targets(&p,&[candidate(11,1)]).is_err());
    let mut invalid=row.clone();invalid.input_fingerprint=Some("A".repeat(64));
    assert!(choose_study_targets(&p,&[invalid]).is_err());
    let mut invalid=row.clone();invalid.source_rank=0;assert!(choose_study_targets(&p,&[invalid]).is_err());
    let mut p=p;p.scope=StudyScope::Comments {comment_keys:vec![key(10,"missing")]};
    assert!(choose_study_targets(&p,&[]).is_err());
    let mut second=row.clone();second.comment_key=key(10,"different");
    assert!(choose_study_targets(&command().preview(),&[row,second]).is_err());
}
#[test]
fn content_fingerprint_matches_independent_python_golden() {
    assert_eq!(prepared("孩子做作业总是拖延").input_fingerprint,"c61512a54fa4aba0a9c5b1d9f898656d8c22432425e3ae61b2bed8a71165d652");
}
#[test]
fn material_reobservation_changes_audit_hash_but_not_content_fingerprint() {
    let first=prepared("孩子做作业总是拖延");let mut ctx=context();
    ctx["sources"][0]["sourceRef"]=json!(Uuid::from_u128(99));
    let again=prepare_study_input(&key(10,"评论-A"),Uuid::from_u128(98),"孩子做作业总是拖延",&ctx,None).unwrap();
    assert_eq!(first.input_fingerprint,again.input_fingerprint);
    assert_ne!(first.input_hash,again.input_hash);
    assert!(first.input_manifest.get("workContext").is_none());
    assert_eq!(first.input_manifest["contract"],"comment-study.target-input.v2");
}
#[test]
fn raw_punctuation_and_whitespace_changes_are_not_lost_by_cleaning() {
    let first=prepared("孩子做作业总是拖延");
    let space=prepared(" 孩子做作业总是拖延 ");
    assert_eq!(first.research_sha256,space.research_sha256);
    assert_ne!(first.input_fingerprint,space.input_fingerprint);
    assert_ne!(first.input_fingerprint,prepared("孩子做作业总是拖延。").input_fingerprint);
}
#[test]
fn unused_context_omissions_do_not_trigger_research_but_retained_text_does() {
    let first=prepared("孩子做作业总是拖延");let mut ctx=context();
    ctx["omitted"]=json!([{"sourceRef":Uuid::new_v4(),"kind":"image_substantive_text","characterCount":12000}]);
    ctx["omittedFragmentCount"]=json!(1);ctx["truncated"]=json!(true);
    let again=prepare_study_input(&key(10,"评论-A"),Uuid::from_u128(21),"孩子做作业总是拖延",&ctx,None).unwrap();
    assert_eq!(first.input_fingerprint,again.input_fingerprint);assert_ne!(first.input_hash,again.input_hash);
    ctx["sources"][0]["text"]=json!("另一作品");
    let changed=prepare_study_input(&key(10,"评论-A"),Uuid::from_u128(21),"孩子做作业总是拖延",&ctx,None).unwrap();
    assert_ne!(first.input_fingerprint,changed.input_fingerprint);
}
#[test]
fn retained_fragment_order_is_part_of_content_equivalence() {
    let mut ctx=context();let second=json!({"kind":"body","text":"合成正文","characterCount":4,
        "sourceRef":Uuid::from_u128(22),"slotOrdinal":null});
    ctx["sources"].as_array_mut().unwrap().push(second);ctx["includedCharacterCount"]=json!(8);
    let first=prepare_study_input(&key(10,"评论-A"),Uuid::from_u128(21),"孩子做作业总是拖延",&ctx,None).unwrap();
    ctx["sources"].as_array_mut().unwrap().reverse();
    let second=prepare_study_input(&key(10,"评论-A"),Uuid::from_u128(21),"孩子做作业总是拖延",&ctx,None).unwrap();
    assert_ne!(first.input_fingerprint,second.input_fingerprint);
}
#[test]
fn filling_parent_dependency_changes_fingerprint_without_changing_raw() {
    let missing=prepared("我也是");let p=parent("known");
    let filled=prepare_study_input(&key(10,"评论-A"),Uuid::from_u128(21),"我也是",&context(),Some(&p)).unwrap();
    assert_eq!(missing.dependency_state,"parent_required_missing");assert_eq!(filled.dependency_state,"parent_available");
    assert_eq!(missing.raw_sha256,filled.raw_sha256);assert_ne!(missing.input_fingerprint,filled.input_fingerprint);
    assert_ne!(filled.input_manifest["parentContext"]["researchText"],"not trusted");
}
#[test]
fn parent_reobservation_does_not_change_fingerprint_but_parent_body_does() {
    let mut p=parent("known");
    let first=prepare_study_input(&key(10,"评论-A"),Uuid::from_u128(21),"我也是",&context(),Some(&p)).unwrap();
    p["sourceRef"]=json!(Uuid::from_u128(31));
    let next=prepare_study_input(&key(10,"评论-A"),Uuid::from_u128(21),"我也是",&context(),Some(&p)).unwrap();
    assert_eq!(first.input_fingerprint,next.input_fingerprint);assert_ne!(first.input_hash,next.input_hash);
    p["commentText"]=json!("孩子完成作业不再需要督促");
    let changed=prepare_study_input(&key(10,"评论-A"),Uuid::from_u128(21),"我也是",&context(),Some(&p)).unwrap();
    assert_ne!(first.input_fingerprint,changed.input_fingerprint);
}
#[test]
fn restricted_and_unknown_parents_never_leak_attached_stale_text() {
    let mut fingerprints=Vec::new();
    for state in ["restricted","unknown"] {
        let p=parent(state);
        let input=prepare_study_input(&key(10,"评论-A"),Uuid::from_u128(21),"我也是",&context(),Some(&p)).unwrap();
        assert_eq!(input.dependency_state,"parent_required_missing");assert_eq!(input.parent_source_ref,None);
        assert!(!input.input_manifest.to_string().contains("家长督促"));fingerprints.push(input.input_fingerprint);
    }
    assert_eq!(fingerprints[0],fingerprints[1]);
}
#[test]
fn direct_comment_does_not_fingerprint_unused_parent_context() {
    let first=prepared("孩子做作业总是拖延");let p=parent("known");
    let second=prepare_study_input(&key(10,"评论-A"),Uuid::from_u128(21),"孩子做作业总是拖延",&context(),Some(&p)).unwrap();
    assert_eq!(first.input_fingerprint,second.input_fingerprint);assert!(second.input_manifest["parentContext"].is_null());
}
#[test]
fn corrupt_context_parent_and_unusable_raw_are_rejected_without_truncation() {
    let k=key(10,"评论-A");
    for raw in ["😀","@合成用户","", "bad\0text"] {
        assert!(prepare_study_input(&k,Uuid::from_u128(21),raw,&context(),None).is_err());
    }
    assert!(prepare_study_input(&k,Uuid::from_u128(21),&"长".repeat(16001),&context(),None).is_err());
    for (field,value) in [("workRef",json!(Uuid::from_u128(11))),("cleanerVersion",json!("old")),
        ("includedCharacterCount",json!(0)),("characterBudget",json!(3)),("sources",json!(null))] {
        let mut ctx=context();ctx[field]=value;
        assert!(prepare_study_input(&k,Uuid::from_u128(21),"孩子做作业总是拖延",&ctx,None).is_err());
    }
    let mut p=parent("known");p["commentKey"]=json!(key(11,"parent"));
    assert!(prepare_study_input(&k,Uuid::from_u128(21),"我也是",&context(),Some(&p)).is_err());
}
