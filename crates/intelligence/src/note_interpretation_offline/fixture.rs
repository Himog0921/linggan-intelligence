//! Handwritten synthetic examples, never loaded from the corpus, environment or provider.
use super::*;
use linggan_evidence::note_material_input::ProcessingScope;

const AS_OF: &str = "2026-10-20T00:00:00Z";
const NAMES: [&str; 23] = [
    "拖延与启动",
    "写作业",
    "走神",
    "情绪起伏",
    "丢三落四",
    "读写困难",
    "考试压力",
    "学校与老师",
    "同伴社交",
    "亲子冲突",
    "睡眠作息",
    "手机游戏",
    "收纳自理",
    "确诊与自我识别",
    "用药经历",
    "训练经历",
    "优势与天赋",
    "被误解",
    "工作生活",
    "旧版待移除方向",
    "启动与作业",
    "时间安排",
    "生活转换",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Scenario {
    Baseline,
    Candidate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewRequest {
    pub scenario: Scenario,
    pub judgments: Vec<ManualJudgment>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteView {
    pub title: String,
    pub likes: Option<u64>,
    pub published_week: Option<String>,
    pub input: InputPreview,
    pub baseline: Output,
    pub result: ReviewedOutput,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub contract: &'static str,
    pub origin: Origin,
    pub scenario: Scenario,
    pub sample: FrozenSample,
    pub catalogue: CatalogueDraft,
    pub baseline_catalogue: CatalogueDraft,
    pub candidate_catalogue: CatalogueDraft,
    pub comparison: Vec<StructureChange>,
    pub notes: Vec<NoteView>,
    pub summary: Summary,
}

pub fn direction(index: usize) -> Uuid {
    Uuid::from_u128(1000 + index as u128)
}
pub fn work(index: usize) -> Uuid {
    Uuid::from_u128(100 + index as u128)
}

fn catalogue(scenario: Scenario) -> CatalogueDraft {
    let indices: Vec<_> = match scenario {
        Scenario::Baseline => (0..20).collect(),
        Scenario::Candidate => (2..19).chain(20..23).collect(),
    };
    CatalogueDraft {
        origin: Origin::OfflineMock,
        version_ref: Uuid::from_u128(if scenario == Scenario::Baseline {
            10
        } else {
            11
        }),
        groups: vec![
            DirectionGroup {
                group_ref: Uuid::from_u128(30),
                name: "合成：学习与生活".into(),
            },
            DirectionGroup {
                group_ref: Uuid::from_u128(31),
                name: "合成：关系与自我".into(),
            },
        ],
        directions: indices
            .into_iter()
            .map(|i| DirectionDraft {
                direction_ref: direction(i),
                group_ref: Uuid::from_u128(if i % 2 == 0 { 30 } else { 31 }),
                name: NAMES[i].into(),
                definition: format!(
                    "合成结构示例：围绕“{}”描述困扰或经历，不是正式方向定义。",
                    NAMES[i]
                ),
                example_work_refs: vec![work(i % 3), work((i + 1) % 3)],
            })
            .collect(),
        sample_work_refs: (0..6).map(work).collect(),
    }
}

fn material(i: usize) -> Material {
    let title = [
        "合成01：作业开始前",
        "合成02：任务中走神",
        "合成03：课堂注意力",
        "合成04：暂不适配",
        "合成05：无关内容",
        "合成06：尚不能判断",
    ][i];
    let body = "作业开始前总是拖一会儿。仅为合成内容，没有真实人物或模型分析。";
    let fragments: Vec<_> = [("title", title), ("body", body)]
        .into_iter()
        .enumerate()
        .map(|(n, (field, text))| Fragment {
            citation_id: format!("synthetic-{i}-{n}"),
            field: field.into(),
            logical_position: "native".into(),
            source_ref: Uuid::from_u128(200 + i as u128),
            source_version: "synthetic.v1".into(),
            start: 0,
            end: text.chars().count(),
            text_hash: format!("synthetic-text-{i}-{n}"),
            text: text.into(),
        })
        .collect();
    Material {
        contract: note_material_input::VERSION.into(),
        fingerprint_contract: note_material_input::FINGERPRINT_VERSION.into(),
        domain_ref: Uuid::from_u128(1),
        work_ref: work(i),
        as_of: AS_OF.into(),
        dossier_revision: format!("synthetic-dossier-{i}"),
        material_fingerprint: format!("synthetic-material-{i}"),
        provenance_hash: format!("synthetic-provenance-{i}"),
        snapshot_identity: format!("synthetic-snapshot-{i}"),
        ready: true,
        not_ready_reasons: vec![],
        columns: INPUT_FIELDS
            .into_iter()
            .map(|field| Column {
                field: field.into(),
                state: if matches!(field, "title" | "body") {
                    ColumnState::Available
                } else if field == "ocr" {
                    ColumnState::TerminalFailure
                } else {
                    ColumnState::NotProvided
                },
                reasons: if field == "ocr" {
                    vec!["synthetic_terminal_example".into()]
                } else {
                    vec![]
                },
            })
            .collect(),
        fragments,
        layer_version: "synthetic-layer.v1".into(),
        processing_scope: ProcessingScope {
            included: vec![],
            context_only: vec![],
            not_connected: vec![],
        },
    }
}

fn inputs(scenario: Scenario) -> Result<(CatalogueDraft, Vec<Input>), Invalid> {
    let catalogue = catalogue(scenario);
    let skill = Uuid::from_u128(if scenario == Scenario::Baseline {
        20
    } else {
        21
    });
    let inputs = (0..6)
        .map(|i| Input::from_catalogue_draft(material(i), skill, &catalogue))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((catalogue, inputs))
}

fn machine(input: &Input, i: usize, scenario: Scenario) -> Result<Output, Invalid> {
    let mut output = mock_unknown(input)?;
    output.reason = Some("固定合成示例；没有调用模型，不证明分类质量。".into());
    output.relevance = match i {
        4 => Relevance::NotRelated,
        5 => Relevance::Unknown,
        _ => Relevance::Related,
    };
    if i < 3 {
        output.primary_direction = Some(direction(if i == 0 {
            if scenario == Scenario::Baseline {
                0
            } else {
                20
            }
        } else {
            2
        }));
        output.also_related = Some(direction(3));
        output.confidence = if i == 1 {
            Confidence::Possible
        } else {
            Confidence::Certain
        };
        output.perspective = [
            Perspective::ParentLivedExperience,
            Perspective::AdultSelfReport,
            Perspective::ProfessionalEducation,
        ][i];
        output.keywords = vec![Keyword {
            text: "作业".into(),
            citation_id: format!("synthetic-{i}-1"),
            start: 0,
            end: 2,
        }];
    }
    validate_output(input, &output)?;
    Ok(output)
}

fn changes() -> Vec<StructureChange> {
    let mut changes: Vec<_> = (2..19)
        .map(|i| StructureChange {
            kind: ChangeKind::Keep,
            old_refs: vec![direction(i)],
            new_refs: vec![direction(i)],
        })
        .collect();
    changes.extend([
        StructureChange {
            kind: ChangeKind::Merge,
            old_refs: vec![direction(0), direction(1)],
            new_refs: vec![direction(20)],
        },
        StructureChange {
            kind: ChangeKind::Delete,
            old_refs: vec![direction(19)],
            new_refs: vec![],
        },
        StructureChange {
            kind: ChangeKind::New,
            old_refs: vec![],
            new_refs: vec![direction(21)],
        },
        StructureChange {
            kind: ChangeKind::New,
            old_refs: vec![],
            new_refs: vec![direction(22)],
        },
    ]);
    changes
}

pub fn preview(request: &PreviewRequest) -> Result<Preview, Invalid> {
    if request.judgments.len() > 6 {
        return Err(Invalid("offline_too_many_judgments"));
    }
    let (baseline_catalogue, baseline) = inputs(Scenario::Baseline)?;
    let (candidate_catalogue, candidate) = inputs(Scenario::Candidate)?;
    let current = if request.scenario == Scenario::Baseline {
        &baseline
    } else {
        &candidate
    };
    let catalogue = if request.scenario == Scenario::Baseline {
        baseline_catalogue.clone()
    } else {
        candidate_catalogue.clone()
    };
    let sample = freeze_sample("handwritten-synthetic-20261020.v1", current)?;
    catalogue.validate_sample(&sample)?;
    let comparison = changes();
    validate_comparison(&baseline_catalogue, &candidate_catalogue, &comparison)?;
    let mut seen = BTreeSet::new();
    for judgment in &request.judgments {
        if !seen.insert(judgment.identity.work_ref) {
            return Err(Invalid("offline_duplicate_judgment"));
        }
        let original = baseline
            .iter()
            .chain(candidate.iter())
            .find(|input| input.identity() == judgment.identity)
            .ok_or(Invalid("offline_judgment_not_in_fixture"))?;
        validate_judgment(original, judgment)?;
    }
    let mut notes = vec![];
    for (i, input) in current.iter().enumerate() {
        let judgment = request
            .judgments
            .iter()
            .find(|j| j.identity.work_ref == input.material.work_ref);
        let original = judgment.and_then(|j| {
            baseline
                .iter()
                .chain(candidate.iter())
                .find(|input| input.identity() == j.identity)
                .map(|input| (input, j))
        });
        let result = project_judgment(input, &machine(input, i, request.scenario)?, original)?;
        notes.push(NoteView {
            title: input.material.fragments[0].text.clone(),
            likes: [Some(6200), Some(1000), None, Some(0), Some(5100), None][i],
            published_week: [
                Some("2026-W40"),
                Some("2026-W40"),
                None,
                Some("2026-W41"),
                Some("2026-W41"),
                None,
            ][i]
                .map(str::to_owned),
            input: prepare_input(input)?,
            baseline: machine(&baseline[i], i, Scenario::Baseline)?,
            result,
        });
    }
    let stats: Vec<_> = notes
        .iter()
        .zip(current)
        .map(|(note, input)| NoteStatistics {
            input,
            output: &note.result.effective,
            likes: note.likes,
            published_week: note.published_week.as_deref(),
        })
        .collect();
    let mut summary = summarize(&stats)?;
    for direction in &catalogue.directions {
        if !summary
            .directions
            .iter()
            .any(|row| row.direction_ref == direction.direction_ref)
        {
            summary
                .directions
                .push(super::statistics::DirectionStatistics {
                    direction_ref: direction.direction_ref,
                    ..Default::default()
                });
        }
    }
    summary.directions.sort_by_key(|row| row.direction_ref);
    notes.sort_by(|a, b| {
        b.likes
            .cmp(&a.likes)
            .then(a.input.identity.work_ref.cmp(&b.input.identity.work_ref))
    });
    Ok(Preview {
        contract: "note-interpretation-offline-preview.v1",
        origin: Origin::OfflineMock,
        scenario: request.scenario,
        sample,
        catalogue,
        baseline_catalogue,
        candidate_catalogue,
        comparison,
        notes,
        summary,
    })
}
