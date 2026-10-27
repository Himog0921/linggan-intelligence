//! Encode canonical plain text as Markdown without interpreting source punctuation.
use serde_json::Value;

pub(super) fn escape(value: &str) -> String {
    let mut out = String::new();
    for character in value.chars() {
        if matches!(
            character,
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '#' | '|' | '~' | '&'
        ) {
            out.push('\\');
        }
        out.push(character);
    }
    // Prevent source lines from creating lists, rules, or setext headings.
    out.split('\n')
        .map(|line| {
            let trimmed = line.trim_start();
            let offset = line.len() - trimmed.len();
            let number = trimmed.bytes().take_while(u8::is_ascii_digit).count();
            let point = if trimmed.starts_with(['-', '+', '=']) {
                Some(offset)
            } else if number > 0 && matches!(trimmed.as_bytes().get(number), Some(b'.' | b')')) {
                Some(offset + number)
            } else {
                None
            };
            if let Some(point) = point {
                format!("{}\\{}", &line[..point], &line[point..])
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn block(value: &Value) -> String {
    match value["kind"].as_str() {
        Some("list") => value["items"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(i, item)| {
                let marker = if value["ordered"] == true {
                    format!("{}.", value["start"].as_u64().unwrap_or(1) + i as u64)
                } else {
                    "-".into()
                };
                let indent = " ".repeat(marker.len() + 1);
                format!(
                    "{marker} {}",
                    escape(item.as_str().unwrap_or("")).replace('\n', &format!("\n{indent}"))
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Some("table") => table(value),
        Some("dialogue") => value["turns"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|turn| {
                format!(
                    "{}：{}",
                    super::content::speaker_label(
                        turn["speaker"].as_str().unwrap_or("unconfirmed")
                    ),
                    escape(turn["text"].as_str().unwrap_or(""))
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => escape(value["text"].as_str().unwrap_or("")),
    }
}

fn table(value: &Value) -> String {
    let row = |values: &[Value]| {
        format!(
            "| {} |",
            values
                .iter()
                .map(|cell| { escape(cell.as_str().unwrap_or("")).replace('\n', "<br>") })
                .collect::<Vec<_>>()
                .join(" | ")
        )
    };
    let columns = value["columns"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut lines = vec![
        row(columns),
        format!("| {} |", vec!["---"; columns.len()].join(" | ")),
    ];
    lines.extend(
        value["rows"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|cells| row(cells.as_array().map(Vec::as_slice).unwrap_or(&[]))),
    );
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn literal_markup_and_multiline_cells_remain_text() {
        assert_eq!(
            escape("请填 <product-id> 与 **两个星号**、`原话`、&copy;"),
            "请填 \\<product-id\\> 与 \\*\\*两个星号\\*\\*、\\`原话\\`、\\&copy;"
        );
        assert_eq!(
            escape("1. 原文编号\n- 原文短划\n==="),
            "1\\. 原文编号\n\\- 原文短划\n\\==="
        );
        let source =
            json!({"kind":"table","columns":["说明"],"rows":[["第一行\n第二行 <br> | **原文**"]]});
        assert_eq!(
            block(&source),
            "| 说明 |\n| --- |\n| 第一行<br>第二行 \\<br\\> \\| \\*\\*原文\\*\\* |"
        );
        assert_eq!(source["rows"][0][0], "第一行\n第二行 <br> | **原文**");
    }
}
