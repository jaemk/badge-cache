use std::pin::Pin;
use std::task::{Context, Poll};

use actix_service::{Service, Transform};
use actix_web::{dev::ServiceRequest, dev::ServiceResponse, Error};
use chrono::Local;
use futures::future::{ok, Ready};
use futures::Future;

use crate::LOG;

pub struct Logger;
impl Logger {
    pub fn new() -> Self {
        Self {}
    }
}

// `S` - type of the next service
// `B` - type of response's body
impl<S, B> Transform<S> for Logger
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = LoggerMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(LoggerMiddleware { service })
    }
}

pub struct LoggerMiddleware<S> {
    service: S,
}

impl<S, B> Service for LoggerMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = Error;
    #[allow(clippy::type_complexity)]
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        let start = Local::now();
        let method = req.method().as_str().to_string();
        let path = req.path().to_string();

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;

            let elapsed = Local::now()
                .signed_duration_since(start)
                .to_std()
                .unwrap_or_else(|e| {
                    slog::error!(LOG, "failed converting to std::time::Duration, {:?}", e);
                    std::time::Duration::from_secs(0)
                });
            let ms =
                (elapsed.as_secs() * 1_000) as f32 + (elapsed.subsec_nanos() as f32 / 1_000_000.);

            slog::info!(
                LOG, "completed request";
                "request_start" => &start.format("%Y-%m-%d_%H:%M:%S").to_string(),
                "method" => &method,
                "status" => res.status().as_u16(),
                "path" => &path,
                "ms" => ms,
            );
            Ok(res)
        })
    }
}
