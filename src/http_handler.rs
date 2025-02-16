use crate::api::{create_dns_record, get_existing_record, update_dns_record};
use lambda_http::tracing::{error, info};
use lambda_http::{Body, Error, Request, RequestExt, Response};
use reqwest::Client;

/// This function is the entry point for the Lambda function.
/// It receives a request with query parameters and updates the DNS record for the given domain and subdomain.
/// If the record does not exist, it creates a new one.
///
/// Following query-parameters are required:
/// - apikey: The API key for the porkbun API
/// - secretapikey: The secret API key for the porkbun API
/// - domain: The domain for which the DNS record should be updated
/// - ip: The IP address to which the DNS record should be updated
pub(crate) async fn function_handler(event: Request) -> Result<Response<Body>, Error> {
    // Extract query parameters
    info!("Validating request");
    let query_params = event.query_string_parameters();
    let api_key = match query_params.first("apikey") {
        Some(query_param) => query_param,
        None => return Ok(json_response(400, "Missing query-parameter 'apikey'")),
    };
    let secret_key = match query_params.first("secretapikey") {
        Some(query_param) => query_param,
        None => return Ok(json_response(400, "Missing query-parameter 'secretapikey'")),
    };
    let subdomain_with_domain = match query_params.first("domain") {
        Some(query_param) => query_param,
        None => return Ok(json_response(400, "Missing query-parameter 'domain'")),
    };
    let ip = match query_params.first("ip") {
        Some(query_param) => query_param,
        None => return Ok(json_response(400, "Missing query-parameter 'ip'")),
    };
    info!(
        "Valid request received for updating the dns-entry for domain: '{:?}' to ip: '{:?}'.",
        subdomain_with_domain, ip
    );

    // Extract domain and subdomain
    let domain_parts: Vec<&str> = subdomain_with_domain.split('.').collect();
    if domain_parts.len() < 3 {
        return Ok(json_response(400, "Invalid subdomain format"));
    }
    let domain_name = format!(
        "{}.{}",
        domain_parts[domain_parts.len() - 2],
        domain_parts.last().unwrap()
    );
    let subdomain = domain_parts[..domain_parts.len() - 2].join(".");
    info!("DomainName: {:?}, Subdomain: {:?}", domain_name, subdomain);

    let client = Client::new();

    // Check if the record exists
    match get_existing_record(
        &client,
        api_key,
        secret_key,
        &domain_name,
        &subdomain_with_domain,
    )
    .await
    {
        Ok(Some(record)) => {
            if record.content == ip {
                info!(
                    "Skip updating, record with id {:?} is already up to date.",
                    record.id
                );
                return Ok(json_response(
                    200,
                    &format!("DNS record '{:?}' is already up to date", record.name),
                ));
            }

            info!("Going to update existing DNS record '{:?}' of domain {:?} with subdomain: {:?} and IP: {:?}.", record, domain_name, subdomain, ip);
            update_dns_record(
                &client,
                api_key,
                secret_key,
                &domain_name,
                &subdomain,
                &record.id,
                &ip,
            )
            .await?;
            Ok(json_response(
                200,
                &format!("DNS record '{:?}' updated successfully", record.name),
            ))
        }
        Ok(None) => {
            info!(
                "Going to create a new DNS record - DomainName: {:?}, Subdomain: {:?}, IP: {:?}",
                domain_name, subdomain, ip
            );
            create_dns_record(&client, api_key, secret_key, &domain_name, &subdomain, &ip).await?;
            Ok(json_response(
                200,
                &format!(
                    "DNS record for subdomain '{:?}' successfully created",
                    subdomain
                ),
            ))
        }
        Err(e) => {
            error!(
                "Failed to retrieve records for domainName {:?}: {:?}",
                domain_name, e
            );
            Ok(json_response(500, "Failed to retrieve DNS records"))
        }
    }
}

// Helper function to generate JSON responses
fn json_response(status_code: u16, message: &str) -> Response<Body> {
    Response::builder()
        .status(status_code)
        .header("Content-Type", "application/json")
        .body(Body::Text(format!("{{\"message\": \"{}\"}}", message)))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lambda_http::{Request, RequestExt};
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_without_query_strings() {
        let request = Request::default();

        let response = function_handler(request).await.unwrap();
        assert_eq!(response.status(), 400);

        let body_bytes = response.body().to_vec();
        let body_string = String::from_utf8(body_bytes).unwrap();
        let body_json = serde_json::from_str::<serde_json::Value>(&body_string).unwrap();

        assert_eq!(
            body_json["message"].as_str().unwrap(),
            "Missing query-parameter 'apikey'"
        );
    }

    #[tokio::test]
    async fn test_with_missing_secret_api_key() {
        let mut query_string_parameters: HashMap<String, String> = HashMap::new();
        query_string_parameters.insert("apikey".into(), "porkDyn".into());

        let request = Request::default().with_query_string_parameters(query_string_parameters);

        let response = function_handler(request).await.unwrap();
        assert_eq!(response.status(), 400);

        let body_bytes = response.body().to_vec();
        let body_string = String::from_utf8(body_bytes).unwrap();
        let body_json = serde_json::from_str::<serde_json::Value>(&body_string).unwrap();

        assert_eq!(
            body_json["message"].as_str().unwrap(),
            "Missing query-parameter 'secretapikey'"
        );
    }

    #[tokio::test]
    async fn test_with_missing_domain() {
        let mut query_string_parameters: HashMap<String, String> = HashMap::new();
        query_string_parameters.insert("apikey".into(), "porkDyn".into());
        query_string_parameters.insert("secretapikey".into(), "secret".into());

        let request = Request::default().with_query_string_parameters(query_string_parameters);

        let response = function_handler(request).await.unwrap();
        assert_eq!(response.status(), 400);

        let body_bytes = response.body().to_vec();
        let body_string = String::from_utf8(body_bytes).unwrap();
        let body_json = serde_json::from_str::<serde_json::Value>(&body_string).unwrap();

        assert_eq!(
            body_json["message"].as_str().unwrap(),
            "Missing query-parameter 'domain'"
        );
    }

    #[tokio::test]
    async fn test_with_missing_ip() {
        let mut query_string_parameters: HashMap<String, String> = HashMap::new();
        query_string_parameters.insert("apikey".into(), "porkDyn".into());
        query_string_parameters.insert("secretapikey".into(), "secret".into());
        query_string_parameters.insert("domain".into(), "me.example.org".into());

        let request = Request::default().with_query_string_parameters(query_string_parameters);

        let response = function_handler(request).await.unwrap();
        assert_eq!(response.status(), 400);

        let body_bytes = response.body().to_vec();
        let body_string = String::from_utf8(body_bytes).unwrap();
        let body_json = serde_json::from_str::<serde_json::Value>(&body_string).unwrap();

        assert_eq!(
            body_json["message"].as_str().unwrap(),
            "Missing query-parameter 'ip'"
        );
    }
}
