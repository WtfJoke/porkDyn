use crate::api::{create_dns_record, get_existing_a_record, update_dns_record};
use crate::credentials::Credentials;
use crate::domain::Domain;
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
    let qualified_domain_name = match query_params.first("domain") {
        Some(query_param) => query_param,
        None => return Ok(json_response(400, "Missing query-parameter 'domain'")),
    };
    let ip: &str = match query_params.first("ip") {
        Some(query_param) => query_param,
        None => return Ok(json_response(400, "Missing query-parameter 'ip'")),
    };
    info!(
        "Valid request received for updating the dns-entry for domain: '{:?}' to ip: '{:?}'.",
        qualified_domain_name, ip
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

    // Check if the record exists
    let success_message: String = match get_existing_a_record(&client, &credentials, &domain).await
    {
        // If the record exists and the IP is the same, do nothing and return a success message
        Ok(Some(record)) if record.content == ip => {
            info!(
                "Skip updating, record with id {:?} is already up to date.",
                record.id
            );
            format!("DNS record {:?} is already up to date", record.name)
        }
        // If the record exists and the IP is different, update the record
        Ok(Some(record)) => {
            info!(
                "Updating DNS record {:?} for domain {:?} with subdomain {:?} to IP {:?}",
                record,
                domain.domain_name(),
                domain.subdomain(),
                ip
            );
            update_dns_record(&client, &credentials, &domain, &record.id, ip).await?;
            format!("DNS record '{:?}' updated successfully", record.name)
        }
        // If the record does not exist, create a new one
        Ok(None) => {
            info!(
                "Creating new DNS record for domain {:?} with subdomain {:?} and IP {:?}",
                domain.domain_name(),
                domain.subdomain(),
                ip
            );
            create_dns_record(&client, &credentials, &domain, ip).await?;
            format!(
                "DNS record for subdomain '{:?}' successfully created",
                domain.subdomain()
            )
        }
        // If there is an error, return a 500 error
        Err(e) => {
            error!(
                "Failed to retrieve records for domain {:?}: {:?}",
                domain.domain_name(),
                e
            );
            return Ok(json_response(500, "Failed to retrieve DNS records"));
        }
    };

    Ok(json_response(200, &success_message))
}

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
