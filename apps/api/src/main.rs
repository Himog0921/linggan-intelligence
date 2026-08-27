mod local_web;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    local_web::serve().await
}
