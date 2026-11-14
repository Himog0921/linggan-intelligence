//! Operator-only replay of complete L1 replies saved in the local database.
use linggan_intelligence::ai_foundation::content_understanding::recover_saved_replies;
use linggan_storage_postgres::Database;
use std::process::ExitCode;
use uuid::Uuid;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("L1 offline recovery: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [mode, revision, requests @ ..] = args.as_slice() else {
        return Err(
            "usage: content_understanding_recovery --preview|--apply REVISION REQUEST...".into(),
        );
    };
    if !matches!(mode.as_str(), "--preview" | "--apply") {
        return Err("specify --preview or --apply".into());
    }
    let requests = requests
        .iter()
        .map(|s| Uuid::parse_str(s))
        .collect::<Result<Vec<_>, _>>()?;
    let dsn = std::env::var("LINGGAN_LOCAL_DATABASE_URL")?;
    let parsed = url::Url::parse(&dsn).map_err(|_| "invalid local database URL")?;
    if !matches!(parsed.scheme(), "postgresql" | "postgres")
        || !matches!(parsed.host_str(), Some("127.0.0.1" | "[::1]"))
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err("supply a loopback PostgreSQL URL without connection overrides".into());
    }
    let database = Database::connect(&dsn)
        .await
        .map_err(|_| "local database connection failed")?;
    let receipts = recover_saved_replies(&database, &requests, revision, mode == "--apply").await?;
    println!("{}", serde_json::to_string_pretty(&receipts)?);
    Ok(())
}
