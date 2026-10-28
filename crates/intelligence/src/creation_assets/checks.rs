//! Three-level manuscript checks and the bridge sentence entry checks (spec appendix A).
//! Pure functions over a snapshot; the one implementation of every rule lives here.
//!
//! Only structural rules about the account and the bridge slot can block copying (必改).
//! Rules driven by word lists remind (提醒): the lists have not been calibrated against real
//! notes, so they must not be able to stop a note from leaving.
use super::copy_format::{Block, blocks, tag_key};
use super::subject::{CardTerms, Context, Subject};
use super::text::*;
use super::types::*;
use super::words::*;
use uuid::Uuid;

const GROUP_CLAIMS: &[&[Tok]] = &[
    &[
        Tok::Lit("A"),
        Tok::Gap,
        Tok::Lit("娃"),
        Tok::Gap,
        Tok::Lit("都"),
    ],
    &[
        Tok::Lit("所有"),
        Tok::Opt("的"),
        Tok::Gap,
        Tok::Lit("A"),
        Tok::Gap,
        Tok::Lit("娃"),
    ],
    &[
        Tok::Lit("ADHD"),
        Tok::Gap,
        Tok::Opt("的"),
        Tok::Gap,
        Tok::Lit("孩子"),
        Tok::Gap,
        Tok::Lit("都"),
    ],
];
const A_WA: &[&[Tok]] = &[&[Tok::Lit("a"), Tok::Gap, Tok::Lit("娃")]];

const RULE_ORDER: &[&str] = &[
    "R-M1", "R-M2", "R-M3", "R-M4", "R-W10", "R-W11", "R-W1", "R-W2", "R-W3", "R-W6", "R-W7",
    "R-W8", "R-W9", "R-I1", "R-I2", "R-L1", "R-L2", "R-L3", "R-L4", "R-L5", "R-L6", "R-L7",
];

fn rank(rule: &str) -> usize {
    RULE_ORDER
        .iter()
        .position(|known| *known == rule)
        .unwrap_or(usize::MAX)
}

/// Shows at most 20 characters of a match in a message.
fn shown(text: &str) -> String {
    let mut chars = text.chars();
    let head: String = chars.by_ref().take(20).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

fn fix(label: &str, action: FixAction) -> Fix {
    Fix {
        label: label.into(),
        action,
    }
}

fn finding(
    rule: &str,
    level: Level,
    message: String,
    location: Location,
    fixes: Vec<Fix>,
) -> Finding {
    Finding {
        rule: rule.into(),
        level,
        message,
        location,
        fixes,
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Title,
    Cover,
    Paragraph(Uuid),
    Bridge,
}
struct Field<'a> {
    kind: Kind,
    text: &'a str,
}
impl Field<'_> {
    fn headline(&self) -> bool {
        matches!(self.kind, Kind::Title | Kind::Cover)
    }
    fn locate(&self, range: (usize, usize)) -> Location {
        let (start, end) = utf16_range(self.text, range);
        match self.kind {
            Kind::Title => Location::Title { start, end },
            Kind::Cover => Location::Cover { start, end },
            Kind::Paragraph(id) => Location::Paragraph { id, start, end },
            Kind::Bridge => Location::Bridge { start, end },
        }
    }
}

fn fields<'a>(subject: &'a Subject, bridge: Option<(&'a str, Option<Uuid>)>) -> Vec<Field<'a>> {
    let mut out = vec![
        Field {
            kind: Kind::Title,
            text: &subject.title,
        },
        Field {
            kind: Kind::Cover,
            text: &subject.cover_title,
        },
    ];
    for block in blocks(&subject.paragraphs, bridge) {
        out.push(match block {
            Block::Paragraph(id, text) => Field {
                kind: Kind::Paragraph(id),
                text,
            },
            Block::Bridge(text) => Field {
                kind: Kind::Bridge,
                text,
            },
        });
    }
    out
}

/// Spellings of "A娃" and "ADHD" other than the agreed ones, with the agreed replacement.
/// Hashtags are left alone.
fn writing_variants(text: &str) -> Vec<((usize, usize), &'static str)> {
    let mut out = Vec::new();
    for range in find_patterns(text, A_WA) {
        let before = text[..range.0].chars().next_back();
        if &text[range.0..range.1] != "A娃"
            && !before.is_some_and(is_ascii_word)
            && !inside_hashtag(text, range.0)
        {
            out.push((range, "A娃"));
        }
    }
    let lower = text.to_ascii_lowercase();
    for (start, hit) in lower.match_indices("adhd") {
        let end = start + hit.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        if &text[start..end] != "ADHD"
            && !before.is_some_and(is_ascii_word)
            && !after.is_some_and(is_ascii_word)
            && !inside_hashtag(text, start)
        {
            out.push(((start, end), "ADHD"));
        }
    }
    out.sort();
    out
}

/// Matches of banned spellings that are not just part of an allowed name.
fn banned_hits(text: &str, cards: &CardTerms) -> Vec<(usize, usize)> {
    let allowed = find_any_owned(text, &cards.allowed);
    find_any_owned(text, &cards.banned)
        .into_iter()
        .filter(|hit| !allowed.iter().any(|a| a.0 <= hit.0 && hit.1 <= a.1))
        .collect()
}

fn bio_suggestion(identity: &Identity, cards: Option<&CardTerms>) -> String {
    let name = cards
        .map(|c| c.canonical.as_str())
        .filter(|n| !n.is_empty())
        .unwrap_or("陪跑营");
    match identity.org_relation {
        OrgRelation::Parent => format!("我是{name}的学员家长"),
        OrgRelation::Coach => format!("我是{name}的陪跑老师"),
        _ => String::new(),
    }
}

fn bridge_slot_findings(subject: &Subject, ctx: &Context, state: BridgeState) -> Vec<Finding> {
    let skip = fix("这篇不植入", FixAction::SkipBridge);
    match state {
        BridgeState::NoAccount => vec![finding(
            "R-M1",
            Level::Must,
            "这篇还没选账号，不知道能不能放衔接句".into(),
            Location::Slot,
            vec![fix("选择账号", FixAction::ChooseAccount), skip],
        )],
        BridgeState::Unconfirmed => vec![finding(
            "R-M1",
            Level::Must,
            "账号身份还没确认，不能放衔接句".into(),
            Location::Slot,
            vec![fix("去确认身份", FixAction::OpenIdentity), skip],
        )],
        BridgeState::BioMissing => {
            let suggestion = ctx
                .identity
                .as_ref()
                .map(|i| bio_suggestion(i, ctx.cards.as_ref()))
                .unwrap_or_default();
            vec![finding(
                "R-M2",
                Level::Must,
                "账号简介还没写明和机构的关系，不能放衔接句".into(),
                Location::Slot,
                vec![
                    fix(
                        "复制建议简介",
                        FixAction::CopyBioSuggestion { text: suggestion },
                    ),
                    fix("我已改好简介", FixAction::ConfirmBioWritten),
                    skip,
                ],
            )]
        }
        BridgeState::Ready => {
            let group = ctx.identity.as_ref().and_then(|i| i.org_relation.group());
            match subject.bridge.as_ref().and_then(|b| b.sentence.as_ref()) {
                None => vec![finding(
                    "R-M4",
                    Level::Must,
                    "还没选衔接句".into(),
                    Location::Slot,
                    vec![fix("自动选一句", FixAction::AutoPickBridge), skip],
                )],
                Some(used) if Some(used.group) != group => vec![finding(
                    "R-M3",
                    Level::Must,
                    "选中的衔接句是另一种身份的口吻，和这个账号对不上".into(),
                    Location::Slot,
                    vec![fix("换成对应的一句", FixAction::SwapBridge)],
                )],
                Some(_) => vec![],
            }
        }
        BridgeState::NoRelation | BridgeState::Skipped => vec![],
    }
}

/// Cure promises (R-W10 in the body, R-W11 in the title and cover), effect claims (R-W1) and
/// conclusions about every child (R-W2). A cure promise hides the effect claim it contains.
fn phrase_findings(field: &Field<'_>) -> Vec<Finding> {
    let mut out = Vec::new();
    let cure = find_any(field.text, CURE_PROMISES);
    for &range in &cure {
        let said = shown(&field.text[range.0..range.1]);
        out.push(if field.headline() {
            finding(
                "R-W11",
                Level::Warn,
                format!("“{said}”是疗效承诺，建议换一个标题"),
                field.locate(range),
                vec![fix("换一个标题", FixAction::EditByHand)],
            )
        } else {
            finding(
                "R-W10",
                Level::Warn,
                format!("“{said}”是疗效承诺，建议删掉或改写"),
                field.locate(range),
                vec![
                    fix("删掉这几个字", FixAction::DeleteMatch),
                    fix("手动改写", FixAction::EditByHand),
                ],
            )
        });
    }
    for range in find_any(field.text, EFFECT_CLAIMS) {
        if cure.iter().any(|c| overlaps(*c, range)) {
            continue;
        }
        out.push(finding(
            "R-W1",
            Level::Warn,
            format!(
                "“{}”是在讲效果，建议只写学到了什么、做了什么",
                shown(&field.text[range.0..range.1])
            ),
            field.locate(range),
            vec![fix("改成只讲学到了什么、做了什么", FixAction::EditByHand)],
        ));
    }
    for range in find_patterns(field.text, GROUP_CLAIMS) {
        out.push(finding(
            "R-W2",
            Level::Warn,
            format!(
                "“{}”是在对所有孩子下结论，建议只说自己的孩子，或写成“有的孩子”",
                shown(&field.text[range.0..range.1])
            ),
            field.locate(range),
            vec![fix("改成只说自己的孩子", FixAction::EditByHand)],
        ));
    }
    out
}

/// Product names that are not the organisation's wording (R-W3).
fn name_findings(field: &Field<'_>, cards: &CardTerms) -> Vec<Finding> {
    banned_hits(field.text, cards)
        .into_iter()
        .map(|range| {
            finding(
                "R-W3",
                Level::Warn,
                format!(
                    "“{}”不是产品的可用叫法，建议统一为“{}”",
                    shown(&field.text[range.0..range.1]),
                    cards.canonical
                ),
                field.locate(range),
                vec![fix(
                    "统一叫法",
                    FixAction::ReplaceText {
                        replacement: cards.canonical.clone(),
                    },
                )],
            )
        })
        .collect()
}

/// "A娃" and "ADHD" written in more than one way (R-W6).
fn variant_findings(field: &Field<'_>) -> Vec<Finding> {
    writing_variants(field.text)
        .into_iter()
        .map(|(range, replacement)| {
            finding(
                "R-W6",
                Level::Warn,
                format!(
                    "“{}”写法不统一，建议统一为“{replacement}”",
                    &field.text[range.0..range.1]
                ),
                field.locate(range),
                vec![fix(
                    "一键统一",
                    FixAction::ReplaceText {
                        replacement: replacement.into(),
                    },
                )],
            )
        })
        .collect()
}

fn content_findings(fields: &[Field<'_>], cards: Option<&CardTerms>) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut mention: Option<Location> = None;
    for field in fields {
        out.extend(phrase_findings(field));
        if let Some(cards) = cards {
            out.extend(name_findings(field, cards));
            if mention.is_none() {
                mention = find_any_owned(field.text, &cards.allowed)
                    .first()
                    .map(|range| field.locate(*range));
            }
        }
        out.extend(variant_findings(field));
    }
    if let Some(location) = mention {
        out.push(finding(
            "R-I2",
            Level::Hint,
            "正文提到了产品：发布前确认账号简介里仍然写明身份".into(),
            location,
            vec![],
        ));
    }
    out
}

fn shape_findings(subject: &Subject) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (index, tag) in subject.tags.iter().enumerate() {
        let key = tag_key(tag);
        if !key.is_empty() && !seen.insert(key) {
            out.push(finding(
                "R-W7",
                Level::Warn,
                format!("话题“{}”重复了，复制时会自动去掉", shown(tag.trim())),
                Location::Tag { index },
                vec![fix("删掉重复的", FixAction::RemoveTag)],
            ));
        }
    }
    let cover = char_count(&subject.cover_title);
    if cover > MAX_COVER_CHARS {
        out.push(finding(
            "R-W8",
            Level::Warn,
            format!("封面大字有 {cover} 个字，建议不超过 {MAX_COVER_CHARS} 个字"),
            Location::Cover {
                start: 0,
                end: subject.cover_title.encode_utf16().count(),
            },
            vec![fix("手动改短", FixAction::EditByHand)],
        ));
    }
    let title = char_count(&subject.title);
    if title > MAX_TITLE_CHARS {
        out.push(finding(
            "R-W9",
            Level::Warn,
            format!("标题有 {title} 个字，建议不超过 {MAX_TITLE_CHARS} 个字"),
            Location::Title {
                start: 0,
                end: subject.title.encode_utf16().count(),
            },
            vec![fix("手动改短", FixAction::EditByHand)],
        ));
    }
    if subject.cover_ai == Some(true) {
        out.push(finding(
            "R-I1",
            Level::Hint,
            "封面是 AI 作图：发布时记得勾选平台的 AI 声明".into(),
            Location::Slot,
            vec![],
        ));
    }
    out
}

fn finish(mut findings: Vec<Finding>) -> Vec<Finding> {
    findings.sort_by_key(|f| (f.level, rank(&f.rule)));
    let mut count: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    findings.retain(|f| {
        let seen = count.entry(f.rule.clone()).or_default();
        *seen += 1;
        *seen <= MAX_FINDINGS_PER_RULE
    });
    findings
}

fn name_rules_not_run(cards: Option<&CardTerms>, rules: &[(&str, &str)]) -> Vec<NotRun> {
    if cards.is_some() {
        return vec![];
    }
    rules
        .iter()
        .map(|(rule, what)| NotRun {
            rule: (*rule).into(),
            reason: format!("产品事实卡还没有确认，{what}没有检查"),
        })
        .collect()
}

/// Runs every manuscript rule. A note may be copied when no finding is at [`Level::Must`].
pub fn check_manuscript(subject: &Subject, ctx: &Context) -> CheckReport {
    let state = ctx.bridge_state(subject);
    let cards = ctx.cards.as_ref();
    let mut findings = bridge_slot_findings(subject, ctx, state);
    findings.extend(content_findings(
        &fields(subject, ctx.bridge_output(subject)),
        cards,
    ));
    findings.extend(shape_findings(subject));
    CheckReport {
        ruleset_version: RULESET_VERSION.into(),
        bridge_state: state,
        bridge_group: ctx.identity.as_ref().and_then(|i| i.org_relation.group()),
        findings: finish(findings),
        not_run: name_rules_not_run(cards, &[("R-W3", "产品叫法"), ("R-I2", "是否提到产品")]),
        fact_cards: cards.map(|c| c.revisions.clone()).unwrap_or_default(),
    }
}

/// Entry checks for a sentence being added to the bridge library (R-L1…R-L7). Only a cure
/// promise blocks the entry; the rest inform the reviewer.
pub fn check_sentence(text: &str, cards: Option<&CardTerms>) -> (Vec<Finding>, Vec<NotRun>) {
    let whole = Location::Text {
        start: 0,
        end: text.encode_utf16().count(),
    };
    let at = |range: (usize, usize)| {
        let (start, end) = utf16_range(text, range);
        Location::Text { start, end }
    };
    let mut out = Vec::new();
    let cure = find_any(text, CURE_PROMISES);
    for &range in &cure {
        out.push(finding(
            "R-L1",
            Level::Must,
            format!(
                "“{}”是疗效承诺，衔接句里不能写，不能加入",
                shown(&text[range.0..range.1])
            ),
            at(range),
            vec![fix("手动改写", FixAction::EditByHand)],
        ));
    }
    for range in find_any(text, EFFECT_CLAIMS) {
        if !cure.iter().any(|c| overlaps(*c, range)) {
            out.push(finding(
                "R-L2",
                Level::Warn,
                format!("“{}”是在讲效果", shown(&text[range.0..range.1])),
                at(range),
                vec![fix("手动改写", FixAction::EditByHand)],
            ));
        }
    }
    for range in find_patterns(text, GROUP_CLAIMS) {
        out.push(finding(
            "R-L3",
            Level::Warn,
            format!("“{}”是在对所有孩子下结论", shown(&text[range.0..range.1])),
            at(range),
            vec![fix("手动改写", FixAction::EditByHand)],
        ));
    }
    if let Some(cards) = cards {
        for range in banned_hits(text, cards) {
            out.push(finding(
                "R-L4",
                Level::Warn,
                format!(
                    "“{}”不是产品的可用叫法，建议统一为“{}”",
                    shown(&text[range.0..range.1]),
                    cards.canonical
                ),
                at(range),
                vec![fix(
                    "统一叫法",
                    FixAction::ReplaceText {
                        replacement: cards.canonical.clone(),
                    },
                )],
            ));
        }
        if find_any_owned(text, &cards.allowed).is_empty() {
            out.push(finding(
                "R-L5",
                Level::Hint,
                "没有提到产品名，可以这样写，但读者可能不知道在说什么".into(),
                whole.clone(),
                vec![],
            ));
        }
    }
    for range in find_any(text, SOLICITING) {
        out.push(finding(
            "R-L6",
            Level::Warn,
            format!("“{}”像是官方或招揽的口吻", shown(&text[range.0..range.1])),
            at(range),
            vec![fix("手动改写", FixAction::EditByHand)],
        ));
    }
    let length = char_count(text);
    if length > MAX_SENTENCE_CHARS {
        out.push(finding(
            "R-L7",
            Level::Warn,
            format!("这句有 {length} 个字，建议不超过 {MAX_SENTENCE_CHARS} 个字"),
            whole,
            vec![fix("手动改短", FixAction::EditByHand)],
        ));
    }
    (
        finish(out),
        name_rules_not_run(cards, &[("R-L4", "产品叫法"), ("R-L5", "是否提到产品名")]),
    )
}

pub fn blocks_entry(findings: &[Finding]) -> bool {
    findings.iter().any(|f| f.level == Level::Must)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_workbench::{BridgeChoice, BridgeGroup, BridgeUse};

    fn identity(relation: OrgRelation, bio: Option<bool>) -> Identity {
        Identity {
            account_ref: Uuid::new_v4(),
            revision: 1,
            org_relation: relation,
            bio_discloses_relation: bio,
            bio_checked_at: None,
            bio_checked_by: None,
            has_real_person: None,
            backing_person_role: None,
            can_provide_experience: None,
            saved_at: String::new(),
            saved_by: "local_machine".into(),
        }
    }
    fn cards() -> CardTerms {
        CardTerms {
            canonical: "示例营".into(),
            allowed: vec!["示例营".into(), "示例陪跑营".into()],
            banned: vec!["示例课".into(), "陪跑".into()],
            revisions: vec![CardRevision {
                code: "CP-0001".into(),
                revision: 2,
            }],
        }
    }
    fn subject(paragraphs: &[&str]) -> Subject {
        Subject {
            has_account: true,
            title: "标题".into(),
            cover_title: "封面".into(),
            paragraphs: paragraphs
                .iter()
                .map(|t| (Uuid::new_v4(), (*t).to_string()))
                .collect(),
            tags: vec![],
            bridge: None,
            cover_ai: None,
        }
    }
    fn used(group: BridgeGroup, text: &str) -> BridgeChoice {
        BridgeChoice {
            skip: false,
            sentence: Some(BridgeUse {
                sentence_code: "XJ-0001".into(),
                sentence_revision: 1,
                group,
                text: text.into(),
                edited: false,
            }),
            after_paragraph: None,
        }
    }
    fn ready_ctx() -> Context {
        Context {
            identity: Some(identity(OrgRelation::Parent, Some(true))),
            cards: Some(cards()),
        }
    }
    fn rules(report: &CheckReport) -> Vec<&str> {
        report.findings.iter().map(|f| f.rule.as_str()).collect()
    }
    fn must(report: &CheckReport) -> Vec<&str> {
        report
            .findings
            .iter()
            .filter(|f| f.level == Level::Must)
            .map(|f| f.rule.as_str())
            .collect()
    }

    #[test]
    fn a_draft_without_an_account_needs_one_or_a_skip() {
        let mut s = subject(&["正文"]);
        s.has_account = false;
        let report = check_manuscript(&s, &Context::default());
        assert_eq!(must(&report), vec!["R-M1"]);
        assert_eq!(report.bridge_state, BridgeState::NoAccount);
        assert!(matches!(
            report.findings[0].fixes[0].action,
            FixAction::ChooseAccount
        ));
        s.bridge = Some(BridgeChoice {
            skip: true,
            ..BridgeChoice::default()
        });
        let report = check_manuscript(&s, &Context::default());
        assert!(must(&report).is_empty());
        assert_eq!(report.bridge_state, BridgeState::Skipped);
    }

    #[test]
    fn identity_states_decide_which_bridge_rule_blocks() {
        let s = subject(&["正文"]);
        let cases = [
            (None, vec!["R-M1"]),
            (Some(identity(OrgRelation::Unknown, None)), vec!["R-M1"]),
            (Some(identity(OrgRelation::Parent, None)), vec!["R-M2"]),
            (
                Some(identity(OrgRelation::Coach, Some(false))),
                vec!["R-M2"],
            ),
            (
                Some(identity(OrgRelation::Parent, Some(true))),
                vec!["R-M4"],
            ),
            (Some(identity(OrgRelation::NoRelation, None)), vec![]),
        ];
        for (identity, expected) in cases {
            let ctx = Context {
                identity: identity.clone(),
                cards: None,
            };
            assert_eq!(must(&check_manuscript(&s, &ctx)), expected, "{identity:?}");
        }
    }

    #[test]
    fn the_chosen_sentence_must_match_the_account_voice() {
        let mut s = subject(&["正文"]);
        s.bridge = Some(used(BridgeGroup::Coach, "我是示例营的陪跑老师"));
        assert_eq!(must(&check_manuscript(&s, &ready_ctx())), vec!["R-M3"]);
        s.bridge = Some(used(BridgeGroup::Parent, "我是示例营的学员家长"));
        let report = check_manuscript(&s, &ready_ctx());
        assert!(must(&report).is_empty());
        assert_eq!(report.bridge_state, BridgeState::Ready);
        assert_eq!(report.bridge_group, Some(BridgeGroup::Parent));
    }

    #[test]
    fn a_skipped_or_unrelated_account_never_scans_or_blocks_on_the_bridge() {
        let mut s = subject(&["正文"]);
        s.bridge = Some(BridgeChoice {
            skip: true,
            sentence: Some(BridgeUse {
                sentence_code: "XJ-0001".into(),
                sentence_revision: 1,
                group: BridgeGroup::Parent,
                text: "保证能好".into(),
                edited: false,
            }),
            after_paragraph: None,
        });
        let report = check_manuscript(&s, &ready_ctx());
        assert!(report.findings.is_empty(), "{:?}", report.findings);
        let unrelated = Context {
            identity: Some(identity(OrgRelation::NoRelation, None)),
            cards: None,
        };
        s.bridge = Some(used(BridgeGroup::Parent, "保证能好"));
        assert!(check_manuscript(&s, &unrelated).findings.is_empty());
    }

    #[test]
    fn missing_bio_suggests_wording_from_the_confirmed_card() {
        let s = subject(&["正文"]);
        let ctx = Context {
            identity: Some(identity(OrgRelation::Coach, Some(false))),
            cards: Some(cards()),
        };
        let report = check_manuscript(&s, &ctx);
        let suggestion = report.findings[0]
            .fixes
            .iter()
            .find_map(|f| match &f.action {
                FixAction::CopyBioSuggestion { text } => Some(text.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(suggestion, "我是示例营的陪跑老师");
        let without = Context { cards: None, ..ctx };
        let report = check_manuscript(&s, &without);
        assert!(matches!(&report.findings[0].fixes[0].action,
            FixAction::CopyBioSuggestion { text } if text == "我是陪跑营的陪跑老师"));
    }

    #[test]
    fn cure_promises_remind_and_never_block_copying() {
        let mut s = subject(&["这个方法能根治拖拉", "没有问题"]);
        s.title = "三天治愈".into();
        s.bridge = Some(used(BridgeGroup::Parent, "孩子痊愈了"));
        let report = check_manuscript(&s, &ready_ctx());
        assert!(must(&report).is_empty(), "word lists must not block");
        let mut found: Vec<(&str, &Location)> = report
            .findings
            .iter()
            .map(|f| (f.rule.as_str(), &f.location))
            .collect();
        found.sort_by_key(|(rule, _)| *rule);
        assert_eq!(
            found.iter().map(|(rule, _)| *rule).collect::<Vec<_>>(),
            vec!["R-W10", "R-W10", "R-W11"]
        );
        assert!(
            found
                .iter()
                .any(|(_, l)| matches!(l, Location::Bridge { .. }))
        );
        assert!(
            found
                .iter()
                .any(|(_, l)| matches!(l, Location::Title { start: 2, end: 4 }))
        );
        assert!(report.findings.iter().all(|f| f.level == Level::Warn));
        assert_eq!(report.findings[0].rule, "R-W10");
    }

    #[test]
    fn a_cure_promise_hides_the_effect_claim_it_contains() {
        let report = check_manuscript(
            &subject(&["彻底改掉了坏习惯", "进步很大"]),
            &Context::default(),
        );
        let rules = rules(&report);
        assert_eq!(rules.iter().filter(|r| **r == "R-W10").count(), 1);
        assert_eq!(rules.iter().filter(|r| **r == "R-W1").count(), 1);
        let effect = report.findings.iter().find(|f| f.rule == "R-W1").unwrap();
        assert!(effect.message.contains("进步很大"));
    }

    #[test]
    fn effect_claims_and_group_conclusions_are_located_in_utf16_units() {
        let text = "😀孩子变好了，所有的 A娃都一样";
        let report = check_manuscript(&subject(&[text]), &Context::default());
        let effect = report.findings.iter().find(|f| f.rule == "R-W1").unwrap();
        let group = report.findings.iter().find(|f| f.rule == "R-W2").unwrap();
        let units: Vec<u16> = text.encode_utf16().collect();
        let slice = |location: &Location| match location {
            Location::Paragraph { start, end, .. } => {
                String::from_utf16(&units[*start..*end]).unwrap()
            }
            other => panic!("{other:?}"),
        };
        assert_eq!(slice(&effect.location), "变好了");
        assert_eq!(slice(&group.location), "所有的 A娃");
        for text in ["ADHD的孩子都能行", "adhd 孩子 都这样", "a娃都好"] {
            let report = check_manuscript(&subject(&[text]), &Context::default());
            assert!(rules(&report).contains(&"R-W2"), "{text}");
        }
        for text in ["A娃不都一样", "有的孩子很好", "所有孩子"] {
            let report = check_manuscript(&subject(&[text]), &Context::default());
            assert!(!rules(&report).contains(&"R-W2"), "{text}");
        }
    }

    #[test]
    fn writing_variants_are_flagged_but_hashtags_and_words_are_left_alone() {
        let report = check_manuscript(
            &subject(&[
                "a娃和 A 娃，adhd 与 Adhd，ADHD 与 A娃正常，#a娃成长 #adhd日记，Alpha娃，ADHDer",
            ]),
            &Context::default(),
        );
        let said: Vec<&str> = report
            .findings
            .iter()
            .filter(|f| f.rule == "R-W6")
            .map(|f| f.message.as_str())
            .collect();
        assert_eq!(said.len(), 4, "{said:?}");
        assert!(said[0].contains("a娃") && said[1].contains("A 娃"));
        assert!(said.iter().any(|m| m.contains("“adhd”")));
        assert!(said.iter().any(|m| m.contains("“Adhd”")));
        let first = report.findings.iter().find(|f| f.rule == "R-W6").unwrap();
        assert!(matches!(&first.fixes[0].action,
            FixAction::ReplaceText { replacement } if replacement == "A娃"));
    }

    #[test]
    fn banned_names_are_flagged_unless_they_sit_inside_an_allowed_name() {
        let ctx = Context {
            identity: None,
            cards: Some(cards()),
        };
        let mut s = subject(&["我报了示例课，也在示例陪跑营里陪跑，还有示例营"]);
        s.bridge = Some(BridgeChoice {
            skip: true,
            ..BridgeChoice::default()
        });
        let report = check_manuscript(&s, &ctx);
        let names: Vec<&str> = report
            .findings
            .iter()
            .filter(|f| f.rule == "R-W3")
            .map(|f| f.message.as_str())
            .collect();
        assert_eq!(names.len(), 2, "{names:?}");
        assert!(names[0].contains("示例课") && names[0].contains("统一为“示例营”"));
        assert!(
            names[1].contains("“陪跑”"),
            "a lone 陪跑 is not part of an allowed name"
        );
        assert!(rules(&report).contains(&"R-I2"));
        let inside = subject(&["示例陪跑营很好"]);
        let none = Context {
            identity: Some(identity(OrgRelation::NoRelation, None)),
            cards: Some(cards()),
        };
        assert!(!rules(&check_manuscript(&inside, &none)).contains(&"R-W3"));
    }

    #[test]
    fn name_rules_report_that_they_did_not_run_without_a_confirmed_card() {
        let report = check_manuscript(&subject(&["示例课"]), &Context::default());
        assert!(!rules(&report).contains(&"R-W3"));
        assert_eq!(
            report
                .not_run
                .iter()
                .map(|n| n.rule.as_str())
                .collect::<Vec<_>>(),
            vec!["R-W3", "R-I2"]
        );
        assert!(report.not_run[0].reason.contains("产品事实卡还没有确认"));
        let with = check_manuscript(&subject(&["x"]), &ready_ctx());
        assert!(with.not_run.is_empty());
        assert_eq!(with.fact_cards[0].code, "CP-0001");
    }

    #[test]
    fn lengths_count_characters_without_spaces_at_the_limit() {
        let mut s = subject(&["正文"]);
        s.cover_title = "一二三四五六七八九十".into();
        s.title = "一二三四五 六七八九十一二三四五 六七八九十".into();
        let report = check_manuscript(&s, &Context::default());
        assert!(!rules(&report).contains(&"R-W8"));
        assert!(!rules(&report).contains(&"R-W9"));
        s.cover_title.push('一');
        s.title.push('一');
        let report = check_manuscript(&s, &Context::default());
        let cover = report.findings.iter().find(|f| f.rule == "R-W8").unwrap();
        assert!(cover.message.contains("11 个字"));
        assert!(rules(&report).contains(&"R-W9"));
    }

    #[test]
    fn repeated_topics_and_ai_covers_are_reported_in_their_levels() {
        let mut s = subject(&["正文"]);
        s.tags = vec![
            "ADHD".into(),
            "#adhd".into(),
            "放手".into(),
            "＃放手 ".into(),
        ];
        s.cover_ai = Some(true);
        let report = check_manuscript(&s, &Context::default());
        let repeats: Vec<&Location> = report
            .findings
            .iter()
            .filter(|f| f.rule == "R-W7")
            .map(|f| &f.location)
            .collect();
        assert_eq!(
            repeats,
            vec![&Location::Tag { index: 1 }, &Location::Tag { index: 3 }]
        );
        let hint = report.findings.iter().find(|f| f.rule == "R-I1").unwrap();
        assert_eq!(hint.level, Level::Hint);
        s.cover_ai = Some(false);
        assert!(!rules(&check_manuscript(&s, &Context::default())).contains(&"R-I1"));
        s.cover_ai = None;
        assert!(!rules(&check_manuscript(&s, &Context::default())).contains(&"R-I1"));
    }

    #[test]
    fn findings_are_ordered_by_level_then_rule_and_capped_per_rule() {
        let mut s = subject(&["治好 治好 治好"]);
        s.tags = vec!["a".into(), "a".into()];
        let report = check_manuscript(&s, &Context::default());
        let levels: Vec<Level> = report.findings.iter().map(|f| f.level).collect();
        let mut sorted = levels.clone();
        sorted.sort();
        assert_eq!(levels, sorted);
        assert_eq!(report.findings[0].level, Level::Must);
        let many = "治好，".repeat(100);
        let report = check_manuscript(&subject(&[many.as_str()]), &Context::default());
        assert_eq!(report.count(Level::Warn), MAX_FINDINGS_PER_RULE);
    }

    #[test]
    fn sentence_entry_blocks_only_cure_promises() {
        let (findings, not_run) = check_sentence("我是学员家长，孩子保证能好", Some(&cards()));
        assert!(blocks_entry(&findings));
        assert_eq!(findings[0].rule, "R-L1");
        assert!(not_run.is_empty());
        let (findings, _) = check_sentence("孩子变好了，欢迎私信我报名", Some(&cards()));
        assert!(!blocks_entry(&findings));
        let said: Vec<&str> = findings.iter().map(|f| f.rule.as_str()).collect();
        assert!(said.contains(&"R-L2") && said.contains(&"R-L5"));
        assert_eq!(said.iter().filter(|r| **r == "R-L6").count(), 2);
    }

    #[test]
    fn sentence_entry_covers_names_groups_and_length() {
        let (findings, _) = check_sentence("示例营的示例课让所有A娃都进步", Some(&cards()));
        let said: Vec<&str> = findings.iter().map(|f| f.rule.as_str()).collect();
        assert!(said.contains(&"R-L3") && said.contains(&"R-L4"));
        assert!(!said.contains(&"R-L5"), "the product is named");
        let long = "一".repeat(MAX_SENTENCE_CHARS + 1);
        let (findings, _) = check_sentence(&long, Some(&cards()));
        assert!(findings.iter().any(|f| f.rule == "R-L7"));
        let (findings, _) = check_sentence(&"一".repeat(MAX_SENTENCE_CHARS), Some(&cards()));
        assert!(!findings.iter().any(|f| f.rule == "R-L7"));
        let (findings, not_run) = check_sentence("普通的一句话", None);
        assert!(findings.is_empty());
        assert_eq!(
            not_run.iter().map(|n| n.rule.as_str()).collect::<Vec<_>>(),
            vec!["R-L4", "R-L5"]
        );
    }
}
