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
4. **Address Reconciliation** (`api.rs`): `PorkbunClient::reconcile_address` owns lookup, selection, comparison, and edit/create; it returns a typed outcome or `ApiError`.

When changing reconciliation behavior or its terminology, read `GLOSSARY.md`. Use the HTTP contract in `README.md` and the regression tests to preserve compatibility.

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
- `main.rs` constructs one `PorkbunClient` for reuse across Lambda invocations. The client owns its transport and fixed production endpoint; credentials are passed per reconciliation.
- `reconcile_address` accepts a validated address and matching record type, and returns `Unchanged`, `Updated`, or `Created`, or a typed `ApiError`.
- Provider operations, record IDs, and wire formats are private to the client. Tests use the internal `with_base_url` constructor with a local mock server.
- Compatibility rules: first exact-name record wins; address comparison is literal; a successful lookup with missing/null records means absence; JSON application status is interpreted without a preliminary HTTP status check. Failed lookups propagate as errors.

**HTTP Handler Logic**:
- Supports IPv4-only, IPv6-only, or dual-stack updates
- Validates the complete request before any provider call, then reconciles A before AAAA and stops on the first error.
- Owns message formatting, message-only JSON, and HTTP status mapping. Typed `ApiError` maps directly to 502.
- An AAAA failure leaves a completed A write in place and returns only the error.

### Data Flow Pattern
1. Extract and validate query parameters
2. Parse domain into components
3. Validate and classify IP addresses
4. Reconcile each address through `PorkbunClient::reconcile_address`
5. Return joined success messages, or the first error

## Important Implementation Details

- The crate name is `pork_dyn` (with underscore) as defined in Cargo.toml
- Uses `lambda_http` for AWS Lambda integration with HTTP events
- HTTP client uses `reqwest` with rustls-tls (not native-tls)
- All API calls to Porkbun use POST requests with JSON bodies containing credentials
- Error handling uses `thiserror` for custom error types
- Reconciliation and handler regression tests exercise the real client against a local `wiremock` server. Keep tests at these interfaces; mock the external provider boundary.
