use clap::Parser;
use color_eyre::eyre::Result;
use tonic::transport::Server;
use tracing::info;

use buildkite_keda_scaler::{BuildkiteMetrics, BuildkiteScaler};

static BUILDKITE_AGENT_API_URL: &str = "https://agent.buildkite.com";

#[derive(Parser, Debug)]
#[command(author, about, version, long_about = None)]
pub struct Cli {
    /// The Buildkite agent token.
    #[arg(long, env = "BUILDKITE_AGENT_TOKEN")]
    pub agent_token: String,
    /// Buildkite agent API URL, defaults to `https://agent.buildkite.com`.
    #[arg(long, env)]
    pub agent_api_url: Option<String>,
    /// The address to listen on. Defaults to `0.0.0.0:9090`.
    #[arg(long, env)]
    pub address: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Cli::parse();

    let client = BuildkiteMetrics::new(
        args.agent_api_url
            .unwrap_or_else(|| BUILDKITE_AGENT_API_URL.to_string()),
        Some(args.agent_token),
    );

    let scaler = BuildkiteScaler::new(client);

    let address = args.address.unwrap_or("0.0.0.0:9090".to_string()).parse()?;
    info!("listening on {}", address);
    Server::builder()
        .add_service(scaler.into_service())
        .serve(address)
        .await?;

    Ok(())
}
