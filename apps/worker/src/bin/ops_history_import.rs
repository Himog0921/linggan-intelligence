//! Operator tool for OPS-MIGRATION-001 D1: imports the content workbench's three operation
//! exports into the operations ledger. `preview` never writes; `apply` needs the preview hash.
//! Output is aggregate JSON only — no names, titles or links.
use linggan_intelligence::ops_ledger::{
    ExportFiles, ImportRequest, apply_ops_import, compare_with_ledger, preview_ops_import,
    read_ledger_totals,
};
use std::collections::HashMap;
use std::process::ExitCode;

const USAGE: &str = "usage: linggan-ops-history-import (preview|apply) --accounts FILE \
--publications FILE --daily FILE --exported-at TIME_WITH_OFFSET --domain-ref UUID \
[--preview-hash SHA256 (apply only)]";
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

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
    let request = ImportRequest {
        files: ExportFiles {
            accounts: read_bounded(&flags["--accounts"])?,
            publications: read_bounded(&flags["--publications"])?,
            daily: read_bounded(&flags["--daily"])?,
        },
        exported_at: flags["--exported-at"].clone(),
        domain_ref: uuid::Uuid::parse_str(&flags["--domain-ref"])?,
    };
    let url = std::env::var("LINGGAN_LOCAL_DATABASE_URL")?;
    let database = linggan_storage_postgres::Database::connect(&url).await?;
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

/// Parses the full command line before any file or database is touched.
fn parse_args(args: &[String]) -> Result<(String, HashMap<String, String>), String> {
    let Some((mode, rest)) = args.split_first() else {
        return Err(USAGE.into());
    };
    if mode != "preview" && mode != "apply" {
        return Err(USAGE.into());
    }
    let mut required = vec![
        "--accounts",
        "--publications",
        "--daily",
        "--exported-at",
        "--domain-ref",
    ];
    if mode == "apply" {
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
