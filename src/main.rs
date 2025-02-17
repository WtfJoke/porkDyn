use lambda_http::{run, service_fn, tracing, Error};
mod api;
mod credentials;
mod domain;
mod error;
mod http_handler;

use http_handler::function_handler;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();

    run(service_fn(function_handler)).await
}
