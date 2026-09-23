use super::*;

fn command() -> CreateStudyPolicyCommand {
    serde_json::from_value(json!({
        "domainRef":crate::comment_study_source::ADHD_DOMAIN_REF,"methodName":"合成研究方法",
        "parentPolicyRef":null,"modelConfigRef":Uuid::from_u128(10),
        "defaults":{"commentBudget":100,"contextCharacterBudget":6000},
        "stageInstructions":{"semantic":"保留用户原意","resolution":"","pair":""}
    })).unwrap()
}
fn model() -> StudyModelSnapshot {
    StudyModelSnapshot { model_config_ref:Uuid::from_u128(10),
        identity:StudyModelIdentity { model_ref:Uuid::from_u128(11),
            connection_version_ref:Uuid::from_u128(12),model_id:"synthetic-model".into() },
        input_token_limit:8192,output_token_limit:1024,timeout_seconds:60 }
}
fn request_value() -> Value {
    json!({"domainRef":crate::comment_study_source::ADHD_DOMAIN_REF,"methodName":"合成",
        "modelConfigRef":Uuid::from_u128(10),"parentPolicyRef":null,
        "defaults":{"commentBudget":100,"contextCharacterBudget":6000},
        "stageInstructions":{"semantic":"","resolution":"","pair":""}})
}

#[test]
fn client_cannot_supply_hash_schema_origin_or_parameters() {
    for key in ["methodHash","methodManifest","outputSchema","origin","temperature"] {
        let mut value=request_value(); value[key]=json!("forged");
        assert!(serde_json::from_value::<CreateStudyPolicyCommand>(value).is_err(),"{key}");
    }
    for (section,key) in [("defaults","tokenLimit"),("stageInstructions","systemInstruction")] {
        let mut value=request_value(); value[section][key]=json!("forged");
        assert!(serde_json::from_value::<CreateStudyPolicyCommand>(value).is_err());
    }
}

#[test]
fn all_three_stage_instructions_and_both_defaults_are_required() {
    for (section,key) in [("stageInstructions","semantic"),("stageInstructions","resolution"),
        ("stageInstructions","pair"),("defaults","commentBudget"),("defaults","contextCharacterBudget")] {
        let mut value=request_value();value[section].as_object_mut().unwrap().remove(key);
        assert!(serde_json::from_value::<CreateStudyPolicyCommand>(value).is_err());
    }
    let mut value=request_value();value.as_object_mut().unwrap().remove("parentPolicyRef");
    assert!(serde_json::from_value::<CreateStudyPolicyCommand>(value).unwrap().validate().is_ok());
}

#[test]
fn unicode_bounds_keep_whitespace_and_do_not_measure_utf16_units() {
    let mut request=command();request.method_name=format!(" {} ","中".repeat(100));
    request.stage_instructions.semantic="😀".repeat(8000);
    assert!(request.validate().is_ok());
    request.stage_instructions.semantic.push('😀');assert!(request.validate().is_err());
    request.stage_instructions.semantic="  原文\n".into();request.method_name=" ".into();
    assert!(request.validate().is_err());request.method_name="合成".into();
    let built=compile_study_method(&request,&model()).unwrap();
    assert!(built.manifest.stages.semantic.system_instruction.contains("\n  原文\n\n</stage-instructions>"));
    request.stage_instructions.pair="a\0b".into();assert!(request.validate().is_err());
}

#[test]
fn create_request_enforces_existing_domain_id_and_budget_boundaries() {
    for value in [0,3001] { let mut c=command();c.defaults.comment_budget=value;assert!(c.validate().is_err()); }
    for value in [0,20001] { let mut c=command();c.defaults.context_character_budget=value;assert!(c.validate().is_err()); }
    let mut c=command();c.domain_ref=Uuid::nil();
    assert_eq!(c.validate(),Err(StudyPolicyContractError::UnsupportedDomain));
    c=command();c.parent_policy_ref=Some(Uuid::nil());assert!(c.validate().is_err());
    c=command();c.model_config_ref=Uuid::nil();assert!(c.validate().is_err());
}

#[test]
fn model_snapshot_must_match_requested_config_and_real_parameter_bounds() {
    let mut m=model();m.model_config_ref=Uuid::from_u128(99);
    assert_eq!(compile_study_method(&command(),&m).unwrap_err(),StudyPolicyContractError::InvalidModelSnapshot);
    for (output,timeout) in [(0,60),(8193,60),(128,0),(128,61)] {
        m=model();m.output_token_limit=output;m.timeout_seconds=timeout;
        assert!(compile_study_method(&command(),&m).is_err());
    }
    m=model();m.identity.model_id="\0".into();assert!(compile_study_method(&command(),&m).is_err());
}

#[test]
fn schemas_match_all_approved_fixtures_and_fit_adapter_cap() {
    for (stage,source) in [
        (StudyStage::Semantic,include_str!("../../../../docs/data-contracts/comment-study-productization-001/semantic-output.schema.json")),
        (StudyStage::Resolution,include_str!("../../../../docs/data-contracts/comment-study-productization-001/resolution-output.schema.json")),
        (StudyStage::Pair,include_str!("../../../../docs/data-contracts/comment-study-productization-001/pair-output.schema.json"))] {
        let schema=study_output_schema(stage).unwrap();
        assert_eq!(schema,serde_json::from_str::<Value>(source).unwrap());
        assert!(serde_json::to_vec(&schema).unwrap().len()<=6144);
    }
}

#[test]
fn every_schema_object_is_closed_and_declares_all_its_fields() {
    fn check(value:&Value) {
        if value["type"]=="object" || value["type"].as_array().is_some_and(|a|a.contains(&json!("object"))) {
            assert_eq!(value["additionalProperties"],false);
            let props=value["properties"].as_object().unwrap();
            let keys:std::collections::BTreeSet<_>=props.keys().map(String::as_str).collect();
            let required:std::collections::BTreeSet<_>=value["required"].as_array().unwrap().iter().map(|v|v.as_str().unwrap()).collect();
            assert_eq!(keys,required);
        }
        match value { Value::Object(m)=>{for v in m.values(){check(v)}},
            Value::Array(a)=>{for v in a{check(v)}},_=>{} }
    }
    for s in [StudyStage::Semantic,StudyStage::Resolution,StudyStage::Pair]{check(&study_output_schema(s).unwrap());}
}

#[test]
fn nullable_reason_frame_and_stable_identity_are_not_freeform_objects() {
    let s=study_output_schema(StudyStage::Semantic).unwrap();
    let p=&s["properties"]["results"]["items"]["properties"];
    assert_eq!(p["reason"]["type"],json!(["string","null"]));
    let frame=&p["signals"]["items"]["properties"]["problemFrame"];
    assert_eq!(frame["type"],json!(["object","null"]));
    assert_eq!(frame["properties"]["actor"]["required"],json!(["value","basis"]));
    let pair=study_output_schema(StudyStage::Pair).unwrap();
    let proposal=&pair["properties"]["proposedProblem"];
    assert_eq!(proposal["type"],json!(["object","null"]));
    assert_eq!(proposal["properties"]["stableIdentity"]["properties"]["actor"]["type"],json!(["string","null"]));
}

#[test]
fn method_is_deterministic_and_has_no_fabricated_temperature_or_budget_fields() {
    let a=compile_study_method(&command(),&model()).unwrap();
    let b=compile_study_method(&command(),&model()).unwrap();assert_eq!(a,b);
    verify_study_method(&a,&model()).unwrap();
    assert_eq!(a.method_hash.len(),64);assert!(a.method_hash.bytes().all(|c|c.is_ascii_hexdigit()&&!c.is_ascii_uppercase()));
    let v=serde_json::to_value(a.manifest).unwrap();
    for field in ["temperature","tokenLimit","policyRef","createdAt","methodName"]{assert!(v.get(field).is_none());}
}

#[test]
fn display_name_parent_and_default_budgets_do_not_change_method_hash() {
    let a=compile_study_method(&command(),&model()).unwrap();
    let mut c=command();c.method_name="另一个名称".into();c.parent_policy_ref=Some(Uuid::from_u128(123));
    c.defaults.comment_budget=300;c.defaults.context_character_budget=20000;
    let b=compile_study_method(&c,&model()).unwrap();assert_eq!(a,b);
}

#[test]
fn editing_one_stage_changes_only_its_stage_hash_and_the_method_hash() {
    let a=compile_study_method(&command(),&model()).unwrap();
    let mut c=command();c.stage_instructions.resolution="补充研究视角".into();
    let b=compile_study_method(&c,&model()).unwrap();
    assert_ne!(a.method_hash,b.method_hash);
    assert_eq!(a.manifest.stages.semantic.stage_hash,b.manifest.stages.semantic.stage_hash);
    assert_ne!(a.manifest.stages.resolution.stage_hash,b.manifest.stages.resolution.stage_hash);
    assert_eq!(a.manifest.stages.pair.stage_hash,b.manifest.stages.pair.stage_hash);
}

#[test]
fn real_model_parameters_and_identity_participate_in_every_stage_hash() {
    let a=compile_study_method(&command(),&model()).unwrap();
    for change in 0..5 {
        let mut m=model();match change {0=>m.output_token_limit+=1,1=>m.timeout_seconds-=1,
            2=>m.input_token_limit+=1,3=>m.identity.connection_version_ref=Uuid::from_u128(77),
            _=>m.identity.model_id="synthetic-model-2".into()}
        let b=compile_study_method(&command(),&m).unwrap();
        assert_ne!(a.manifest.stages.semantic.stage_hash,b.manifest.stages.semantic.stage_hash);
        assert_ne!(a.manifest.stages.resolution.stage_hash,b.manifest.stages.resolution.stage_hash);
        assert_ne!(a.manifest.stages.pair.stage_hash,b.manifest.stages.pair.stage_hash);
    }
}

#[test]
fn corrupt_saved_content_or_wrong_model_is_rejected_not_silently_rebuilt() {
    let a=compile_study_method(&command(),&model()).unwrap();
    let mut changed=a.clone();changed.manifest.stages.semantic.system_instruction.push('!');
    assert_eq!(verify_study_method(&changed,&model()),Err(StudyPolicyContractError::IntegrityMismatch));
    let mut wrong=model();wrong.output_token_limit+=1;
    assert_eq!(verify_study_method(&a,&wrong),Err(StudyPolicyContractError::IntegrityMismatch));
    changed=a.clone();changed.manifest.stages.pair.output_schema=json!({});
    assert!(verify_study_method(&changed,&model()).is_err());
    changed=a.clone();changed.method_hash="0".repeat(64);assert!(verify_study_method(&changed,&model()).is_err());
    assert_eq!(a,compile_study_method(&command(),&model()).unwrap());
}

#[test]
fn unsupported_and_unrecorded_methods_do_not_acquire_current_defaults() {
    assert!(serde_json::from_value::<StudyMethodManifest>(json!({})).is_err());
    let mut a=compile_study_method(&command(),&model()).unwrap();
    a.manifest.builder_revision="unknown-builder".into();
    assert_eq!(verify_study_method(&a,&model()),Err(StudyPolicyContractError::UnsupportedRevision));
}

#[test]
fn canonical_json_has_explicit_utf8_order_and_preserves_unicode_text() {
    let v=json!({"z":[2,1],"a":{"é":"  原声😀\n","a":null},"\u{e000}":1,"\u{10000}":2});
    let bytes=canonical_json_v1(&v).unwrap();
    assert_eq!(String::from_utf8(bytes).unwrap(),"{\"a\":{\"a\":null,\"é\":\"  原声😀\\n\"},\"z\":[2,1],\"\u{e000}\":1,\"\u{10000}\":2}");
    // Fixed Python-generated golden, not an expected value produced by this Rust implementation.
    assert_eq!(json_hash(&v).unwrap(),"bfb6b70426a9abd4ca373cc2bb00b5dc8da9fa68bf9ec5bbec29f7fe5dc3a3e5");
    assert_ne!(json_hash(&json!([1,2])).unwrap(),json_hash(&json!([2,1])).unwrap());
    assert_ne!(json_hash(&json!("é")).unwrap(),json_hash(&json!("e\u{301}")).unwrap());
}

#[test]
fn canonical_json_rejects_floats_and_excessive_nesting_but_accepts_all_integer_extremes() {
    for v in [json!(1.0),json!({"nested":[1.5]}),json!(-0.0)]{assert!(canonical_json_v1(&v).is_err());}
    for v in [json!(i64::MIN),json!(u64::MAX)]{assert!(canonical_json_v1(&v).is_ok());}
    let mut v=json!(0);for _ in 0..66{v=json!([v]);}assert!(canonical_json_v1(&v).is_err());
}

#[test]
fn existing_worker_system_text_is_byte_preserved() {
    for (s,digest) in [
        (StudyStage::Semantic,"638878036ea105570bda2f40f4489cee58d41c433e10f191123cc78c667d7e81"),
        (StudyStage::Resolution,"a72c28f8813274395fede820bf996929a2891294e9963b1d6e7253094b7f4f05"),
        (StudyStage::Pair,"22533acaea8695e988b43336589c42293c94f070fd9966b302aabf4fde3ea8f2")] {
        let actual:String=Sha256::digest(base_instruction(s).as_bytes()).iter().map(|b|format!("{b:02x}")).collect();
        assert_eq!(actual,digest);
    }
}
