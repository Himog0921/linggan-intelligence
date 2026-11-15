use linggan_intelligence::ai_foundation::content_understanding::{
    ReviewedCore, publish_reviewed_core,
};
use linggan_storage_postgres::Database;
use sha2::{Digest, Sha256};
use std::io::Read as _;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let (apply, expected_sha256) = match args.as_slice() {
        [mode, digest] if mode == "--preview" => (false, digest),
        [mode, digest] if mode == "--apply" => (true, digest),
        _ => {
            return Err(
                "choose --preview or --apply and supply the reviewed packet SHA-256".into(),
            );
        }
    };
    let url = std::env::var("LINGGAN_LOCAL_DATABASE_URL")?;
    let parsed = url::Url::parse(&url).map_err(|_| "invalid database URL")?;
    if parsed.scheme() != "postgresql"
        || parsed.host_str() != Some("127.0.0.1")
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !matches!(
            (parsed.path(), parsed.port()),
            ("/proof", Some(port)) if port != 55432
        ) && (parsed.path(), parsed.port()) != ("/linggan_intelligence_dev", Some(55432))
    {
        return Err("unexpected local database identity".into());
    }
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let digest: String = Sha256::digest(input.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if digest != *expected_sha256 {
        return Err("reviewed packet checksum mismatch".into());
    }
    let reviewed: ReviewedCore = serde_json::from_str(&input)?;
    let db = Database::connect(&url)
        .await
        .map_err(|_| "local database connection failed")?;
    let receipt = publish_reviewed_core(&db, &reviewed, apply).await?;
    println!("{}", serde_json::to_string(&receipt)?);
    Ok(())
}
