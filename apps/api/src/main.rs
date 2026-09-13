use std::{env, sync::Arc};

use linggan_api::comment_research_router_v0;
use linggan_storage_postgres::CommentFactStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("LINGGAN_DATABASE_URL")
        .map_err(|_| "LINGGAN_DATABASE_URL must be set before starting the API")?;
    let bind_address = env::var("LINGGAN_API_BIND").unwrap_or_else(|_| "127.0.0.1:3000".to_owned());

    let store = Arc::new(CommentFactStore::connect(&database_url).await?);
    let listener = tokio::net::TcpListener::bind(&bind_address).await?;

    println!("Linggan Intelligence API listening on {bind_address}");
    axum::serve(listener, comment_research_router_v0(store)).await?;
    Ok(())
}
