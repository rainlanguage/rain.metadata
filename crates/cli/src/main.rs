#[cfg(feature = "tokio-full")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rain_metadata::cli::main()
}

#[cfg(not(feature = "tokio-full"))]
#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    rain_metadata::cli::main()
}
