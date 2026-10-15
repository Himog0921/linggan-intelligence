#![cfg_attr(test, recursion_limit = "256")]

mod local_web;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    local_web::serve().await
}
