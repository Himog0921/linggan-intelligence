//! Explicit operator command. Preview is read-only; no normal runtime worker is started.
use linggan_intelligence::content_workbench::recovery::{self, RecoveryPreview};
use linggan_storage_postgres::Database;
use std::process::ExitCode;
use uuid::Uuid;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("creation recovery: {e}");
            ExitCode::FAILURE
        }
    }
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    enum Mode {
        Preview(Uuid),
        Apply(RecoveryPreview, String, Uuid),
    }
    let mode=match args.as_slice() {
        [flag,id] if flag=="--work-id"=>Mode::Preview(Uuid::parse_str(id)?),
        [flag,file,hash_flag,hash,request_flag,request] if flag=="--apply" && hash_flag=="--preview-hash" && request_flag=="--request-id"=>{
            let preview:RecoveryPreview=serde_json::from_slice(&std::fs::read(file)?)?;
            if preview.sha256()?!=*hash {return Err("preview hash mismatch".into());}
            Mode::Apply(preview,hash.clone(),Uuid::parse_str(request)?)
        },
        _=>return Err("usage: linggan-creation-recovery --work-id UUID | --apply preview.json --preview-hash SHA256 --request-id UUID".into()),
    };
    // A dedicated explicit DSN; never auto-load the running service's .env or worker settings.
    let url = std::env::var("LINGGAN_CREATION_RECOVERY_DATABASE_URL")?;
    let db = Database::connect(&url).await?;
    match mode {
        Mode::Preview(work) => {
            let p = recovery::preview(&db, work).await?;
            eprintln!("preview sha256: {}", p.sha256()?);
            println!("{}", serde_json::to_string_pretty(&p)?);
        }
        Mode::Apply(p, hash, request) => println!(
            "{}",
            serde_json::to_string_pretty(&recovery::apply(&db, &p, &hash, request).await?)?
        ),
    }
    Ok(())
}
