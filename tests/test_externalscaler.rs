use std::{collections::HashMap, future::Future, net::SocketAddr, time::Duration};

use buildkite_keda_scaler::{
    externalscaler::proto::{
        external_scaler_client::ExternalScalerClient, GetMetricsRequest, ScaledObjectRef,
    },
    BuildkiteMetrics, BuildkiteScaler,
};
use color_eyre::Result;
use rand::Rng;
use serde_json::json;
use tonic::{
    transport::{Channel, Server},
    Request,
};
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

#[tokio::test]
async fn test_is_active() -> Result<()> {
    let (server, client) = setup().await?;

    let test = async {
        let mut client = client.await;

        {
            // default queue has no waiting jobs, so it's not active
            let scaler_metadata = HashMap::from([("queue".to_string(), "default".to_string())]);
            let request = Request::new(ScaledObjectRef {
                namespace: "test".to_string(),
                name: "test".to_string(),
                scaler_metadata,
            });

            let response = client.is_active(request).await.unwrap().into_inner();
            assert!(!response.result);
        }

        {
            // large queue has jobs waiting, so it's active
            let scaler_metadata = HashMap::from([("queue".to_string(), "large".to_string())]);
            let request = Request::new(ScaledObjectRef {
                namespace: "test".to_string(),
                name: "test".to_string(),
                scaler_metadata,
            });

            let response = client.is_active(request).await.unwrap().into_inner();
            assert!(response.result);
        }

        {
            // customize target waiting jobs
            let scaler_metadata = HashMap::from([
                ("queue".to_string(), "large".to_string()),
                ("targetWaitingJobs".to_string(), "100".to_string()),
            ]);

            let request = Request::new(ScaledObjectRef {
                namespace: "test".to_string(),
                name: "test".to_string(),
                scaler_metadata,
            });

            let response = client.is_active(request).await.unwrap().into_inner();
            assert!(!response.result);
        }

        {
            // queue is required
            let scaler_metadata = HashMap::from([]);

            let request = Request::new(ScaledObjectRef {
                namespace: "test".to_string(),
                name: "test".to_string(),
                scaler_metadata,
            });

            let response = client.is_active(request).await;
            assert!(response.is_err());
        }
    };

    tokio::select! {
        _ = server => panic!("server exited"),
        _ = test => (),
    }

    Ok(())
}

#[tokio::test]
async fn test_get_metrics_spec() -> Result<()> {
    let (server, client) = setup().await?;

    let test = async {
        let mut client = client.await;

        {
            // queue is required
            let scaler_metadata = HashMap::from([]);

            let request = Request::new(ScaledObjectRef {
                namespace: "test".to_string(),
                name: "test".to_string(),
                scaler_metadata,
            });

            let response = client.get_metric_spec(request).await;
            assert!(response.is_err());
        }

        {
            // defaults to non zero target waiting jobs
            let scaler_metadata = HashMap::from([("queue".to_string(), "default".to_string())]);

            let request = Request::new(ScaledObjectRef {
                namespace: "test".to_string(),
                name: "test".to_string(),
                scaler_metadata,
            });

            let response = client.get_metric_spec(request).await.unwrap().into_inner();
            assert!(response.metric_specs.len() == 1);
            let spec = response.metric_specs.first().unwrap();
            assert!(spec.target_size > 0);
            assert!(!spec.metric_name.is_empty());
        }

        {
            // customize target size
            let scaler_metadata = HashMap::from([
                ("queue".to_string(), "default".to_string()),
                ("targetWaitingJobs".to_string(), "23".to_string()),
            ]);

            let request = Request::new(ScaledObjectRef {
                namespace: "test".to_string(),
                name: "test".to_string(),
                scaler_metadata,
            });

            let response = client.get_metric_spec(request).await.unwrap().into_inner();
            assert!(response.metric_specs.len() == 1);
            let spec = response.metric_specs.first().unwrap();
            assert_eq!(spec.target_size, 23);
        }
    };

    tokio::select! {
        _ = server => panic!("server exited"),
        _ = test => (),
    }

    Ok(())
}

#[tokio::test]
async fn test_get_metrics() -> Result<()> {
    let (server, client) = setup().await?;

    let test = async {
        let mut client = client.await;

        {
            // queue is required
            let scaler_metadata = HashMap::from([]);

            let object_ref = ScaledObjectRef {
                namespace: "test".to_string(),
                name: "test".to_string(),
                scaler_metadata,
            };
            let request = Request::new(GetMetricsRequest {
                scaled_object_ref: Some(object_ref),
                metric_name: "buildkite-default".to_string(),
            });

            let response = client.get_metrics(request).await;
            assert!(response.is_err());
        }

        {
            // scaled object ref is required
            let request = Request::new(GetMetricsRequest {
                scaled_object_ref: None,
                metric_name: "buildkite-default".to_string(),
            });

            let response = client.get_metrics(request).await;
            assert!(response.is_err());
        }

        {
            // 0 jobs waiting from buildkite
            let scaler_metadata = HashMap::from([("queue".to_string(), "default".to_string())]);

            let object_ref = ScaledObjectRef {
                namespace: "test".to_string(),
                name: "test".to_string(),
                scaler_metadata,
            };
            let request = Request::new(GetMetricsRequest {
                scaled_object_ref: Some(object_ref),
                metric_name: "buildkite-default".to_string(),
            });

            let response = client.get_metrics(request).await.unwrap().into_inner();
            assert!(response.metric_values.len() == 1);
            let metrics = response.metric_values.first().unwrap();
            assert_eq!(metrics.metric_value, 0);
        }

        {
            // non 0 jobs waiting from buildkite
            let scaler_metadata = HashMap::from([("queue".to_string(), "large".to_string())]);

            let object_ref = ScaledObjectRef {
                namespace: "test".to_string(),
                name: "test".to_string(),
                scaler_metadata,
            };
            let request = Request::new(GetMetricsRequest {
                scaled_object_ref: Some(object_ref),
                metric_name: "buildkite-default".to_string(),
            });

            let response = client.get_metrics(request).await.unwrap().into_inner();
            assert!(response.metric_values.len() == 1);
            let metrics = response.metric_values.first().unwrap();
            assert_eq!(metrics.metric_value, 5);
        }

        {
            // queue with no jobs waiting
            let scaler_metadata = HashMap::from([("queue".to_string(), "missing".to_string())]);

            let object_ref = ScaledObjectRef {
                namespace: "test".to_string(),
                name: "test".to_string(),
                scaler_metadata,
            };
            let request = Request::new(GetMetricsRequest {
                scaled_object_ref: Some(object_ref),
                metric_name: "buildkite-default".to_string(),
            });

            let response = client.get_metrics(request).await.unwrap().into_inner();
            assert!(response.metric_values.len() == 1);
            let metrics = response.metric_values.first().unwrap();
            assert_eq!(metrics.metric_value, 0);
        }
    };

    tokio::select! {
        _ = server => panic!("server exited"),
        _ = test => (),
    }

    Ok(())
}

async fn mock_metrics(server: &MockServer) {
    let body = json!({
        "agents": {
            "idle": 1,
            "busy": 2,
            "total": 3,
            "queues": {
                "default": {
                    "idle": 1,
                    "busy": 0,
                    "total": 1,
                },
                "small": {
                    "idle": 0,
                    "busy": 1,
                    "total": 1,
                },
                "large": {
                    "idle": 0,
                    "busy": 1,
                    "total": 1,
                },
            },
        },
        "jobs": {
            "scheduled": 0,
            "running": 0,
            "waiting": 0,
            "total": 0,
            "queues": {
                "default": {
                    "scheduled": 0,
                    "running": 0,
                    "waiting": 0,
                    "total": 0,
                },
                "small": {
                    "scheduled": 0,
                    "running": 0,
                    "waiting": 1,
                    "total": 1,
                },
                "large": {
                    "scheduled": 0,
                    "running": 0,
                    "waiting": 5,
                    "total": 5,
                },
            },
        },
        "organization": {
            "slug": "test"
        },
    });

    Mock::given(method("GET"))
        .and(path("/v3/metrics"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

async fn setup() -> Result<(
    impl Future<Output = ()>,
    impl Future<Output = ExternalScalerClient<Channel>>,
)> {
    let auth_token = "test_token".to_string();

    let metrics = MockServer::start().await;
    mock_metrics(&metrics).await;

    let client = BuildkiteMetrics::new(metrics.uri(), Some(auth_token));
    let scaler = BuildkiteScaler::new(client);
    let mut rng = rand::thread_rng();
    let port = rng.gen_range(9_000..10_000);
    let address: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

    let server = {
        let address = address;
        async move {
            let result = Server::builder()
                .add_service(scaler.into_service())
                .serve(address)
                .await;
            assert!(result.is_ok());
        }
    };

    let client = async move {
        // give time to the server to start
        tokio::time::sleep(Duration::from_secs(1)).await;
        ExternalScalerClient::connect(format!("http://127.0.0.1:{}", address.port()))
            .await
            .unwrap()
    };

    Ok((server, client))
}
