use crate::api::{PorkbunClient, ReconciliationOutcome};
use crate::credentials::Credentials;
use crate::domain::Domain;
use crate::ip_utils::{validate_and_classify_ip, IpType, RecordType};
use lambda_http::tracing::{error, info};
use lambda_http::{Body, Error, Request, RequestExt, Response};

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
///
/// At least one address is required:
/// - ip: The IPv4 address to which the DNS A record should be updated
/// - ipv6: The IPv6 address to which the DNS AAAA record should be updated
pub(crate) async fn function_handler(
    event: Request,
    porkbun: &PorkbunClient,
) -> Result<Response<Body>, Error> {
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
    // Validate every supplied address before making any DNS requests.
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

    let mut results = Vec::new();

    // Process each IP address
    for ip_update in ip_updates {
        let record_type = ip_update.record_type.as_str();
        let result = porkbun
            .reconcile_address(
                &credentials,
                &domain,
                &ip_update.address,
                &ip_update.record_type,
            )
            .await;

        match result {
            Ok(outcome) => {
                let message = match outcome {
                    ReconciliationOutcome::Unchanged => format!(
                        "{} record {:?} is already up to date",
                        record_type,
                        domain.qualified_name()
                    ),
                    ReconciliationOutcome::Updated => format!(
                        "{} record '{:?}' updated successfully",
                        record_type,
                        domain.qualified_name()
                    ),
                    ReconciliationOutcome::Created => format!(
                        "{} record for subdomain '{:?}' successfully created",
                        record_type,
                        domain.subdomain()
                    ),
                };
                results.push(message);
            }
            Err(e) => {
                error!("Failed to process {} record: {:?}", record_type, e);
                return Ok(json_response(
                    502,
                    &format!("Upstream DNS service error: {}", e),
                ));
            }
        }
    }

    let success_message = results.join("; ");
    Ok(json_response(200, &success_message))
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
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_without_query_strings() {
        assert_rejected_without_provider_request(
            Request::default(),
            "Missing query-parameter 'apikey'",
        )
        .await;
    }

    #[tokio::test]
    async fn test_with_missing_secret_api_key() {
        let mut query_string_parameters: HashMap<String, String> = HashMap::new();
        query_string_parameters.insert("apikey".into(), "porkDyn".into());

        let request = Request::default().with_query_string_parameters(query_string_parameters);

        assert_rejected_without_provider_request(request, "Missing query-parameter 'secretapikey'")
            .await;
    }

    #[tokio::test]
    async fn test_with_missing_domain() {
        let mut query_string_parameters: HashMap<String, String> = HashMap::new();
        query_string_parameters.insert("apikey".into(), "porkDyn".into());
        query_string_parameters.insert("secretapikey".into(), "secret".into());

        let request = Request::default().with_query_string_parameters(query_string_parameters);

        assert_rejected_without_provider_request(request, "Missing query-parameter 'domain'").await;
    }

    #[tokio::test]
    async fn test_with_missing_ip() {
        let mut query_string_parameters: HashMap<String, String> = HashMap::new();
        query_string_parameters.insert("apikey".into(), "porkDyn".into());
        query_string_parameters.insert("secretapikey".into(), "secret".into());
        query_string_parameters.insert("domain".into(), "me.example.org".into());

        let request = Request::default().with_query_string_parameters(query_string_parameters);

        assert_rejected_without_provider_request(
            request,
            "At least one IP address must be provided (ip or ipv6)",
        )
        .await;
    }

    #[tokio::test]
    async fn test_with_invalid_ipv4() {
        let mut query_string_parameters: HashMap<String, String> = HashMap::new();
        query_string_parameters.insert("apikey".into(), "porkDyn".into());
        query_string_parameters.insert("secretapikey".into(), "secret".into());
        query_string_parameters.insert("domain".into(), "me.example.org".into());
        query_string_parameters.insert("ip".into(), "invalid_ip".into());

        let request = Request::default().with_query_string_parameters(query_string_parameters);

        assert_rejected_without_provider_request(
            request,
            "Invalid IPv4 address: Invalid IP address: invalid_ip",
        )
        .await;
    }

    #[tokio::test]
    async fn test_with_invalid_ipv6() {
        let mut query_string_parameters: HashMap<String, String> = HashMap::new();
        query_string_parameters.insert("apikey".into(), "porkDyn".into());
        query_string_parameters.insert("secretapikey".into(), "secret".into());
        query_string_parameters.insert("domain".into(), "me.example.org".into());
        query_string_parameters.insert("ipv6".into(), "invalid_ipv6".into());

        let request = Request::default().with_query_string_parameters(query_string_parameters);

        assert_rejected_without_provider_request(
            request,
            "Invalid IPv6 address: Invalid IP address: invalid_ipv6",
        )
        .await;
    }

    #[tokio::test]
    async fn test_with_ipv6_in_ip_parameter() {
        let mut query_string_parameters: HashMap<String, String> = HashMap::new();
        query_string_parameters.insert("apikey".into(), "porkDyn".into());
        query_string_parameters.insert("secretapikey".into(), "secret".into());
        query_string_parameters.insert("domain".into(), "me.example.org".into());
        query_string_parameters.insert("ip".into(), "2001:db8::1".into());

        let request = Request::default().with_query_string_parameters(query_string_parameters);

        assert_rejected_without_provider_request(
            request,
            "IPv6 address provided in 'ip' parameter, use 'ipv6' parameter instead",
        )
        .await;
    }

    #[tokio::test]
    async fn test_with_ipv4_in_ipv6_parameter() {
        let mut query_string_parameters: HashMap<String, String> = HashMap::new();
        query_string_parameters.insert("apikey".into(), "porkDyn".into());
        query_string_parameters.insert("secretapikey".into(), "secret".into());
        query_string_parameters.insert("domain".into(), "me.example.org".into());
        query_string_parameters.insert("ipv6".into(), "192.168.1.1".into());

        let request = Request::default().with_query_string_parameters(query_string_parameters);

        assert_rejected_without_provider_request(
            request,
            "IPv4 address provided in 'ipv6' parameter, use 'ip' parameter instead",
        )
        .await;
    }

    #[tokio::test]
    async fn single_address_success_preserves_exact_response_messages() {
        for (parameter, kind, address, previous, write, expected) in [
            (
                "ip",
                "A",
                "192.0.2.1",
                Some("192.0.2.1"),
                None,
                "A record \"home.example.com\" is already up to date",
            ),
            (
                "ip",
                "A",
                "192.0.2.1",
                Some("192.0.2.2"),
                Some("edit/example.com/42"),
                "A record '\"home.example.com\"' updated successfully",
            ),
            (
                "ip",
                "A",
                "192.0.2.1",
                None,
                Some("create/example.com"),
                "A record for subdomain '\"home\"' successfully created",
            ),
            (
                "ipv6",
                "AAAA",
                "2001:db8::1",
                Some("2001:db8::1"),
                None,
                "AAAA record \"home.example.com\" is already up to date",
            ),
            (
                "ipv6",
                "AAAA",
                "2001:db8::1",
                Some("2001:db8::2"),
                Some("edit/example.com/42"),
                "AAAA record '\"home.example.com\"' updated successfully",
            ),
            (
                "ipv6",
                "AAAA",
                "2001:db8::1",
                None,
                Some("create/example.com"),
                "AAAA record for subdomain '\"home\"' successfully created",
            ),
        ] {
            let server = MockServer::start().await;
            let records: Vec<_> = previous
                .into_iter()
                .map(|content| {
                    json!({
                        "id": "42", "name": "home.example.com", "type": kind, "content": content
                    })
                })
                .collect();
            mount_response(
                &server,
                &format!("retrieveByNameType/example.com/{kind}/home"),
                json!({"status": "SUCCESS", "records": records}),
            )
            .await;
            if let Some(write) = write {
                mount_response(&server, write, json!({"status": "SUCCESS"})).await;
            }

            let response = handle(&server, request(&[(parameter, address)])).await;

            assert_response(&response, 200, expected);
            assert_eq!(
                server.received_requests().await.unwrap().len(),
                if write.is_some() { 2 } else { 1 }
            );
        }
    }

    #[tokio::test]
    async fn provider_failures_return_message_only_bad_gateway_responses() {
        for (operation, expected) in [
            (
                "lookup",
                "Upstream DNS service error: Failed to retrieve DNS record: Access denied",
            ),
            (
                "create",
                "Upstream DNS service error: Failed to create DNS record: Access denied",
            ),
            (
                "update",
                "Upstream DNS service error: Failed to update DNS record: Access denied",
            ),
        ] {
            let server = MockServer::start().await;
            let error = json!({"status": "ERROR", "message": "Access denied"});
            if operation == "lookup" {
                mount_response(&server, "retrieveByNameType/example.com/A/home", error).await;
            } else {
                let records = if operation == "update" {
                    json!([{"id": "42", "name": "home.example.com", "type": "A", "content": "192.0.2.2"}])
                } else {
                    json!([])
                };
                mount_response(
                    &server,
                    "retrieveByNameType/example.com/A/home",
                    json!({"status": "SUCCESS", "records": records}),
                )
                .await;
                mount_response(
                    &server,
                    if operation == "update" {
                        "edit/example.com/42"
                    } else {
                        "create/example.com"
                    },
                    error,
                )
                .await;
            }

            let response = handle(
                &server,
                request(&[("ip", "192.0.2.1"), ("ipv6", "2001:db8::1")]),
            )
            .await;

            assert_response(&response, 502, expected);
            // An A failure stops the dual-stack request before AAAA is attempted.
            assert_eq!(
                server.received_requests().await.unwrap().len(),
                if operation == "lookup" { 1 } else { 2 }
            );
        }
    }

    #[tokio::test]
    async fn decoding_failures_are_bad_gateway_responses() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(
                "/api/json/v3/dns/retrieveByNameType/example.com/A/home",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_raw("not JSON", "application/json"))
            .expect(1)
            .mount(&server)
            .await;

        let response = handle(&server, request(&[("ip", "192.0.2.1")])).await;

        assert_eq!(response.status(), 502);
        assert_eq!(response.headers()["content-type"], "application/json");
        let body: Value = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body.as_object().unwrap().len(), 1);
        assert!(body["message"]
            .as_str()
            .unwrap()
            .starts_with("Upstream DNS service error: Network error: "));
    }

    #[tokio::test]
    async fn dual_stack_reconciles_a_before_aaaa_and_joins_messages_in_order() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(
                "/api/json/v3/dns/retrieveByNameType/example.com/A/home",
            ))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(std::time::Duration::from_millis(30))
                    .set_body_json(json!({"status": "SUCCESS", "records": [{
                        "id": "42", "name": "home.example.com", "type": "A", "content": "192.0.2.2"
                    }]})),
            )
            .expect(1)
            .mount(&server)
            .await;
        mount_response(&server, "edit/example.com/42", json!({"status": "SUCCESS"})).await;
        mount_response(
            &server,
            "retrieveByNameType/example.com/AAAA/home",
            json!({"status": "SUCCESS", "records": []}),
        )
        .await;
        mount_response(&server, "create/example.com", json!({"status": "SUCCESS"})).await;

        let response = handle(
            &server,
            request(&[("ip", "192.0.2.1"), ("ipv6", "2001:db8::1")]),
        )
        .await;

        assert_response(&response, 200, "A record '\"home.example.com\"' updated successfully; AAAA record for subdomain '\"home\"' successfully created");
        let requests = server.received_requests().await.unwrap();
        let paths: Vec<_> = requests.iter().map(|request| request.url.path()).collect();
        assert_eq!(
            paths,
            [
                "/api/json/v3/dns/retrieveByNameType/example.com/A/home",
                "/api/json/v3/dns/edit/example.com/42",
                "/api/json/v3/dns/retrieveByNameType/example.com/AAAA/home",
                "/api/json/v3/dns/create/example.com",
            ]
        );
    }

    #[tokio::test]
    async fn aaaa_failure_returns_only_the_error_after_a_was_updated() {
        let server = MockServer::start().await;
        mount_response(
            &server,
            "retrieveByNameType/example.com/A/home",
            json!({"status": "SUCCESS", "records": [{
                "id": "42", "name": "home.example.com", "type": "A", "content": "192.0.2.2"
            }]}),
        )
        .await;
        mount_response(&server, "edit/example.com/42", json!({"status": "SUCCESS"})).await;
        mount_response(
            &server,
            "retrieveByNameType/example.com/AAAA/home",
            json!({"status": "SUCCESS", "records": []}),
        )
        .await;
        mount_response(
            &server,
            "create/example.com",
            json!({"status": "ERROR", "message": "Access denied"}),
        )
        .await;

        let response = handle(
            &server,
            request(&[("ip", "192.0.2.1"), ("ipv6", "2001:db8::1")]),
        )
        .await;

        assert_response(
            &response,
            502,
            "Upstream DNS service error: Failed to create DNS record: Access denied",
        );
        let requests = server.received_requests().await.unwrap();
        let paths: Vec<_> = requests.iter().map(|request| request.url.path()).collect();
        assert_eq!(
            paths,
            [
                "/api/json/v3/dns/retrieveByNameType/example.com/A/home",
                "/api/json/v3/dns/edit/example.com/42",
                "/api/json/v3/dns/retrieveByNameType/example.com/AAAA/home",
                "/api/json/v3/dns/create/example.com",
            ]
        );
    }

    #[tokio::test]
    async fn all_input_is_validated_before_any_provider_request() {
        let server = MockServer::start().await;
        for (parameters, expected) in [
            (
                vec![("ip", "192.0.2.1"), ("ipv6", "invalid")],
                "Invalid IPv6 address: Invalid IP address: invalid",
            ),
            (
                vec![("ip", "192.0.2.1"), ("ipv6", "192.0.2.2")],
                "IPv4 address provided in 'ipv6' parameter, use 'ip' parameter instead",
            ),
            (
                vec![("ip", "invalid"), ("ipv6", "2001:db8::1")],
                "Invalid IPv4 address: Invalid IP address: invalid",
            ),
            (
                vec![("ip", "192.0.2.1"), ("domain", "example.com")],
                "Invalid subdomain format",
            ),
        ] {
            let response = handle(&server, request(&parameters)).await;

            assert_response(&response, 400, expected);
        }
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn reused_client_uses_each_requests_credentials() {
        let server = MockServer::start().await;
        let client = PorkbunClient::with_base_url(format!("{}/api/json/v3", server.uri()));
        for (key, secret) in [
            ("first-key", "first-secret"),
            ("second-key", "second-secret"),
        ] {
            Mock::given(method("POST"))
                .and(path(
                    "/api/json/v3/dns/retrieveByNameType/example.com/A/home",
                ))
                .and(body_json(json!({"apikey": key, "secretapikey": secret})))
                .respond_with(ResponseTemplate::new(200).set_body_json(
                    json!({"status": "SUCCESS", "records": [{
                        "id": "42", "name": "home.example.com", "type": "A", "content": "192.0.2.1"
                    }]}),
                ))
                .expect(1)
                .mount(&server)
                .await;

            let response = function_handler(
                request(&[
                    ("apikey", key),
                    ("secretapikey", secret),
                    ("ip", "192.0.2.1"),
                ]),
                &client,
            )
            .await
            .unwrap();

            assert_response(
                &response,
                200,
                "A record \"home.example.com\" is already up to date",
            );
        }
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    fn request(parameters: &[(&str, &str)]) -> Request {
        let mut query = HashMap::from([
            ("apikey".to_string(), "key".to_string()),
            ("secretapikey".to_string(), "secret".to_string()),
            ("domain".to_string(), "home.example.com".to_string()),
        ]);
        query.extend(
            parameters
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string())),
        );
        Request::default().with_query_string_parameters(query)
    }

    async fn mount_response(server: &MockServer, operation: &str, body: Value) {
        Mock::given(method("POST"))
            .and(path(format!("/api/json/v3/dns/{operation}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(1)
            .mount(server)
            .await;
    }

    async fn handle(server: &MockServer, request: Request) -> Response<Body> {
        let client = PorkbunClient::with_base_url(format!("{}/api/json/v3", server.uri()));
        function_handler(request, &client).await.unwrap()
    }

    async fn assert_rejected_without_provider_request(request: Request, message: &str) {
        let server = MockServer::start().await;
        let response = handle(&server, request).await;

        assert_response(&response, 400, message);
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    fn assert_response(response: &Response<Body>, status: u16, message: &str) {
        assert_eq!(response.status(), status);
        assert_eq!(response.headers()["content-type"], "application/json");
        let body: Value = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body, json!({"message": message}));
    }
}
