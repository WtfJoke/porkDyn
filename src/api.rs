use crate::{credentials::Credentials, domain::Domain, error::ApiError, ip_utils::RecordType};
use lambda_http::tracing::{error, info, log::debug};
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub(crate) struct PorkbunClient {
    client: Client,
    base_url: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ReconciliationOutcome {
    Unchanged,
    Updated,
    Created,
}

#[derive(Debug, Deserialize)]
struct DnsRecord {
    id: String,
    name: String,
    #[serde(rename = "type")]
    _record_type: String,
    content: String,
}
#[derive(Debug, Deserialize)]
struct ExistingRecordsResponse {
    status: String,
    records: Option<Vec<DnsRecord>>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateDnsRecordResponse {
    status: String,
    id: Option<u64>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EditDnsRecordResponse {
    status: String,
    message: Option<String>,
}

const API_BASE_URL: &str = "https://api.porkbun.com/api/json/v3";
const DEFAULT_TTL: u64 = 600;

impl PorkbunClient {
    pub(crate) fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: API_BASE_URL.to_string(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_base_url(base_url: String) -> Self {
        Self {
            base_url,
            ..Self::new()
        }
    }

    /// Reconciles one validated address whose family matches `record_type`.
    ///
    /// The first exact-name match is compared using literal address text.
    /// A failed lookup is an error, never an absent record eligible for creation.
    /// Request-level validation and dual-stack sequencing belong to the caller.
    pub(crate) async fn reconcile_address(
        &self,
        credentials: &Credentials,
        domain: &Domain,
        address: &str,
        record_type: &RecordType,
    ) -> Result<ReconciliationOutcome, ApiError> {
        match self
            .get_existing_dns_record(credentials, domain, record_type)
            .await?
        {
            Some(record) if record.content == address => {
                info!(
                    "Skip updating, {} record with id {:?} is already up to date.",
                    record_type.as_str(),
                    record.id
                );
                Ok(ReconciliationOutcome::Unchanged)
            }
            Some(record) => {
                self.update_dns_record(credentials, domain, &record.id, address, record_type)
                    .await?;
                Ok(ReconciliationOutcome::Updated)
            }
            None => {
                self.create_dns_record(credentials, domain, address, record_type)
                    .await?;
                Ok(ReconciliationOutcome::Created)
            }
        }
    }

    async fn get_existing_dns_record(
        &self,
        credentials: &Credentials,
        domain: &Domain,
        record_type: &RecordType,
    ) -> Result<Option<DnsRecord>, ApiError> {
        let domain_name = domain.domain_name();
        let subdomain = domain.subdomain();
        let qualified_name = domain.qualified_name();
        let record_type_str = record_type.as_str();
        let url = format!(
            "{}/dns/retrieveByNameType/{}/{}/{}",
            self.base_url, domain_name, record_type_str, subdomain
        );
        info!(
            "Get existing '{}' record for domain {:?} by calling {:?}",
            record_type_str, domain_name, url
        );
        let response: ExistingRecordsResponse = self.client
        .post(&url)
        .json(&serde_json::json!({ "apikey": credentials.api_key(), "secretapikey": credentials.secret_key() }))
        .send()
        .await?
        .json()
        .await?;

        if response.status != "SUCCESS" {
            let error_msg = response
                .message
                .unwrap_or_else(|| "Failed to retrieve DNS records".to_string());
            error!("Failed to retrieve DNS records: {}", error_msg);
            return Err(ApiError::RetrieveRecordFailed(error_msg));
        }

        if let Some(records) = response.records {
            info!("Found record: {:?}", records);
            for record in records {
                debug!("Checking record: {:?} to find {:?}", record, qualified_name);
                if record.name == qualified_name {
                    info!(
                        "Found matching record for subdomain {:?}: {:?}",
                        qualified_name, record
                    );
                    return Ok(Some(record));
                }
            }
        }

        info!("No existing record found.");
        Ok(None)
    }

    async fn update_dns_record(
        &self,
        credentials: &Credentials,
        domain: &Domain,
        record_id: &str,
        ip: &str,
        record_type: &RecordType,
    ) -> Result<(), ApiError> {
        let domain_name = domain.domain_name();
        let subdomain = domain.subdomain();
        let url: String = format!("{}/dns/edit/{}/{}", self.base_url, domain_name, record_id);
        let request_body: CreateUpdateDnsRecordRequest =
            CreateUpdateDnsRecordRequest::new(credentials, subdomain, ip, record_type);
        info!(
            "Update DNS record: {:?} for subdomain {:?}.",
            url, subdomain
        );
        let edit_response: EditDnsRecordResponse = self
            .client
            .post(&url)
            .json(&request_body)
            .send()
            .await?
            .json()
            .await?;

        if edit_response.status == "SUCCESS" {
            info!("Updated DNS record with id: {:?}", record_id);
            Ok(())
        } else {
            let error_msg = edit_response
                .message
                .unwrap_or_else(|| "Unknown error".to_string());
            error!("Failed to update DNS record: {}", error_msg);
            Err(ApiError::UpdateRecordFailed(error_msg))
        }
    }

    async fn create_dns_record(
        &self,
        credentials: &Credentials,
        domain: &Domain,
        ip: &str,
        record_type: &RecordType,
    ) -> Result<(), ApiError> {
        let domain_name = domain.domain_name();
        let subdomain = domain.subdomain();
        let url = format!("{}/dns/create/{}", self.base_url, domain_name);
        let request_body: CreateUpdateDnsRecordRequest =
            CreateUpdateDnsRecordRequest::new(credentials, subdomain, ip, record_type);
        info!("Create DNS record: {:?} for subdomain {:?}", url, subdomain);
        let create_response: CreateDnsRecordResponse = self
            .client
            .post(&url)
            .json(&request_body)
            .send()
            .await?
            .json()
            .await?;

        if create_response.status == "SUCCESS" {
            info!("Created DNS record with id: {:?}", create_response.id);
            Ok(())
        } else {
            let error_msg = create_response
                .message
                .unwrap_or_else(|| "Unknown error".to_string());
            error!("Failed to create DNS record: {}", error_msg);
            Err(ApiError::CreateRecordFailed(error_msg))
        }
    }
}

#[derive(Debug, Serialize)]
struct CreateUpdateDnsRecordRequest {
    apikey: String,
    #[serde(rename = "secretapikey")]
    secret_api_key: String,
    name: String,
    #[serde(rename = "type")]
    record_type: String,
    content: String,
    ttl: u64,
}

impl CreateUpdateDnsRecordRequest {
    fn new(credentials: &Credentials, subdomain: &str, ip: &str, record_type: &RecordType) -> Self {
        CreateUpdateDnsRecordRequest {
            apikey: credentials.api_key().into(),
            secret_api_key: credentials.secret_key().into(),
            name: subdomain.into(),
            record_type: record_type.as_str().into(),
            content: ip.into(),
            ttl: DEFAULT_TTL,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn unchanged_record_requires_no_write() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(
                "/api/json/v3/dns/retrieveByNameType/example.com/A/home",
            ))
            .and(body_json(
                json!({"apikey": "key", "secretapikey": "secret"}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "SUCCESS",
                "records": [{
                    "id": "42", "name": "home.example.com", "type": "A", "content": "192.0.2.1",
                    "ttl": "60"
                }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = PorkbunClient::with_base_url(format!("{}/api/json/v3", server.uri()));
        let outcome = client
            .reconcile_address(
                &Credentials::new("key".into(), "secret".into()),
                &Domain::new("home.example.com").unwrap(),
                "192.0.2.1",
                &RecordType::A,
            )
            .await
            .unwrap();

        assert_eq!(outcome, ReconciliationOutcome::Unchanged);
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn updates_first_exact_name_match_using_literal_address_text() {
        for (record_type, kind, response_kind, previous, desired) in [
            (RecordType::A, "A", "AAAA", "192.0.2.1", "192.0.2.2"),
            (
                RecordType::AAAA,
                "AAAA",
                "A",
                "2001:db8::1",
                "2001:0db8:0:0:0:0:0:1",
            ),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path(format!("/api/json/v3/dns/retrieveByNameType/example.com/{kind}/home")))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "status": "SUCCESS",
                    "records": [
                        {"id": "unrelated", "name": "other.example.com", "type": kind, "content": desired},
                        {"id": "wrong-case", "name": "HOME.example.com", "type": kind, "content": desired},
                        {"id": "first", "name": "home.example.com", "type": response_kind, "content": previous},
                        {"id": "second", "name": "home.example.com", "type": kind, "content": desired}
                    ]
                })))
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(path("/api/json/v3/dns/edit/example.com/first"))
                .and(body_json(json!({
                    "apikey": "key", "secretapikey": "secret", "name": "home",
                    "type": kind, "content": desired, "ttl": 600
                })))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(json!({"status": "SUCCESS"})),
                )
                .expect(1)
                .mount(&server)
                .await;

            let outcome = reconcile(&server, desired, &record_type).await.unwrap();

            assert_eq!(outcome, ReconciliationOutcome::Updated);
            assert_eq!(server.received_requests().await.unwrap().len(), 2);
        }
    }

    #[tokio::test]
    async fn creates_record_when_successful_lookup_has_no_matching_record() {
        for lookup_response in [
            json!({"status": "SUCCESS", "records": []}),
            json!({"status": "SUCCESS"}),
            json!({"status": "SUCCESS", "records": null}),
            json!({"status": "SUCCESS", "records": [
                {"id": "other", "name": "other.example.com", "type": "A", "content": "192.0.2.1"}
            ]}),
        ] {
            for (record_type, kind, address) in [
                (RecordType::A, "A", "192.0.2.1"),
                (RecordType::AAAA, "AAAA", "2001:db8::1"),
            ] {
                let server = MockServer::start().await;
                Mock::given(method("POST"))
                    .and(path(format!(
                        "/api/json/v3/dns/retrieveByNameType/example.com/{kind}/home"
                    )))
                    .respond_with(ResponseTemplate::new(200).set_body_json(&lookup_response))
                    .expect(1)
                    .mount(&server)
                    .await;
                Mock::given(method("POST"))
                    .and(path("/api/json/v3/dns/create/example.com"))
                    .and(body_json(json!({
                        "apikey": "key", "secretapikey": "secret", "name": "home",
                        "type": kind, "content": address, "ttl": 600
                    })))
                    // A successful creation does not require an ID in the response.
                    .respond_with(
                        ResponseTemplate::new(200).set_body_json(json!({"status": "SUCCESS"})),
                    )
                    .expect(1)
                    .mount(&server)
                    .await;

                let outcome = reconcile(&server, address, &record_type).await.unwrap();

                assert_eq!(outcome, ReconciliationOutcome::Created);
                assert_eq!(server.received_requests().await.unwrap().len(), 2);
            }
        }
    }

    #[tokio::test]
    async fn lookup_failure_is_an_error_not_absence() {
        for (response, expected) in [
            (
                json!({"status": "ERROR", "message": "Access denied", "records": []}),
                "Access denied",
            ),
            (json!({"status": "ERROR"}), "Failed to retrieve DNS records"),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path(
                    "/api/json/v3/dns/retrieveByNameType/example.com/A/home",
                ))
                .respond_with(ResponseTemplate::new(200).set_body_json(response))
                .expect(1)
                .mount(&server)
                .await;

            let error = reconcile(&server, "192.0.2.1", &RecordType::A)
                .await
                .unwrap_err();

            assert!(
                matches!(&error, ApiError::RetrieveRecordFailed(message) if message == expected),
                "{error:?}"
            );
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn write_failures_preserve_the_operation_and_provider_message() {
        for existing in [false, true] {
            for (response, expected) in [
                (
                    json!({"status": "ERROR", "message": "Access denied"}),
                    "Access denied",
                ),
                (json!({"status": "ERROR"}), "Unknown error"),
            ] {
                let server = MockServer::start().await;
                mount_lookup(&server, existing).await;
                Mock::given(method("POST"))
                    .and(path(write_path(existing)))
                    .respond_with(ResponseTemplate::new(200).set_body_json(response))
                    .expect(1)
                    .mount(&server)
                    .await;

                let error = reconcile(&server, "192.0.2.1", &RecordType::A)
                    .await
                    .unwrap_err();

                match (existing, error) {
                    (true, ApiError::UpdateRecordFailed(message))
                    | (false, ApiError::CreateRecordFailed(message)) => {
                        assert_eq!(message, expected)
                    }
                    (_, error) => panic!("Unexpected error: {error:?}"),
                }
                assert_eq!(server.received_requests().await.unwrap().len(), 2);
            }
        }
    }

    #[tokio::test]
    async fn json_status_controls_success_even_on_http_error_responses() {
        for existing in [false, true] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path(
                    "/api/json/v3/dns/retrieveByNameType/example.com/A/home",
                ))
                .respond_with(ResponseTemplate::new(500).set_body_json(lookup_response(existing)))
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(path(write_path(existing)))
                .respond_with(
                    ResponseTemplate::new(503).set_body_json(json!({"status": "SUCCESS"})),
                )
                .expect(1)
                .mount(&server)
                .await;

            let outcome = reconcile(&server, "192.0.2.1", &RecordType::A)
                .await
                .unwrap();

            assert_eq!(
                outcome,
                if existing {
                    ReconciliationOutcome::Updated
                } else {
                    ReconciliationOutcome::Created
                }
            );
            assert_eq!(server.received_requests().await.unwrap().len(), 2);
        }
    }

    #[tokio::test]
    async fn malformed_responses_stop_reconciliation_at_the_failing_operation() {
        for operation in ["lookup", "create", "update"] {
            for body in ["not json", "{}"] {
                let server = MockServer::start().await;
                let response_path = if operation == "lookup" {
                    "/api/json/v3/dns/retrieveByNameType/example.com/A/home"
                } else {
                    mount_lookup(&server, operation == "update").await;
                    write_path(operation == "update")
                };
                Mock::given(method("POST"))
                    .and(path(response_path))
                    .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
                    .expect(1)
                    .mount(&server)
                    .await;

                let error = reconcile(&server, "192.0.2.1", &RecordType::A)
                    .await
                    .unwrap_err();

                assert!(
                    matches!(error, ApiError::NetworkError(ref error) if error.is_decode()),
                    "{error:?}"
                );
                let requests = server.received_requests().await.unwrap();
                assert_eq!(requests.len(), if operation == "lookup" { 1 } else { 2 });
            }
        }
    }

    #[tokio::test]
    async fn transport_failure_is_propagated() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/api/json/v3", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (connection, _) = listener.accept().await.unwrap();
            drop(connection);
        });

        let error = PorkbunClient::with_base_url(endpoint)
            .reconcile_address(
                &Credentials::new("key".into(), "secret".into()),
                &Domain::new("home.example.com").unwrap(),
                "192.0.2.1",
                &RecordType::A,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ApiError::NetworkError(_)), "{error:?}");
        server.await.unwrap();
    }

    fn lookup_response(existing: bool) -> serde_json::Value {
        if existing {
            json!({"status": "SUCCESS", "records": [{
                "id": "42", "name": "home.example.com", "type": "A", "content": "192.0.2.2"
            }]})
        } else {
            json!({"status": "SUCCESS", "records": []})
        }
    }

    async fn mount_lookup(server: &MockServer, existing: bool) {
        Mock::given(method("POST"))
            .and(path(
                "/api/json/v3/dns/retrieveByNameType/example.com/A/home",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(lookup_response(existing)))
            .expect(1)
            .mount(server)
            .await;
    }

    fn write_path(existing: bool) -> &'static str {
        if existing {
            "/api/json/v3/dns/edit/example.com/42"
        } else {
            "/api/json/v3/dns/create/example.com"
        }
    }

    #[tokio::test]
    async fn malformed_lookup_is_an_error_not_absence() {
        for body in [
            r#"{"status": "SUCCESS", "records": {}}"#,
            r#"{"status": "SUCCESS", "records": [{"name": "home.example.com", "type": "A", "content": "192.0.2.1"}]}"#,
        ] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path(
                    "/api/json/v3/dns/retrieveByNameType/example.com/A/home",
                ))
                .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
                .expect(1)
                .mount(&server)
                .await;

            let error = reconcile(&server, "192.0.2.1", &RecordType::A)
                .await
                .unwrap_err();

            assert!(
                matches!(error, ApiError::NetworkError(ref source) if source.is_decode()),
                "{error:?}"
            );
            assert_eq!(server.received_requests().await.unwrap().len(), 1);
        }
    }

    async fn reconcile(
        server: &MockServer,
        address: &str,
        record_type: &RecordType,
    ) -> Result<ReconciliationOutcome, ApiError> {
        PorkbunClient::with_base_url(format!("{}/api/json/v3", server.uri()))
            .reconcile_address(
                &Credentials::new("key".into(), "secret".into()),
                &Domain::new("home.example.com").unwrap(),
                address,
                record_type,
            )
            .await
    }
}
