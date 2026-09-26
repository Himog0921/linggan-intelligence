//! Bounded operator tool: preview by default, project original accepted packages with --apply.
use linggan_evidence::reproject_accepted_target_materials;
use std::process::ExitCode;
use uuid::Uuid;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("material reproject: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (target_ref, domain_ref, apply) = match args.as_slice() {
        [target_flag, target, domain_flag, domain]
            if target_flag == "--target" && domain_flag == "--domain" =>
        {
            (Uuid::parse_str(target)?, Uuid::parse_str(domain)?, false)
        }
        [target_flag, target, domain_flag, domain, apply_flag]
            if target_flag == "--target"
                && domain_flag == "--domain"
                && apply_flag == "--apply" =>
        {
            (Uuid::parse_str(target)?, Uuid::parse_str(domain)?, true)
        }
        _ => {
            return Err(
                "usage: linggan-material-reproject --target UUID --domain UUID [--apply]".into(),
            );
        }
    };
    let url = std::env::var("LINGGAN_LOCAL_DATABASE_URL")?;
    let database = linggan_storage_postgres::Database::connect(&url).await?;
    let report =
        reproject_accepted_target_materials(&database, target_ref, domain_ref, apply).await?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
