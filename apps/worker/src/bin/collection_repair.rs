//! Operator tool; without --apply it emits a bounded, read-only JSON preview.
use linggan_evidence::collection_repair::{
    RepairPreview, apply_collection_repair, preview_collection_repair_for_task,
};
use std::process::ExitCode;
use uuid::Uuid;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("collection repair: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Parse the entire explicit apply selection before connecting to any database.
    let mut task_id = None;
    let selection = match args.as_slice() {
        [] => None,
        [flag, value] if flag=="--task-id" => { task_id=Some(Uuid::parse_str(value)?); None },
        [mode, file, batch_flag, batch, hash_flag, hash]
            if mode=="--apply" && batch_flag=="--batch-id" && hash_flag=="--preview-hash" => {
                let preview: RepairPreview=serde_json::from_slice(&std::fs::read(file)?)?;
                Some((preview, Uuid::parse_str(batch)?, hash.clone()))
            },
        _ => return Err("usage: linggan-collection-repair [--task-id UUID] [--apply preview.json --batch-id UUID --preview-hash SHA256]".into()),
    };
    let url = std::env::var("LINGGAN_LOCAL_DATABASE_URL")?;
    let database = linggan_storage_postgres::Database::connect(&url).await?;
    if let Some((preview, batch, hash)) = selection {
        // Emit each item's committed result. On a later failure, earlier effects remain observable;
        // repeating the same preview safely skips those changed items.
        let outcomes = apply_collection_repair(&database, &preview, batch, &hash).await?;
        println!(
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({"batchId":batch,"previewHash":hash,"outcomes":outcomes})
            )?
        );
        if outcomes
            .iter()
            .any(|item| item.outcome == "failed_database")
        {
            return Err("one or more items failed; inspect per-item results".into());
        }
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(
                &preview_collection_repair_for_task(&database, task_id).await?
            )?
        );
    }
    Ok(())
}
