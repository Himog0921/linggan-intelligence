//! Rebuild deterministic source search entries without patrol, collection, or AI execution.
use linggan_intelligence::comment_study_catalog::{
    CatalogSummaryQuery, read_catalog_summary, refresh_clean_cache,
};
use linggan_storage_postgres::Database;
use serde_json::json;
use std::process::ExitCode;
use uuid::Uuid;
#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(value) => {
            println!("{value}");
            ExitCode::SUCCESS
        }
        Err(_) => {
            eprintln!("comment catalog: rebuild failed; no collection or AI was started");
            ExitCode::FAILURE
        }
    }
}
async fn run() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (domain, apply) = match args.as_slice() {
        [flag, domain] if flag == "--domain" => (domain, false),
        [flag, domain, apply] if flag == "--domain" && apply == "--apply" => (domain, true),
        _ => return Err("usage: linggan-comment-catalog --domain UUID [--apply]".into()),
    };
    let domain = Uuid::parse_str(domain)?;
    let db = Database::connect(&std::env::var("LINGGAN_LOCAL_DATABASE_URL")?).await?;
    let query: CatalogSummaryQuery = serde_json::from_value(json!({"domain":domain}))?;
    let before = read_catalog_summary(&db, &query).await?;
    let mut inserted = 0;
    let mut complete = !apply;
    if apply {
        for _ in 0..1000 {
            let pass = refresh_clean_cache(&db, domain, 200).await?;
            inserted += pass.inserted_count;
            if pass.examined_count == 0 {
                complete = true;
                break;
            }
        }
    }
    if !complete {
        return Err("bounded rebuild exhausted; inspect remaining index coverage".into());
    }
    let catalog = if apply {
        read_catalog_summary(&db, &query).await?
    } else {
        before
    };
    Ok(
        json!({"operation":"comment_catalog_rebuild","applied":apply,"insertedCount":inserted,"catalog":catalog,"aiCalls":0,"collectionStarted":false}),
    )
}
