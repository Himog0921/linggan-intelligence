//! Native text parsing and the deterministic text form of canonical blocks.
use serde_json::{Value, json};

pub fn blocks_from_text(text: &str, speaker: &str) -> Vec<Value> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.lines().collect();
    let mut blocks = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        if lines[index].trim().is_empty() {
            index += 1;
            continue;
        }
        if let Some((table, next)) = parse_table(&lines, index, speaker) {
            blocks.push(table);
            index = next;
            continue;
        }
        if let Some((level, heading)) = heading(lines[index]) {
            blocks.push(json!({"kind":"heading","level":level,"text":heading,"speaker":speaker}));
            index += 1;
            continue;
        }
        if let Some((ordered, start, _)) = list_item(lines[index]) {
            let mut items = Vec::new();
            while let Some(line) = lines.get(index) {
                let Some((next_ordered, number, text)) = list_item(line) else {
                    break;
                };
                if next_ordered != ordered || (ordered && number != start + items.len()) {
                    break;
                }
                items.push(text);
                index += 1;
            }
            let mut block =
                json!({"kind":"list","ordered":ordered,"items":items,"speaker":speaker});
            if ordered {
                block["start"] = json!(start);
            }
            blocks.push(block);
            continue;
        }
        let quote = lines[index].starts_with("> ");
        let mut paragraph = vec![];
        while index < lines.len() && !lines[index].trim().is_empty() {
            if !paragraph.is_empty()
                && (heading(lines[index]).is_some()
                    || list_item(lines[index]).is_some()
                    || parse_table(&lines, index, speaker).is_some()
                    || lines[index].starts_with("> ") != quote)
            {
                break;
            }
            paragraph.push(if quote {
                &lines[index][2..]
            } else {
                lines[index]
            });
            index += 1;
        }
        blocks.push(
            json!({"kind":if quote {"quote"} else {"paragraph"},"text":paragraph.join("\n"),
            "speaker":if quote {"unconfirmed"} else {speaker}}),
        );
    }
    blocks
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let level = line.chars().take_while(|c| *c == '#').count();
    (level > 0 && level <= 6 && line.as_bytes().get(level) == Some(&b' '))
        .then(|| (level, &line[level + 1..]))
}

fn list_item(line: &str) -> Option<(bool, usize, &str)> {
    if let Some(text) = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("+ "))
    {
        return Some((false, 1, text));
    }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let rest = line.get(digits..)?;
    let text = rest
        .strip_prefix(". ")
        .or_else(|| rest.strip_prefix(") "))?;
    let number = line[..digits].parse::<usize>().ok().filter(|n| *n > 0)?;
    Some((true, number, text))
}

fn table_cells(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') || trimmed.len() < 2 {
        return None;
    }
    let mut cells = vec![String::new()];
    let mut escaped = false;
    for c in trimmed[1..trimmed.len() - 1].chars() {
        if escaped {
            if !matches!(c, '|' | '\\') {
                cells.last_mut().expect("cell").push('\\');
            }
            cells.last_mut().expect("cell").push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '|' {
            cells.push(String::new());
        } else {
            cells.last_mut().expect("cell").push(c);
        }
    }
    if escaped {
        cells.last_mut().expect("cell").push('\\');
    }
    Some(cells.into_iter().map(|c| c.trim().to_owned()).collect())
}

fn parse_table(lines: &[&str], index: usize, speaker: &str) -> Option<(Value, usize)> {
    let columns = table_cells(lines[index])?;
    let separators = table_cells(lines.get(index + 1)?)?;
    if separators.len() != columns.len()
        || separators.iter().any(|c| {
            let inner = c.trim_matches(':');
            inner.len() < 3 || !inner.chars().all(|ch| ch == '-')
        })
    {
        return None;
    }
    let mut rows = vec![];
    let mut next = index + 2;
    while let Some(line) = lines.get(next) {
        let Some(cells) = table_cells(line) else {
            break;
        };
        if cells.len() != columns.len() {
            break;
        }
        rows.push(cells);
        next += 1;
    }
    Some((
        json!({"kind":"table","columns":columns,"rows":rows,"speaker":speaker}),
        next,
    ))
}

pub fn block_text(block: &Value) -> String {
    match block["kind"].as_str() {
        Some("list") => block["items"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(i, v)| {
                format!(
                    "{} {}",
                    if block["ordered"] == true {
                        format!("{}.", block["start"].as_u64().unwrap_or(1) + i as u64)
                    } else {
                        "-".into()
                    },
                    v.as_str().unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Some("table") => table_text(block),
        Some("dialogue") => block["turns"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|v| {
                format!(
                    "{}：{}",
                    speaker_label(v["speaker"].as_str().unwrap_or("unconfirmed")),
                    v["text"].as_str().unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => block["text"].as_str().unwrap_or("").to_owned(),
    }
}

pub fn speaker_label(speaker: &str) -> &'static str {
    match speaker {
        "author" => "作者",
        "other" => "他人",
        _ => "角色未确认",
    }
}

fn table_text(block: &Value) -> String {
    let row = |values: &[Value]| {
        format!(
            "| {} |",
            values
                .iter()
                .map(|v| v
                    .as_str()
                    .unwrap_or("")
                    .replace('\\', "\\\\")
                    .replace('|', "\\|")
                    .replace('\n', "<br>"))
                .collect::<Vec<_>>()
                .join(" | ")
        )
    };
    let columns = block["columns"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut lines = vec![
        row(columns),
        format!("| {} |", vec!["---"; columns.len()].join(" | ")),
    ];
    for cells in block["rows"].as_array().into_iter().flatten() {
        lines.push(row(cells.as_array().map(Vec::as_slice).unwrap_or(&[])));
    }
    lines.join("\n")
}
