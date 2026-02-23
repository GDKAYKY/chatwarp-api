#[tokio::main]
async fn main() {
    if let Err(error) = chatwarp_api::run().await {
        eprintln!("fatal: {error}");
        std::process::exit(1);
    }
}
