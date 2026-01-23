use crate::api::{create_dns_record, get_existing_dns_record, update_dns_record};
use crate::credentials::Credentials;
use crate::domain::Domain;
use crate::ip_utils::{validate_and_classify_ip, IpType, RecordType};
use lambda_http::tracing::{error, info};
use lambda_http::{Body, Error, Request, RequestExt, Response};
use reqwest::Client;

#[derive(Debug)]
struct IpUpdate {
    address: String,
    record_type: RecordType,
}

impl IpUpdate {
    fn new(address: String, ip_type: IpType) -> Self {
        Self {
            address,
            record_type: RecordType::from(ip_type),
        }
    }
}

/// This function is the entry point for the Lambda function.
/// It receives a request with query parameters and updates the DNS record for the given domain and subdomain.
/// If the record does not exist, it creates a new one.
///
/// Following query-parameters are required:
/// - apikey: The API key for the porkbun API
/// - secretapikey: The secret API key for the porkbun API
/// - domain: The domain for which the DNS record should be updated
/// - ip: The IPv4 address to which the DNS A record should be updated
/// - ipv6: The IPv6 address to which the DNS AAAA record should be updated (optional)
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
    let qualified_domain_name = match query_params.first("domain") {
        Some(query_param) => query_param,
        None => return Ok(json_response(400, "Missing query-parameter 'domain'")),
    };
    // Process IPv4 address (required)
    let ipv4: Option<IpUpdate> = match query_params.first("ip") {
        Some(ip_str) => match validate_and_classify_ip(ip_str) {
            Ok(IpType::V4) => Some(IpUpdate::new(ip_str.to_string(), IpType::V4)),
            Ok(IpType::V6) => {
                error!("IPv6 address provided in 'ip' parameter, use 'ipv6' parameter instead");
                return Ok(json_response(
                    400,
                    "IPv6 address provided in 'ip' parameter, use 'ipv6' parameter instead",
                ));
            }
            Err(e) => {
                error!("Invalid IPv4 address provided: {:?}", e);
                return Ok(json_response(400, &format!("Invalid IPv4 address: {}", e)));
            }
        },
        None => None,
    };

    // Process IPv6 address (optional)
    let ipv6: Option<IpUpdate> = match query_params.first("ipv6") {
        Some(ip_str) => match validate_and_classify_ip(ip_str) {
            Ok(IpType::V6) => Some(IpUpdate::new(ip_str.to_string(), IpType::V6)),
            Ok(IpType::V4) => {
                error!("IPv4 address provided in 'ipv6' parameter, use 'ip' parameter instead");
                return Ok(json_response(
                    400,
                    "IPv4 address provided in 'ipv6' parameter, use 'ip' parameter instead",
                ));
            }
            Err(e) => {
                error!("Invalid IPv6 address provided: {:?}", e);
                return Ok(json_response(400, &format!("Invalid IPv6 address: {}", e)));
            }
        },
        None => None,
    };

    // Ensure at least one IP address is provided
    if ipv4.is_none() && ipv6.is_none() {
        return Ok(json_response(
            400,
            "At least one IP address must be provided (ip or ipv6)",
        ));
    }

    let ip_updates = [ipv4, ipv6].into_iter().flatten().collect::<Vec<_>>();

    info!(
        "Valid request received for updating DNS entries for domain: '{:?}' with {} IP address(es)",
        qualified_domain_name,
        ip_updates.len()
    );

    // Extract domain
    let domain: Domain = match Domain::new(qualified_domain_name) {
        Ok(domain) => {
            info!("Domain: {:?}", domain);
            domain
        }
        Err(e) => {
            error!("Invalid subdomain format: {:?}", e);
            return Ok(json_response(400, "Invalid subdomain format"));
        }
    };

    let credentials = Credentials::new(api_key.to_string(), secret_key.to_string());
    let client = Client::new();

    let mut results = Vec::new();

    // Process each IP address
    for ip_update in ip_updates {
        let result = process_dns_record(
            &client,
            &credentials,
            &domain,
            &ip_update.address,
            &ip_update.record_type,
        )
        .await;

        match result {
            Ok(message) => {
                results.push(message);
            }
            Err(e) => {
                error!(
                    "Failed to process {} record: {:?}",
                    ip_update.record_type.as_str(),
                    e
                );
                return Ok(json_response(
                    500,
                    &format!(
                        "Failed to process {} record",
                        ip_update.record_type.as_str()
                    ),
                ));
            }
        }
    }

    let success_message = results.join("; ");
    Ok(json_response(200, &success_message))
}

async fn process_dns_record(
    client: &Client,
    credentials: &Credentials,
    domain: &Domain,
    ip: &str,
    record_type: &RecordType,
) -> Result<String, Box<dyn std::error::Error>> {
    // Check if the record exists
    match get_existing_dns_record(client, credentials, domain, record_type).await {
        // If the record exists and the IP is the same, do nothing and return a success message
        Ok(Some(record)) if record.content == ip => {
            info!(
                "Skip updating, {} record with id {:?} is already up to date.",
                record_type.as_str(),
                record.id
            );
            Ok(format!(
                "{} record {:?} is already up to date",
                record_type.as_str(),
                record.name
            ))
        }
        // If the record exists and the IP is different, update the record
        Ok(Some(record)) => {
            info!(
                "Updating {} DNS record {:?} for domain {:?} with subdomain {:?} to IP {:?}",
                record_type.as_str(),
                record,
                domain.domain_name(),
                domain.subdomain(),
                ip
            );
            update_dns_record(client, credentials, domain, &record.id, ip, record_type).await?;
            Ok(format!(
                "{} record '{:?}' updated successfully",
                record_type.as_str(),
                record.name
            ))
        }
        // If the record does not exist, create a new one
        Ok(None) => {
            info!(
                "Creating new {} DNS record for domain {:?} with subdomain {:?} and IP {:?}",
                record_type.as_str(),
                domain.domain_name(),
                domain.subdomain(),
                ip
            );
            create_dns_record(client, credentials, domain, ip, record_type).await?;
            Ok(format!(
                "{} record for subdomain '{:?}' successfully created",
                record_type.as_str(),
                domain.subdomain()
            ))
        }
        // If there is an error, propagate it
        Err(e) => {
            error!(
                "Failed to retrieve {} records for domain {:?}: {:?}",
                record_type.as_str(),
                domain.domain_name(),
                e
            );
            Err(Box::new(e))
        }
    }
}

fn json_response(status_code: u16, message: &str) -> Response<Body> {
    let response_body = serde_json::json!({
        "message": message
    });
    Response::builder()
        .status(status_code)
        .header("Content-Type", "application/json")
        .body(Body::Text(response_body.to_string()))
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
            "At least one IP address must be provided (ip or ipv6)"
        );
    }

    #[tokio::test]
    async fn test_with_invalid_ipv4() {
        let mut query_string_parameters: HashMap<String, String> = HashMap::new();
        query_string_parameters.insert("apikey".into(), "porkDyn".into());
        query_string_parameters.insert("secretapikey".into(), "secret".into());
        query_string_parameters.insert("domain".into(), "me.example.org".into());
        query_string_parameters.insert("ip".into(), "invalid_ip".into());

        let request = Request::default().with_query_string_parameters(query_string_parameters);

        let response = function_handler(request).await.unwrap();
        assert_eq!(response.status(), 400);

        let body_bytes = response.body().to_vec();
        let body_string = String::from_utf8(body_bytes).unwrap();
        let body_json = serde_json::from_str::<serde_json::Value>(&body_string).unwrap();

        assert!(body_json["message"]
            .as_str()
            .unwrap()
            .contains("Invalid IPv4 address"));
    }

    #[tokio::test]
    async fn test_with_invalid_ipv6() {
        let mut query_string_parameters: HashMap<String, String> = HashMap::new();
        query_string_parameters.insert("apikey".into(), "porkDyn".into());
        query_string_parameters.insert("secretapikey".into(), "secret".into());
        query_string_parameters.insert("domain".into(), "me.example.org".into());
        query_string_parameters.insert("ipv6".into(), "invalid_ipv6".into());

        let request = Request::default().with_query_string_parameters(query_string_parameters);

        let response = function_handler(request).await.unwrap();
        assert_eq!(response.status(), 400);

        let body_bytes = response.body().to_vec();
        let body_string = String::from_utf8(body_bytes).unwrap();
        let body_json = serde_json::from_str::<serde_json::Value>(&body_string).unwrap();

        assert!(body_json["message"]
            .as_str()
            .unwrap()
            .contains("Invalid IPv6 address"));
    }

    #[tokio::test]
    async fn test_with_ipv6_in_ip_parameter() {
        let mut query_string_parameters: HashMap<String, String> = HashMap::new();
        query_string_parameters.insert("apikey".into(), "porkDyn".into());
        query_string_parameters.insert("secretapikey".into(), "secret".into());
        query_string_parameters.insert("domain".into(), "me.example.org".into());
        query_string_parameters.insert("ip".into(), "2001:db8::1".into());

        let request = Request::default().with_query_string_parameters(query_string_parameters);

        let response = function_handler(request).await.unwrap();
        assert_eq!(response.status(), 400);

        let body_bytes = response.body().to_vec();
        let body_string = String::from_utf8(body_bytes).unwrap();
        let body_json = serde_json::from_str::<serde_json::Value>(&body_string).unwrap();

        assert_eq!(
            body_json["message"].as_str().unwrap(),
            "IPv6 address provided in 'ip' parameter, use 'ipv6' parameter instead"
        );
    }

    #[tokio::test]
    async fn test_with_ipv4_in_ipv6_parameter() {
        let mut query_string_parameters: HashMap<String, String> = HashMap::new();
        query_string_parameters.insert("apikey".into(), "porkDyn".into());
        query_string_parameters.insert("secretapikey".into(), "secret".into());
        query_string_parameters.insert("domain".into(), "me.example.org".into());
        query_string_parameters.insert("ipv6".into(), "192.168.1.1".into());

        let request = Request::default().with_query_string_parameters(query_string_parameters);

        let response = function_handler(request).await.unwrap();
        assert_eq!(response.status(), 400);

        let body_bytes = response.body().to_vec();
        let body_string = String::from_utf8(body_bytes).unwrap();
        let body_json = serde_json::from_str::<serde_json::Value>(&body_string).unwrap();

        assert_eq!(
            body_json["message"].as_str().unwrap(),
            "IPv4 address provided in 'ipv6' parameter, use 'ip' parameter instead"
        );
    }

    // Note: Tests for valid IPv4/IPv6 API calls would require mocking the HTTP client
    // For now, we test the IP validation logic in the ip_utils module
    // Integration tests with actual API calls would need a test environment
}
