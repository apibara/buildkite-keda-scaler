use std::collections::HashMap;

use color_eyre::Result;
use serde::{Deserialize, Serialize};
use tracing::instrument;

/// Buildkite metrics API client.
#[derive(Debug)]
pub struct BuildkiteMetrics {
    client: reqwest::Client,
    base_url: String,
    token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JobQueue {
    pub scheduled: i64,
    pub running: i64,
    pub waiting: i64,
    pub total: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JobMetrics {
    pub scheduled: i64,
    pub running: i64,
    pub waiting: i64,
    pub total: i64,
    pub queues: HashMap<String, JobQueue>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentQueue {
    pub idle: i64,
    pub busy: i64,
    pub total: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentMetrics {
    pub idle: i64,
    pub busy: i64,
    pub total: i64,
    pub queues: HashMap<String, AgentQueue>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Organization {
    pub slug: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Metrics {
    pub jobs: JobMetrics,
    pub agents: AgentMetrics,
    pub organization: Organization,
}

impl BuildkiteMetrics {
    pub fn new(base_url: impl Into<String>, token: impl Into<Option<String>>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into(),
            token: token.into(),
        }
    }

    /// Get metrics from the Buildkite API.
    #[instrument(skip(self), err(Debug))]
    pub async fn get(&self) -> Result<Metrics> {
        let url = format!("{}/v3/metrics", self.base_url);
        let response = self
            .client
            .get(url)
            .authorization(&self.token)
            .send()
            .await?;

        let metrics = response.json::<Metrics>().await?;
        Ok(metrics)
    }
}

trait RequestBuilderExt {
    fn authorization(self, token: &Option<String>) -> Self;
}

impl RequestBuilderExt for reqwest::RequestBuilder {
    fn authorization(self, token: &Option<String>) -> Self {
        if let Some(token) = token {
            self.header("Authorization", format!("Token {}", token))
        } else {
            self
        }
    }
}

impl Metrics {
    pub fn get_job_queue(&self, queue: &str) -> Option<&JobQueue> {
        self.jobs.queues.get(queue)
    }
}
