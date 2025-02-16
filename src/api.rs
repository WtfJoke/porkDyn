use lambda_http::tracing::{error, info, log::debug};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct DnsRecord {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub record_type: String,
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

pub(crate) async fn get_existing_record(
    client: &Client,
    api_key: &str,
    secret_key: &str,
    domain_name: &str,
    subdomain_with_domain: &str,
) -> Result<Option<DnsRecord>, reqwest::Error> {
    let url = format!("{}/dns/retrieve/{}", API_BASE_URL, domain_name);
    info!("Get existing records: {:?} for domain: '{:?}'", url, domain_name);
    let response: ExistingRecordsResponse = client
        .post(&url)
        .json(&serde_json::json!({ "apikey": api_key, "secretapikey": secret_key }))
        .send()
        .await?
        .json()
        .await?;

    if response.status == "SUCCESS" {
        debug!("Found records: {:?}", response.records);
        if let Some(records) = response.records {
            for record in records {
                debug!(
                    "Checking record: {:?} to find {:?}",
                    record, subdomain_with_domain
                );
                if record.name == subdomain_with_domain && record.record_type == "A" {
                    info!("Found existing record: {:?}", record);
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
    api_key: &str,
    secret_key: &str,
    domain: &str,
    subdomain: &str,
    record_id: &str,
    ip: &str,
) -> Result<(), reqwest::Error> {
    let url: String = format!("{}/dns/edit/{}/{}", API_BASE_URL, domain, record_id);
    let request_body: CreateUpdateDnsRecordRequest = CreateUpdateDnsRecordRequest::new(api_key, secret_key, subdomain, ip);
    info!("Update DNS record: '{:?}' for subdomain '{:?}'.", url, subdomain);
    let res: EditDnsRecordResponse = client
        .post(&url)
        .json(&request_body)
        .send()
        .await?
        .json()
        .await?;

    if res.status == "SUCCESS" {
        info!("Updated DNS record with id: {:?}", record_id);
    } else {
        error!("Failed to update DNS record");
    }
    Ok(())
}

pub(crate) async fn create_dns_record(
    client: &Client,
    api_key: &str,
    secret_key: &str,
    domain: &str,
    subdomain: &str,
    ip: &str,
) -> Result<(), reqwest::Error> {
    let url = format!("{}/dns/create/{}", API_BASE_URL, domain);
    let request_body: CreateUpdateDnsRecordRequest = CreateUpdateDnsRecordRequest::new(api_key, secret_key, subdomain, ip);
    info!("Create DNS record: {:?} for subdomain {:?}", url, subdomain);
    let res: CreateDnsRecordResponse = client
        .post(&url)
        .json(&request_body)
        .send()
        .await?
        .json()
        .await?;

    if res.status == "SUCCESS" {
        info!("Created DNS record with id: {:?}", res.id);
    } else {
        error!("Failed to create DNS record");
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct CreateUpdateDnsRecordRequest {
    apikey: String,
    #[serde(rename="secretapikey")]
    secret_api_key: String,
    name: String,
    #[serde(rename="type")]
    record_type: String,
    content: String,
    ttl: u64,
}

impl CreateUpdateDnsRecordRequest {
    pub fn new(api_key: &str, secret_api_key: &str, subdomain: &str, ip: &str) -> Self {
        CreateUpdateDnsRecordRequest {
            apikey: api_key.into(),
            secret_api_key: secret_api_key.into(),
            name: subdomain.into(),
            record_type: "A".into(),
            content: ip.into(),
            ttl: 600,
        }
    }
}