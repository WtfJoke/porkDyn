use crate::{credentials::Credentials, domain::Domain, ip_utils::RecordType};
use lambda_http::tracing::{error, info, log::debug};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct DnsRecord {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    _record_type: String,
    pub content: String,
}
#[derive(Debug, Deserialize)]
struct ExistingRecordsResponse {
    status: String,
    records: Option<Vec<DnsRecord>>,
}

#[derive(Debug, Deserialize)]
struct CreateDnsRecordResponse {
    status: String,
    id: u64,
}

#[derive(Debug, Deserialize)]
struct EditDnsRecordResponse {
    status: String,
}

const API_BASE_URL: &str = "https://api.porkbun.com/api/json/v3";
const DEFAULT_TTL: u64 = 600;

pub(crate) async fn get_existing_dns_record(
    client: &Client,
    credentials: &Credentials,
    domain: &Domain,
    record_type: &RecordType,
) -> Result<Option<DnsRecord>, reqwest::Error> {
    let domain_name = domain.domain_name();
    let subdomain = domain.subdomain();
    let qualified_name = domain.qualified_name();
    let record_type_str = record_type.as_str();
    let url = format!(
        "{}/dns/retrieveByNameType/{}/{}/{}",
        API_BASE_URL, domain_name, record_type_str, subdomain
    );
    info!(
        "Get existing '{}' record for domain {:?} by calling {:?}",
        record_type_str, domain_name, url
    );
    let response: ExistingRecordsResponse = client
        .post(&url)
        .json(&serde_json::json!({ "apikey": credentials.api_key(), "secretapikey": credentials.secret_key() }))
        .send()
        .await?
        .json()
        .await?;

    if response.status == "SUCCESS" {
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
    }
    info!("No existing record found.");
    Ok(None)
}

pub(crate) async fn update_dns_record(
    client: &Client,
    credentials: &Credentials,
    domain: &Domain,
    record_id: &str,
    ip: &str,
    record_type: &RecordType,
) -> Result<(), reqwest::Error> {
    let domain_name = domain.domain_name();
    let subdomain = domain.subdomain();
    let url: String = format!("{}/dns/edit/{}/{}", API_BASE_URL, domain_name, record_id);
    let request_body: CreateUpdateDnsRecordRequest =
        CreateUpdateDnsRecordRequest::new(credentials, subdomain, ip, record_type);
    info!(
        "Update DNS record: {:?} for subdomain {:?}.",
        url, subdomain
    );
    let edit_response: EditDnsRecordResponse = client
        .post(&url)
        .json(&request_body)
        .send()
        .await?
        .json()
        .await?;

    if edit_response.status == "SUCCESS" {
        info!("Updated DNS record with id: {:?}", record_id);
    } else {
        error!("Failed to update DNS record");
    }
    Ok(())
}

pub(crate) async fn create_dns_record(
    client: &Client,
    credentials: &Credentials,
    domain: &Domain,
    ip: &str,
    record_type: &RecordType,
) -> Result<(), reqwest::Error> {
    let domain_name = domain.domain_name();
    let subdomain = domain.subdomain();
    let url = format!("{}/dns/create/{}", API_BASE_URL, domain_name);
    let request_body: CreateUpdateDnsRecordRequest =
        CreateUpdateDnsRecordRequest::new(credentials, subdomain, ip, record_type);
    info!("Create DNS record: {:?} for subdomain {:?}", url, subdomain);
    let create_response: CreateDnsRecordResponse = client
        .post(&url)
        .json(&request_body)
        .send()
        .await?
        .json()
        .await?;

    if create_response.status == "SUCCESS" {
        info!("Created DNS record with id: {:?}", create_response.id);
    } else {
        error!("Failed to create DNS record");
    }
    Ok(())
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
    pub fn new(
        credentials: &Credentials,
        subdomain: &str,
        ip: &str,
        record_type: &RecordType,
    ) -> Self {
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
