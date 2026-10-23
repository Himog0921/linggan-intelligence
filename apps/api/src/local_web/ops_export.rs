//! 导出 (OPS-HTTP-001): the same columns as the pages, plus the workbench record number and the
//! export time of the mirror. CSV carries a UTF-8 BOM for spreadsheet apps; unknown values are
//! empty cells, never 0; text that a spreadsheet could read as a formula is neutralised.
use super::*;
use linggan_intelligence::ops_ledger::read::{
    OpsAccountItem, OpsAsOf, OpsDailyItem, OpsPublicationItem,
};

pub(super) struct ExportTable {
    headers: &'static [&'static str],
    rows: Vec<Vec<Cell>>,
    json: serde_json::Value,
}

enum Cell {
    Text(Option<String>),
    Number(Option<String>),
}

fn text(value: Option<&str>) -> Cell {
    Cell::Text(value.map(str::to_owned))
}

fn number<T: ToString>(value: Option<T>) -> Cell {
    Cell::Number(value.map(|value| value.to_string()))
}

fn platform(value: &str) -> &'static str {
    if value == "douyin" {
        "抖音"
    } else {
        "小红书"
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "是" } else { "否" }
}

pub(super) fn publications(items: &[OpsPublicationItem]) -> ExportTable {
    const HEADERS: &[&str] = &[
        "发布日期",
        "平台",
        "标题",
        "正文",
        "发布链接",
        "账号",
        "账号状态",
        "运营人",
        "阅读量",
        "阅读量状态",
        "爆款",
        "点赞",
        "收藏",
        "评论",
        "封面点击率",
        "日期状态",
        "重复登记数",
        "来源记录号",
    ];
    let rows = items
        .iter()
        .map(|item| {
            vec![
                text(Some(&item.business_date)),
                text(Some(platform(&item.platform))),
                text(item.title.as_deref()),
                text(item.body.as_deref()),
                text(Some(&item.url)),
                text(item.account.as_ref().map(|account| account.name.as_str())),
                text(item.account.as_ref().map(|account| {
                    if account.ledger_state == "in_ledger" {
                        "在台账"
                    } else {
                        "已停用"
                    }
                })),
                text(item.publisher.as_deref()),
                number(item.views.value),
                text(Some(match item.views.state {
                    "value" => "已填",
                    "pending" => "待补",
                    _ => "选填",
                })),
                text(Some(yes_no(item.views.hit))),
                number(item.likes),
                number(item.collects),
                number(item.comments),
                number(item.cover_click_rate),
                text((item.date_state == "suspect_year").then_some("日期存疑")),
                number(Some(item.duplicates.len())),
                text(Some(&item.source_id)),
            ]
        })
        .collect();
    ExportTable {
        headers: HEADERS,
        rows,
        json: serde_json::to_value(items).unwrap_or_default(),
    }
}

pub(super) fn accounts(items: &[OpsAccountItem]) -> ExportTable {
    const HEADERS: &[&str] = &[
        "平台",
        "账号名称",
        "主页链接",
        "设备编号",
        "粉丝",
        "IP所在地",
        "运营人",
        "账号状态",
        "账号健康",
        "保留",
        "账号备注",
        "来源记录号",
    ];
    let rows = items
        .iter()
        .map(|item| {
            vec![
                text(Some(platform(&item.platform))),
                text(Some(&item.name)),
                text(item.profile_url.as_deref()),
                text(item.device_code.as_deref()),
                number(item.follower_count),
                text(item.ip_location.as_deref()),
                text(item.operator.as_deref()),
                text(item.status.as_deref().map(account_status)),
                text(Some("未填")),
                text(item.is_retained.map(yes_no)),
                text(item.notes.as_deref()),
                text(Some(&item.source_id)),
            ]
        })
        .collect();
    ExportTable {
        headers: HEADERS,
        rows,
        json: serde_json::to_value(items).unwrap_or_default(),
    }
}

pub(super) fn daily(items: &[OpsDailyItem]) -> ExportTable {
    const HEADERS: &[&str] = &[
        "日期",
        "星期",
        "运营人",
        "发布",
        "打开",
        "加微",
        "添加率",
        "爆款",
        "月累计加微",
        "月累计发布",
        "来源记录号",
    ];
    let rows = items
        .iter()
        .map(|item| {
            vec![
                text(Some(&item.business_date)),
                text(Some(item.weekday)),
                text(Some(&item.operator)),
                number(Some(item.published)),
                number(item.links_opened),
                number(item.followers_gained),
                number(item.add_rate.map(|rate| format!("{:.4}", rate))),
                number(Some(item.hits)),
                number(item.month_followers_gained),
                number(Some(item.month_published)),
                text(Some(&item.source_id)),
            ]
        })
        .collect();
    ExportTable {
        headers: HEADERS,
        rows,
        json: serde_json::to_value(items).unwrap_or_default(),
    }
}

pub(super) fn account_status(status: &str) -> &'static str {
    match status {
        "active" => "使用中",
        "violation_disabled" => "违规停用",
        "warming" => "养号中",
        "paused" => "暂停",
        "archived" => "已归档",
        _ => "未知",
    }
}

pub(super) fn response(
    kind: &str,
    format: &str,
    table: &ExportTable,
    as_of: Option<&OpsAsOf>,
) -> Response {
    // JSON keeps the machine instant; the spreadsheet column reads in Beijing time.
    let (body, content_type) = if format == "json" {
        let as_of = as_of.map(|as_of| as_of.exported_at.as_str());
        let body = serde_json::json!({ "kind": kind, "asOf": as_of, "items": table.json });
        (body.to_string(), "application/json; charset=utf-8")
    } else {
        let as_of = as_of.map(|as_of| as_of.exported_at_beijing.as_str());
        (csv(table, as_of), "text/csv; charset=utf-8")
    };
    let disposition = format!("attachment; filename=\"ops-{kind}.{format}\"");
    let mut response = (axum::http::StatusCode::OK, body).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    if let Ok(value) = HeaderValue::from_str(&disposition) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }
    response
}

fn csv(table: &ExportTable, as_of: Option<&str>) -> String {
    let mut output = String::from("\u{feff}");
    let mut header: Vec<String> = table
        .headers
        .iter()
        .map(|header| (*header).to_owned())
        .collect();
    header.push("数据截至（北京时间）".to_owned());
    output.push_str(&header.join(","));
    output.push_str("\r\n");
    for row in &table.rows {
        let mut cells: Vec<String> = row.iter().map(csv_cell).collect();
        cells.push(csv_field(as_of.unwrap_or_default()));
        output.push_str(&cells.join(","));
        output.push_str("\r\n");
    }
    output
}

fn csv_cell(cell: &Cell) -> String {
    match cell {
        Cell::Number(value) => value.clone().unwrap_or_default(),
        Cell::Text(None) => String::new(),
        Cell::Text(Some(value)) => {
            let neutralised = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
                format!("'{value}")
            } else {
                value.clone()
            };
            csv_field(&neutralised)
        }
    }
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::{Cell, csv_cell};

    #[test]
    fn csv_keeps_unknown_empty_quotes_text_and_neutralises_formulas() {
        assert_eq!(
            csv_cell(&Cell::Number(None)),
            "",
            "unknown is an empty cell, never 0"
        );
        assert_eq!(csv_cell(&Cell::Number(Some("0".into()))), "0");
        assert_eq!(
            csv_cell(&Cell::Text(Some("a,\"b\"\n".into()))),
            "\"a,\"\"b\"\"\n\""
        );
        assert_eq!(
            csv_cell(&Cell::Text(Some("=HYPERLINK(\"x\")".into()))),
            "\"'=HYPERLINK(\"\"x\"\")\""
        );
        assert_eq!(csv_cell(&Cell::Text(Some("-1 篇".into()))), "'-1 篇");
    }
}
