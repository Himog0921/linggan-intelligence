//! Operator commands for the content retrospect dashboard.
use linggan_intelligence::ops_content_lens::{
    SnapshotScope, build_snapshot, enabled_from_env, load_taxonomy,
};
use linggan_storage_postgres::Database;
use std::{ffi::OsString, process::ExitCode};

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

const USAGE: &str = "usage: linggan-ops-content-lens load-taxonomy --file FILE | build-snapshot [--domain UUID] [--from YYYY-MM-DD] [--to YYYY-MM-DD]";

fn scope(args: &[OsString]) -> Result<SnapshotScope, &'static str> {
    let mut scope = SnapshotScope::default();
    for pair in args.chunks(2) {
        let [flag, value] = pair else {
            return Err(USAGE);
        };
        let value = value.to_str().ok_or(USAGE)?;
        match flag.to_str() {
            Some("--domain") if scope.domain_ref.is_none() => {
                scope.domain_ref = Some(value.parse().map_err(|_| USAGE)?)
            }
            Some("--from") if scope.from.is_none() => scope.from = Some(value.into()),
            Some("--to") if scope.to.is_none() => scope.to = Some(value.into()),
            _ => return Err(USAGE),
        }
    }
    Ok(scope)
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let Some((command, args)) = args.split_first() else {
        return Err(USAGE.into());
    };
    let db = || async {
        Database::connect(&std::env::var("LINGGAN_LOCAL_DATABASE_URL")?)
            .await
            .map_err(Box::<dyn std::error::Error>::from)
    };
    let receipt = match (command.to_str(), args) {
        (Some("load-taxonomy"), [flag, file]) if flag == "--file" => {
            let bytes = std::fs::read(file)?;
            serde_json::to_string(&load_taxonomy(&db().await?, &bytes).await?)?
        }
        (Some("build-snapshot"), args) => {
            if !enabled_from_env() {
                return Err("ops_content_lens_disabled".into());
            }
            let scope = scope(args)?;
            serde_json::to_string(&build_snapshot(&db().await?, scope).await?)?
        }
        _ => return Err(USAGE.into()),
    };
    println!("{receipt}");
    Ok(())
}
