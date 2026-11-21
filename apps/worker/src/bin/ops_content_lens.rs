//! Operator entry point for publishing the content lens taxonomy.
use linggan_intelligence::ops_content_lens::load_taxonomy;
use linggan_storage_postgres::Database;
use std::process::ExitCode;

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

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [command, flag, file] = args.as_slice() else {
        return Err("usage: linggan-ops-content-lens load-taxonomy --file FILE".into());
    };
    if command != "load-taxonomy" || flag != "--file" {
        return Err("usage: linggan-ops-content-lens load-taxonomy --file FILE".into());
    }
    let body = std::fs::read(file)?;
    let database = Database::connect(&std::env::var("LINGGAN_LOCAL_DATABASE_URL")?).await?;
    let receipt = load_taxonomy(&database, &body).await?;
    println!("{}", serde_json::to_string(&receipt)?);
    Ok(())
}
