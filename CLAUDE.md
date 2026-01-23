# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

porkDyn is a Dynamic DNS (DDNS) updater for Porkbun.com, written in Rust and designed to run as an AWS Lambda function. It supports both IPv4 (A records) and IPv6 (AAAA records) with dual-stack capability.

## Commands

### Building
- Development build: `cargo lambda build`
- Production build: `cargo lambda build --release`

### Testing
- Run unit tests: `cargo test`
- Run a specific test: `cargo test <test_name>`
- Local integration testing:
  1. Start local server: `cargo lambda watch` (auto-reloads on changes)
  2. Invoke with example data: `cargo lambda invoke --data-example apigw-request`
  3. Invoke with custom JSON: `cargo lambda invoke --data-file ./data.json`
  4. Invoke via HTTP: `curl https://localhost:9000`

### Deployment
- Deploy to AWS: `cargo lambda deploy`

## Architecture

### Request Flow
1. **HTTP Handler** (`http_handler.rs`): Entry point that validates query parameters (apikey, secretapikey, domain, ip, ipv6)
2. **Domain Parsing** (`domain.rs`): Splits qualified domain name into domain and subdomain components
3. **IP Validation** (`ip_utils.rs`): Validates and classifies IP addresses as IPv4 or IPv6
4. **DNS Operations** (`api.rs`): Interacts with Porkbun API to get, create, or update DNS records

### Core Components

**Domain Model** (`domain.rs`):
- Parses qualified domain names (e.g., "api.example.com") into:
  - `domain_name`: "example.com"
  - `subdomain`: "api"
  - `qualified_name`: "api.example.com"
- Requires at least 3 parts (subdomain.domain.tld)

**IP Processing** (`ip_utils.rs`):
- `IpType`: Enum for V4/V6 classification
- `RecordType`: Enum for DNS record types (A for IPv4, AAAA for IPv6)
- Uses `std::net::IpAddr` for validation

**API Client** (`api.rs`):
- Base URL: `https://api.porkbun.com/api/json/v3`
- `get_existing_dns_record`: Retrieves DNS record by name and type
- `update_dns_record`: Updates existing record by ID
- `create_dns_record`: Creates new DNS record
- TTL is hardcoded to 600 seconds

**HTTP Handler Logic**:
- Supports IPv4-only, IPv6-only, or dual-stack updates
- Processes each IP type independently
- Skips updates if record exists with same IP
- Returns JSON responses with status messages

### Data Flow Pattern
1. Extract and validate query parameters
2. Parse domain into components
3. Validate and classify IP addresses
4. For each IP address:
   - Check if DNS record exists
   - If exists and unchanged: skip update
   - If exists and changed: update record
   - If doesn't exist: create new record
5. Return combined success/error status

## Important Implementation Details

- The crate name is `pork_dyn` (with underscore) as defined in Cargo.toml
- Uses `lambda_http` for AWS Lambda integration with HTTP events
- HTTP client uses `reqwest` with rustls-tls (not native-tls)
- All API calls to Porkbun use POST requests with JSON bodies containing credentials
- Error handling uses `thiserror` for custom error types
- Comprehensive unit tests exist for validation logic but not API integration (would require mocking)
