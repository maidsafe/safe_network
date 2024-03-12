// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{NetworkError, Result};
use futures::Future;
use hyper::{service::Service, Body, Method, Request, Response, Server, StatusCode};
use prometheus_client::{encoding::text::encode, registry::Registry};
use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

const METRICS_CONTENT_TYPE: &str = "application/openmetrics-text;charset=utf-8;version=1.0.0";

pub(crate) fn run_metrics_server(registry: Registry, port: u16) {
    // The server should not bind to localhost/127.0.0.1 as it will not accept connections from containers.
    let addr = ([0, 0, 0, 0], port).into();

    tokio::spawn(async move {
        let server = Server::bind(&addr).serve(MakeMetricService::new(registry));
        info!("Metrics server on http://{}/metrics", server.local_addr());
        println!("Metrics server on http://{}/metrics", server.local_addr());
        // run the server forever
        if let Err(e) = server.await {
            error!("server error: {}", e);
        }
    });
}

pub(crate) struct MetricService {
    reg: Arc<Mutex<Registry>>,
}

type SharedRegistry = Arc<Mutex<Registry>>;

impl MetricService {
    fn get_reg(&mut self) -> SharedRegistry {
        Arc::clone(&self.reg)
    }
    fn respond_with_metrics(&mut self) -> Result<Response<String>> {
        let mut response: Response<String> = Response::default();

        response.headers_mut().insert(
            hyper::header::CONTENT_TYPE,
            METRICS_CONTENT_TYPE
                .try_into()
                .map_err(|_| NetworkError::NetworkMetricError)?,
        );

        let reg = self.get_reg();
        let reg = reg.lock().map_err(|_| NetworkError::NetworkMetricError)?;
        encode(&mut response.body_mut(), &reg).map_err(|err| {
            error!("Failed to encode the metrics Registry {err:?}");
            NetworkError::NetworkMetricError
        })?;

        *response.status_mut() = StatusCode::OK;

        Ok(response)
    }

    fn respond_with_404_not_found(&mut self) -> Response<String> {
        let mut resp = Response::default();
        *resp.status_mut() = StatusCode::NOT_FOUND;
        *resp.body_mut() = "Not found try localhost:[port]/metrics".to_string();
        resp
    }

    fn respond_with_500_server_error(&mut self) -> Response<String> {
        let mut resp = Response::default();
        *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        *resp.body_mut() = "Something went wrong with the Metrics server".to_string();
        resp
    }
}

impl Service<Request<Body>> for MetricService {
    type Response = Response<String>;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let req_path = req.uri().path();
        let req_method = req.method();
        let resp = if (req_method == Method::GET) && (req_path == "/metrics") {
            // Encode and serve metrics from registry.
            match self.respond_with_metrics() {
                Ok(resp) => resp,
                Err(_) => self.respond_with_500_server_error(),
            }
        } else {
            self.respond_with_404_not_found()
        };
        Box::pin(async { Ok(resp) })
    }
}

pub(crate) struct MakeMetricService {
    reg: SharedRegistry,
}

impl MakeMetricService {
    pub(crate) fn new(registry: Registry) -> MakeMetricService {
        MakeMetricService {
            reg: Arc::new(Mutex::new(registry)),
        }
    }
}

impl<T> Service<T> for MakeMetricService {
    type Response = MetricService;
    type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: T) -> Self::Future {
        let reg = self.reg.clone();
        let fut = async move { Ok(MetricService { reg }) };
        Box::pin(fut)
    }
}
