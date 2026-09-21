//! COLLECTION-001 · 插件上报的页面结构自检：把任意 JSON 收成受限形状。
//!
//! 插件的 preflight 已经知道「这次是在哪类页面上看的、哪些检查项在、哪些缺、验证日期是不是
//! 陈旧」，但这些事实此前只留在浏览器里。这份快照把它们带到服务端——代价是它会离开浏览器，
//! 所以要在入口**收一次口**：只留平台、页面类型、能力、两个时刻和检查项名。
//!
//! 收口的形状与插件 `src/shared/selectorHealth.js` 的 `SELECTOR_HEALTH_SNAPSHOT_FIELDS`
//! 是同一份合同的两侧，两边都做同一件事不是重复：
//!
//!   * 插件那一侧防的是「本机把不该出门的东西发出去」——选择器串、DOM 文本、带签名的页面
//!     地址在出门前就消失；
//!   * 这一侧防的是「不管谁发来的都按受限形状收」——上报口在页面上，页面来的东西不可信。
//!
//! 两条规则必须同时成立：
//!
//!   1. **形状不对就不收**，不是收下来再清理：连平台名都不是受限词、时刻位置上放了对象、
//!      载荷超过 [`SELECTOR_HEALTH_MAX_BYTES`] —— 整条不收。收不下的一份**不清空**这台安装
//!      已有的记录：那次报到什么都没证明，不能拿它去改写已有的结论。
//!   2. **词表里没有就写 `unknown` / `unclassified`**：那是「说不出来」，本身是一条如实的
//!      事实，不是把垃圾当默认值存下来。
//!
//! 两个时刻必须分开保留：`checkedAt` 是**这次**看的时刻，`verifiedAt` 是这些选择器上一次
//! 人工重验的日期。谁把它们合成一个，谁就把「刚看了一眼」说成「刚验证过」。
//!
//! 这份快照不参与任何准入、额度或派发判定：它是一句自述，不是一次授权。

use serde_json::{Map, Value};

/// 能上报的平台（闭集）。插件只在这两个站点上跑；别的一律不收。
pub const SELECTOR_HEALTH_PLATFORMS: [&str; 2] = ["xhs", "douyin"];

/// 页面类型的上报词表（闭集）。两套本机写法（`noteDetail`／`note_detail`）在插件那一侧
/// 已经合成一处，这里收的是合成后的字面值；认不出来写 `unknown`。
pub const SELECTOR_HEALTH_PAGE_TYPES: [&str; 6] = [
    "note_detail",
    "search_results",
    "profile",
    "explore",
    "detail",
    "unknown",
];

/// 一次检查能带的检查项上限（`checked`/`missing`/`stale` 各自，`failureCounts` 另算）。
pub const SELECTOR_HEALTH_MAX_CATEGORIES: usize = 16;

/// 一份上报收得下的最大字节数。超长载荷不只是「大」——它多半意味着有人往里塞了别的东西。
pub const SELECTOR_HEALTH_MAX_BYTES: usize = 4096;

const UNKNOWN: &str = "unknown";
const UNCLASSIFIED: &str = "unclassified";

/// 一份上报为什么没被收下。返回原因而不是 `bool`：拒绝要说得出来是什么被拒了，
/// 否则日志里只剩「没收下」三个字，下次还得重新推。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectorHealthRejection {
    /// 连一个对象都不是。
    NotAnObject,
    /// 超过 [`SELECTOR_HEALTH_MAX_BYTES`]。
    TooLarge,
    /// 是对象，但里面没有任何一条认得出的平台记录。
    NoUsableRecord,
}

impl SelectorHealthRejection {
    pub fn code(self) -> &'static str {
        match self {
            Self::NotAnObject => "selector_health_not_an_object",
            Self::TooLarge => "selector_health_too_large",
            Self::NoUsableRecord => "selector_health_no_usable_record",
        }
    }
}

/// 收下这次报到带来的诊断。收不下返回 `None`——那表示**不写**（保持这台安装已有的记录），
/// 不是写一条空的进去。
pub fn accepted_selector_health(candidate: Option<&Value>) -> Option<Value> {
    let raw = candidate?;
    match normalize_selector_health(raw) {
        Ok(accepted) => Some(accepted),
        Err(rejection) => {
            // 拒绝必须留痕：安静丢掉会让「这台机器一直在报一份我们认不出的东西」在服务端
            // 完全隐形，而那正是这类诊断要回答的问题。
            eprintln!(
                "linggan runtime: selector health report rejected ({})",
                rejection.code()
            );
            None
        }
    }
}

/// 把一份上报收成受限形状：键是平台名，值是那台平台的受限快照。
pub fn normalize_selector_health(candidate: &Value) -> Result<Value, SelectorHealthRejection> {
    let Some(submitted) = candidate.as_object() else {
        return Err(SelectorHealthRejection::NotAnObject);
    };
    // 尺寸先看：超长的东西不必再逐字段细看。
    if serde_json::to_vec(candidate).map_or(true, |raw| raw.len() > SELECTOR_HEALTH_MAX_BYTES) {
        return Err(SelectorHealthRejection::TooLarge);
    }

    let mut accepted = Map::new();
    for (platform, entry) in submitted {
        if !SELECTOR_HEALTH_PLATFORMS.contains(&platform.as_str()) {
            continue;
        }
        if let Some(snapshot) = normalize_platform_entry(platform, entry) {
            accepted.insert(platform.clone(), snapshot);
        }
    }
    if accepted.is_empty() {
        return Err(SelectorHealthRejection::NoUsableRecord);
    }
    Ok(Value::Object(accepted))
}

/// 一条平台记录的收口。收不下返回 `None`：这一条不收，别的平台照收。
fn normalize_platform_entry(platform: &str, entry: &Value) -> Option<Value> {
    let entry = entry.as_object()?;
    // 平台名必须就是这一条的键。两边对不上，这条记录就说不清是谁的——宁可不要。
    let declared = entry.get("platform")?.as_str()?;
    if restricted_token(declared).as_deref() != Some(platform) {
        return None;
    }
    let checked_at = moment(entry.get("checkedAt"))?;
    let verified_at = moment(entry.get("verifiedAt"))?;

    let mut snapshot = Map::new();
    snapshot.insert("platform".to_owned(), Value::from(platform));
    snapshot.insert(
        "pageType".to_owned(),
        Value::from(page_type(entry.get("pageType"))),
    );
    snapshot.insert(
        "capability".to_owned(),
        Value::from(token_or_unknown(entry.get("capability"))),
    );
    snapshot.insert("checkedAt".to_owned(), Value::from(checked_at));
    snapshot.insert("verifiedAt".to_owned(), Value::from(verified_at));
    snapshot.insert(
        "checkedCategories".to_owned(),
        category_list(entry.get("checkedCategories")),
    );
    snapshot.insert(
        "missingCategories".to_owned(),
        category_list(entry.get("missingCategories")),
    );
    snapshot.insert(
        "staleCategories".to_owned(),
        category_list(entry.get("staleCategories")),
    );
    snapshot.insert(
        "failureCounts".to_owned(),
        failure_counts(entry.get("failureCounts")),
    );
    Some(Value::Object(snapshot))
}

/// 受限词：小写字母、数字、下划线，1–32 字符。认不出来返回 `None`。
fn restricted_token(value: &str) -> Option<String> {
    let token = value.trim().to_ascii_lowercase();
    let shaped = !token.is_empty()
        && token.len() <= 32
        && token
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
    shaped.then_some(token)
}

/// 词表外一律写 `unknown`：这是「说不出来」，不是默认值。
fn token_or_unknown(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .and_then(restricted_token)
        .unwrap_or_else(|| UNKNOWN.to_owned())
}

/// 页面类型收进闭集。本机写法已经由插件合过一次，这里只认词表里的字面值。
fn page_type(value: Option<&Value>) -> String {
    let token = token_or_unknown(value);
    if SELECTOR_HEALTH_PAGE_TYPES.contains(&token.as_str()) {
        token
    } else {
        UNKNOWN.to_owned()
    }
}

/// 时刻：只接受可能是日期时间的形状。字段缺席写 `unknown`（「不知道是什么时候」），
/// 但**在这个位置上放了别的东西**（对象、数字、数组）是形状不对，整条不收。
fn moment(value: Option<&Value>) -> Option<String> {
    match value {
        None | Some(Value::Null) => Some(UNKNOWN.to_owned()),
        Some(Value::String(text)) => {
            let trimmed = text.trim();
            let shaped = !trimmed.is_empty()
                && trimmed.len() <= 40
                && trimmed.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'+' | b'.' | b'-')
                });
            shaped.then(|| trimmed.to_owned())
        }
        Some(_) => None,
    }
}

/// 检查项名列表：顺序保留、去重、各自封顶。名字形状不对写 `unclassified`——丢掉它等于把
/// 「这次有一项检查没过」从快照里删掉。非字符串的条目不是名字，跳过。
fn category_list(value: Option<&Value>) -> Value {
    let mut names: Vec<String> = Vec::new();
    let items = value.and_then(Value::as_array);
    for item in items.into_iter().flatten() {
        let Some(text) = item.as_str() else {
            continue;
        };
        let name = restricted_token(text).unwrap_or_else(|| UNCLASSIFIED.to_owned());
        if names.iter().any(|existing| existing == &name)
            || names.len() >= SELECTOR_HEALTH_MAX_CATEGORIES
        {
            continue;
        }
        names.push(name);
    }
    Value::Array(names.into_iter().map(Value::String).collect())
}

/// 连续失败次数：键是检查项名，值是正整数。负数和零不写（零次失败就是没有这条记录）。
fn failure_counts(value: Option<&Value>) -> Value {
    let mut counts = Map::new();
    for (category, count) in value.and_then(Value::as_object).into_iter().flatten() {
        if counts.len() >= SELECTOR_HEALTH_MAX_CATEGORIES {
            break;
        }
        let Some(category) = restricted_token(category) else {
            continue;
        };
        let Some(count) = count.as_i64().filter(|count| *count > 0) else {
            continue;
        };
        counts.insert(category, Value::from(count));
    }
    Value::Object(counts)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn report(overrides: Value) -> Value {
        let mut entry = json!({
            "platform": "xhs",
            "pageType": "search_results",
            "capability": "discovery_search",
            "checkedAt": "2026-09-21T10:00:00.000Z",
            "verifiedAt": "2026-06-01T00:00:00+08:00",
            "checkedCategories": ["feed_container"],
            "missingCategories": ["feed_container"],
            "staleCategories": [],
            "failureCounts": { "feed_container": 3 },
        });
        if let (Some(entry), Some(patch)) = (entry.as_object_mut(), overrides.as_object()) {
            for (key, value) in patch {
                entry.insert(key.clone(), value.clone());
            }
        }
        json!({ "xhs": entry })
    }

    #[test]
    fn a_report_is_accepted_in_the_restricted_shape_only() {
        let accepted = normalize_selector_health(&report(json!({}))).expect("收下");
        let snapshot = &accepted["xhs"];

        // 收下来的是这份形状，不是页面报来的原样：多出来的字段不会跟着走。
        let mut keys: Vec<&str> = snapshot
            .as_object()
            .expect("对象")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "capability",
                "checkedAt",
                "checkedCategories",
                "failureCounts",
                "missingCategories",
                "pageType",
                "platform",
                "staleCategories",
                "verifiedAt",
            ]
        );
        assert_eq!(snapshot["failureCounts"]["feed_container"], 3);
    }

    #[test]
    fn a_payload_that_is_not_an_object_at_all_is_refused() {
        for candidate in [json!("xhs"), json!([["xhs"]]), json!(42), json!(null)] {
            assert_eq!(
                normalize_selector_health(&candidate),
                Err(SelectorHealthRejection::NotAnObject)
            );
        }
    }

    #[test]
    fn an_oversized_payload_is_refused_before_it_is_read_field_by_field() {
        let padded = "x".repeat(SELECTOR_HEALTH_MAX_BYTES);
        let candidate = report(json!({ "checkedAt": padded }));
        assert_eq!(
            normalize_selector_health(&candidate),
            Err(SelectorHealthRejection::TooLarge)
        );
    }

    #[test]
    fn a_record_that_cannot_name_its_own_platform_is_dropped_while_others_are_kept() {
        // 平台名对不上、载荷里没有一条认得出的记录、别的站点替它报：三种都不收。
        assert_eq!(
            normalize_selector_health(&report(json!({ "platform": "douyin" }))),
            Err(SelectorHealthRejection::NoUsableRecord)
        );
        assert_eq!(
            normalize_selector_health(&json!({ "weibo": report(json!({}))["xhs"] })),
            Err(SelectorHealthRejection::NoUsableRecord)
        );

        // 一条收不下不影响同一次上报里的另一条。
        let mixed = json!({
            "xhs": report(json!({}))["xhs"],
            "douyin": report(json!({ "platform": "douyin", "checkedAt": {"at": 1} }))["xhs"],
        });
        let accepted = normalize_selector_health(&mixed).expect("收下小红书那条");
        assert_eq!(accepted.as_object().expect("对象").len(), 1);
        assert!(accepted.get("xhs").is_some());
    }

    #[test]
    fn words_outside_the_vocabulary_become_unknown_instead_of_being_stored_as_given() {
        let accepted = normalize_selector_health(&report(json!({
            "pageType": "some_new_page",
            "capability": "SIGNED.LINK/../",
            "checkedCategories": ["feed_container", "评论容器", 7],
            "failureCounts": { "feed_container": -2, "另一个": 4 },
        })))
        .expect("收下");
        let snapshot = &accepted["xhs"];

        assert_eq!(snapshot["pageType"], "unknown");
        assert_eq!(snapshot["capability"], "unknown");
        // 名字形状不对的检查项不丢，写成 unclassified；不是字符串的条目不是名字。
        assert_eq!(
            snapshot["checkedCategories"],
            json!(["feed_container", "unclassified"])
        );
        // 零和负数是「没有这条记录」，不是「失败了 -2 次」。
        assert_eq!(snapshot["failureCounts"], json!({}));
    }

    #[test]
    fn the_two_moments_stay_two_moments() {
        let accepted = normalize_selector_health(&report(json!({
            "checkedAt": "2026-09-21T10:00:00.000Z",
            "verifiedAt": "2026-06-01T00:00:00+08:00",
        })))
        .expect("收下");
        let snapshot = &accepted["xhs"];
        assert_eq!(snapshot["checkedAt"], "2026-09-21T10:00:00.000Z");
        assert_eq!(snapshot["verifiedAt"], "2026-06-01T00:00:00+08:00");

        // 缺席是「不知道是什么时候」，不是「就是现在」。
        let missing =
            normalize_selector_health(&report(json!({ "checkedAt": null }))).expect("收下");
        assert_eq!(missing["xhs"]["checkedAt"], "unknown");
    }
}
