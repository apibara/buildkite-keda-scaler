use crate::agent_api::{BuildkiteMetrics, Metrics};

use proto::external_scaler_server::{ExternalScaler, ExternalScalerServer};
use tonic::{codec::Streaming, Request, Response, Status};

use self::proto::{
    GetMetricSpecResponse, GetMetricsRequest, GetMetricsResponse, IsActiveResponse, MetricSpec,
    MetricValue, ScaledObjectRef,
};

pub mod proto {
    tonic::include_proto!("externalscaler");
}

const DEFAULT_TARGET_WAITING_JOBS: i64 = 1;

pub struct BuildkiteScaler {
    client: BuildkiteMetrics,
}

impl BuildkiteScaler {
    pub fn new(client: BuildkiteMetrics) -> Self {
        Self { client }
    }

    pub fn into_service(self) -> ExternalScalerServer<Self> {
        ExternalScalerServer::new(self)
    }
}

#[tonic::async_trait]
impl ExternalScaler for BuildkiteScaler {
    /// Returns true if the number of jobs waiting in the queue is greater than zero.
    async fn is_active(
        &self,
        request: Request<ScaledObjectRef>,
    ) -> Result<Response<IsActiveResponse>, Status> {
        let request = request.into_inner();

        let queue = request.require_queue()?;

        let target_waiting_jobs = request.target_waiting_jobs()?;

        let metrics = self.client.get().await.map_err(IntoStatus::into_status)?;
        let waiting = metrics.job_queue_waiting(&queue);

        let response = IsActiveResponse {
            result: waiting > target_waiting_jobs,
        };

        Ok(Response::new(response))
    }

    type StreamIsActiveStream = Streaming<IsActiveResponse>;

    async fn stream_is_active(
        &self,
        _request: Request<ScaledObjectRef>,
    ) -> Result<Response<Self::StreamIsActiveStream>, Status> {
        Err(Status::unimplemented("scaler is pull only"))
    }

    async fn get_metric_spec(
        &self,
        request: Request<ScaledObjectRef>,
    ) -> Result<Response<GetMetricSpecResponse>, Status> {
        let request = request.into_inner();

        let queue = request.require_queue()?;
        let target_waiting_jobs = request.target_waiting_jobs()?;

        let metric_spec = MetricSpec {
            metric_name: metric_name(&queue),
            target_size: target_waiting_jobs,
        };

        let response = GetMetricSpecResponse {
            metric_specs: vec![metric_spec],
        };

        Ok(Response::new(response))
    }

    async fn get_metrics(
        &self,
        request: Request<GetMetricsRequest>,
    ) -> Result<Response<GetMetricsResponse>, Status> {
        let request = request.into_inner();

        let queue = request
            .scaled_object_ref
            .ok_or_else(|| Status::invalid_argument("missing scaled object ref".to_string()))?
            .require_queue()?;

        let metrics = self.client.get().await.map_err(IntoStatus::into_status)?;
        let waiting = metrics.job_queue_waiting(&queue);

        let metric = MetricValue {
            metric_name: metric_name(&queue),
            metric_value: waiting,
        };

        let response = GetMetricsResponse {
            metric_values: vec![metric],
        };

        Ok(Response::new(response))
    }
}

trait ScaledObjectRefExt {
    fn require_queue(&self) -> Result<String, Status>;
    fn target_waiting_jobs(&self) -> Result<i64, Status>;
}

trait MetricsExt {
    fn job_queue_waiting(&self, queue: &str) -> i64;
}

trait IntoStatus {
    fn into_status(self) -> Status;
}

impl IntoStatus for color_eyre::Report {
    fn into_status(self) -> Status {
        Status::internal(format!("error: {}", self))
    }
}

impl ScaledObjectRefExt for ScaledObjectRef {
    fn require_queue(&self) -> Result<String, Status> {
        self.scaler_metadata
            .get("queue")
            .cloned()
            .ok_or_else(|| Status::invalid_argument("queue not specified"))
    }

    fn target_waiting_jobs(&self) -> Result<i64, Status> {
        Ok(self
            .scaler_metadata
            .get("targetWaitingJobs")
            .map(|target| target.parse())
            .transpose()
            .map_err(|_| Status::invalid_argument("targetWaitingJobs is not a number"))?
            .unwrap_or(DEFAULT_TARGET_WAITING_JOBS))
    }
}

impl MetricsExt for Metrics {
    fn job_queue_waiting(&self, queue: &str) -> i64 {
        self.get_job_queue(queue)
            .map(|queue| queue.waiting)
            .unwrap_or(0)
    }
}

fn metric_name(queue: &str) -> String {
    format!("buildkite-{}", queue)
}
