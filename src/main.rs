use lambda_http::{run, service_fn, tracing, Error};
mod api;
mod credentials;
mod domain;
mod error;
mod http_handler;
mod ip_utils;

use api::PorkbunClient;
use http_handler::function_handler;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();

    let porkbun = PorkbunClient::new();
    run(service_fn(|event| function_handler(event, &porkbun))).await
}
