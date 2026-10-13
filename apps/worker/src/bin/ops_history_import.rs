//! Operator tool for OPS-MIGRATION-001: imports the content workbench's three operation exports
//! into the operations ledger (D1), and registers the historical covers Mog copied from the
//! workbench machine (D2). `preview` modes never write; `apply` modes need the preview hash.
//! Output is aggregate JSON only — no names, titles, links or paths.
use linggan_intelligence::ops_ledger::covers::{
    CoverImportRequest, apply_cover_import, preview_cover_import,
};
use linggan_intelligence::ops_ledger::{
    ExportFiles, ImportRequest, apply_ops_import, compare_with_ledger, preview_ops_import,
    read_ledger_totals,
};
use linggan_storage_postgres::Database;
use std::collections::HashMap;
use std::process::ExitCode;

const USAGE: &str = "usage:\n  linggan-ops-history-import (preview|apply) --accounts FILE \
--publications FILE --daily FILE --exported-at TIME_WITH_OFFSET --domain-ref UUID \
[--preview-hash SHA256 (apply only)]\n  linggan-ops-history-import (cover-preview|cover-apply) \
--publications FILE --covers-dir DIR --media-root DIR [--preview-hash SHA256 (cover-apply only)]";
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const LEDGER_FLAGS: [&str; 5] = [
    "--accounts",
    "--publications",
    "--daily",
    "--exported-at",
    "--domain-ref",
];
const COVER_FLAGS: [&str; 3] = ["--publications", "--covers-dir", "--media-root"];

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("运营历史导入：{error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mode, flags) = parse_args(&args)?;
    let url = std::env::var("LINGGAN_LOCAL_DATABASE_URL")?;
    let database = Database::connect(&url).await?;
    if mode.starts_with("cover-") {
        return covers(&database, &mode, &flags).await;
    }
    let request = ImportRequest {
        files: ExportFiles {
            accounts: read_bounded(&flags["--accounts"])?,
            publications: read_bounded(&flags["--publications"])?,
            daily: read_bounded(&flags["--daily"])?,
        },
        exported_at: flags["--exported-at"].clone(),
        domain_ref: uuid::Uuid::parse_str(&flags["--domain-ref"])?,
    };
    if mode == "preview" {
        let preview = preview_ops_import(&database, &request).await?;
        println!("{}", serde_json::to_string_pretty(&preview)?);
        return Ok(ExitCode::SUCCESS);
    }
    let reconciliation = preview_ops_import(&database, &request)
        .await?
        .reconciliation;
    let receipt = apply_ops_import(&database, &request, &flags["--preview-hash"]).await?;
    let ledger = read_ledger_totals(&database).await?;
    let checks = compare_with_ledger(&reconciliation, &ledger);
    let consistent = checks.iter().all(|check| check.consistent);
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "receipt": receipt, "allConsistent": consistent, "checks": checks,
            "reconciliation": reconciliation, "ledger": ledger,
        }))?
    );
    if !consistent {
        eprintln!("运营历史导入：已写入，但新旧对账有不一致项，见 checks");
        return Ok(ExitCode::FAILURE);
    }
    Ok(ExitCode::SUCCESS)
}

async fn covers(
    database: &Database,
    mode: &str,
    flags: &HashMap<String, String>,
) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let request = CoverImportRequest {
        publications_export: read_bounded(&flags["--publications"])?,
        covers_dir: existing_dir(&flags["--covers-dir"])?,
        media_root: existing_dir(&flags["--media-root"])?,
    };
    let output = if mode == "cover-preview" {
        serde_json::to_string_pretty(&preview_cover_import(database, &request).await?)?
    } else {
        serde_json::to_string_pretty(
            &apply_cover_import(database, &request, &flags["--preview-hash"]).await?,
        )?
    };
    println!("{output}");
    Ok(ExitCode::SUCCESS)
}

/// Parses the full command line before any file or database is touched.
fn parse_args(args: &[String]) -> Result<(String, HashMap<String, String>), String> {
    let Some((mode, rest)) = args.split_first() else {
        return Err(USAGE.into());
    };
    let mut required: Vec<&str> = match mode.as_str() {
        "preview" | "apply" => LEDGER_FLAGS.to_vec(),
        "cover-preview" | "cover-apply" => COVER_FLAGS.to_vec(),
        _ => return Err(USAGE.into()),
    };
    if mode == "apply" || mode == "cover-apply" {
        required.push("--preview-hash");
    }
    if rest.len() % 2 != 0 {
        return Err(USAGE.into());
    }
    let mut flags = HashMap::new();
    for pair in rest.chunks(2) {
        let (flag, value) = (&pair[0], &pair[1]);
        if !required.contains(&flag.as_str()) || flags.insert(flag.clone(), value.clone()).is_some()
        {
            return Err(USAGE.into());
        }
    }
    if flags.len() != required.len() {
        return Err(USAGE.into());
    }
    Ok((mode.clone(), flags))
}

fn read_bounded(path: &str) -> Result<Vec<u8>, String> {
    let size = std::fs::metadata(path)
        .map_err(|_| format!("读不到文件：{path}"))?
        .len();
    if size > MAX_FILE_BYTES {
        return Err(format!("文件超过 64 MiB，拒绝读取：{path}"));
    }
    std::fs::read(path).map_err(|_| format!("读不到文件：{path}"))
}

fn existing_dir(path: &str) -> Result<std::path::PathBuf, String> {
    let path = std::path::PathBuf::from(path);
    if path.is_dir() {
        Ok(path)
    } else {
        Err(format!("不是已存在的文件夹：{}", path.display()))
    }
}
