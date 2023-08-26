use clap::Parser;
use color_eyre::eyre::Result;
use tonic::transport::Server;
use tracing::{info, Subscriber};
use tracing_subscriber::{prelude::*, registry::LookupSpan, EnvFilter, Layer};

use buildkite_keda_scaler::{BuildkiteMetrics, BuildkiteScaler};

static BUILDKITE_AGENT_API_URL: &str = "https://agent.buildkite.com";
pub type BoxedLayer<S> = Box<dyn Layer<S> + Send + Sync>;

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
    init_tracing();

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

pub fn init_tracing() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    let layers = vec![stdout()];
    tracing_subscriber::registry().with(layers).init();
}

fn stdout<S>() -> BoxedLayer<S>
where
    S: Subscriber,
    for<'a> S: LookupSpan<'a>,
{
    let log_env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("INFO"));

    let json_fmt = std::env::var("RUST_LOG_FORMAT")
        .map(|val| val == "json")
        .unwrap_or(false);

    if json_fmt {
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_target(true)
            .json()
            .with_filter(log_env_filter)
            .boxed()
    } else {
        tracing_subscriber::fmt::layer()
            .with_ansi(true)
            .with_target(true)
            .with_filter(log_env_filter)
            .boxed()
    }
}
