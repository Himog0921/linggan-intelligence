//! No background loop: print a bounded read-only plan, or explicitly apply one saved plan.
use linggan_evidence::dossier_input::{self, BatchOperation, BatchPlan};
use std::process::ExitCode;
use uuid::Uuid;

const MAX_PLAN_BYTES: u64 = 16 * 1024 * 1024;

enum Command {
    Plan {
        domain: Uuid,
        works: Vec<Uuid>,
        as_of: Option<String>,
        operation: BatchOperation,
    },
    Apply(String),
}
fn parse(args: &[String]) -> Result<Command, &'static str> {
    if let [mode, flag, path] = args
        && mode == "apply"
        && flag == "--plan"
    {
        return Ok(Command::Apply(path.clone()));
    }
    let args = if args.first().is_some_and(|v| v == "plan") {
        &args[1..]
    } else {
        args
    };
    let mut flags = std::collections::BTreeMap::new();
    for pair in args.chunks(2) {
        if pair.len() != 2
            || !["--domain", "--works", "--as-of", "--operation"].contains(&pair[0].as_str())
            || flags.insert(pair[0].as_str(), pair[1].as_str()).is_some()
        {
            return Err("invalid_or_duplicate_arguments");
        }
    }
    let domain = flags
        .get("--domain")
        .ok_or("domain_required")?
        .parse()
        .map_err(|_| "invalid_domain")?;
    let works = flags
        .get("--works")
        .ok_or("explicit_works_required")?
        .split(',')
        .map(|v| v.parse().map_err(|_| "invalid_work"))
        .collect::<Result<Vec<Uuid>, _>>()?;
    if works.is_empty()
        || works.len() > dossier_input::MAX_WORKS
        || works
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != works.len()
    {
        return Err("invalid_scope");
    }
    let operation = match flags.get("--operation").copied() {
        Some("native_body") => BatchOperation::NativeBody,
        Some("hashtags") => BatchOperation::Hashtags,
        Some("relayer") => BatchOperation::Relayer,
        Some("snapshot") => BatchOperation::Snapshot,
        Some("historical_media") => BatchOperation::HistoricalMedia,
        _ => return Err("operation_required"),
    };
    Ok(Command::Plan {
        domain,
        works,
        operation,
        as_of: flags.get("--as-of").map(|s| s.to_string()),
    })
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => {
            eprintln!("research-dossier: partial_or_failed; see receipt");
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("research-dossier: {error}");
            ExitCode::FAILURE
        }
    }
}
async fn run() -> Result<bool, String> {
    let command = parse(&std::env::args().skip(1).collect::<Vec<_>>()).map_err(str::to_owned)?;
    // Parse and bound a saved file before opening any database connection.
    let saved = if let Command::Apply(path) = &command {
        let file = std::fs::File::open(path).map_err(|_| "plan_file_unavailable")?;
        Some(decode_plan(file).map_err(str::to_owned)?)
    } else {
        None
    };
    let url = std::env::var("LINGGAN_LOCAL_DATABASE_URL")
        .map_err(|_| "explicit_database_url_required")?;
    let database = linggan_storage_postgres::Database::connect(&url)
        .await
        .map_err(|_| "database_unavailable")?;
    let (output, complete) = match command {
        Command::Plan {
            domain,
            works,
            as_of,
            operation,
        } => {
            let result =
                dossier_input::plan(&database, domain, &works, as_of.as_deref(), operation)
                    .await
                    .map_err(|e| e.to_string())?;
            (
                serde_json::to_value(result).map_err(|_| "serialization_failed")?,
                true,
            )
        }
        Command::Apply(_) => {
            let report = dossier_input::apply_plan(&database, &saved.expect("parsed apply plan"))
                .await
                .map_err(|e| e.to_string())?;
            let complete = report.complete();
            (
                serde_json::to_value(report).map_err(|_| "serialization_failed")?,
                complete,
            )
        }
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&output).map_err(|_| "serialization_failed")?
    );
    Ok(complete)
}
fn decode_plan(reader: impl std::io::Read) -> Result<BatchPlan, &'static str> {
    use std::io::Read;
    let mut bytes = vec![];
    reader
        .take(MAX_PLAN_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "plan_file_unavailable")?;
    if bytes.len() as u64 > MAX_PLAN_BYTES {
        return Err("plan_file_too_large");
    }
    serde_json::from_slice(&bytes).map_err(|_| "plan_file_invalid")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commands_require_bounded_explicit_scope_and_apply_is_explicit() {
        let args = |s: &str| s.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
        assert!(parse(&args("--domain 00000000-0000-4000-8000-000000000001 --works 00000000-0000-4000-8000-000000000010 --operation hashtags")).is_ok());
        assert!(matches!(parse(&args("--domain 00000000-0000-4000-8000-000000000001 --works 00000000-0000-4000-8000-000000000010 --operation historical_media")).unwrap(), Command::Plan { operation: BatchOperation::HistoricalMedia, .. }));
        for s in [
            "",
            "apply",
            "apply --plan x --apply",
            "--domain x --domain x",
            "--works all --operation relayer",
        ] {
            assert!(parse(&args(s)).is_err());
        }
        assert!(matches!(
            parse(&args("apply --plan x")).unwrap(),
            Command::Apply(_)
        ));
    }
    #[test]
    fn largest_legal_scope_can_be_read_but_unbounded_files_are_rejected() {
        use serde_json::json;
        use std::io::Read;
        let details: Vec<_> = (1..=500).map(Uuid::from_u128).collect();
        let entries:Vec<_>=(1000..1200).map(|n|json!({"workRef":Uuid::from_u128(n),"expectedRevision":"a".repeat(64),"materialFingerprint":"b".repeat(64),"relayerSourceHash":"c".repeat(64),"detailRefs":details,"pendingTagDetails":500,"sourceNotProvided":0,"sourceInvalid":0,"sourceNonempty":500,"relayer":{}})).collect();
        let value = json!({"contract":"research-dossier-plan.v2","domainRef":Uuid::nil(),"asOf":"2026-10-19T00:00:00Z","layerVersion":"rules-v3","maximumWorks":200,"operation":"hashtags","entries":entries,"planHash":"d".repeat(64)});
        let bytes = serde_json::to_vec_pretty(&value).unwrap();
        assert!(bytes.len() > 1024 * 1024);
        assert_eq!(decode_plan(bytes.as_slice()).unwrap().entries.len(), 200);
        assert_eq!(
            decode_plan(std::io::repeat(b' ').take(MAX_PLAN_BYTES + 1)).unwrap_err(),
            "plan_file_too_large"
        );
    }
}
