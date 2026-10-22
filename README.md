# porkDyn 🐷

A Dynamic DNS (DDNS) updater for [Porkbun.com](https://porkbun.com), written in Rust and designed to run as an AWS Lambda function.

![logo](./images/logos/porkDynWithCrabSmaller.jpg)

## Features

- **IPv4 Support**: Updates A records for IPv4 addresses
- **IPv6 Support**: Updates AAAA records for IPv6 addresses
- **Dual Stack Support**: Update both IPv4 and IPv6 records in a single request
- **Smart Updates**: Only updates DNS records when IP addresses change
- **AWS Lambda**: Runs serverlessly with minimal cost
- **Router Compatible**: Works with FRITZ!Box and other routers that support custom DDNS URLs

## Quick Start

### 1. Get Your Porkbun API Credentials

1. Log into your [Porkbun account](https://porkbun.com/account/api)
2. Enable API access for your domain
3. Generate API credentials (you'll get an API Key and Secret API Key)
4. Save these credentials securely - you'll need them later

### 2. Set Up Your Subdomain

Decide on a subdomain for your DDNS (e.g., `home.yourdomain.com`). The subdomain must have at least 3 parts: `subdomain.domain.tld`.

Examples:
- ✅ `home.example.com`
- ✅ `vpn.mydomain.org`
- ❌ `example.com` (needs a subdomain)

### 3. Deploy to AWS Lambda

#### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) toolchain installed
- [Cargo Lambda](https://www.cargo-lambda.info/guide/installation.html) CLI tool
- AWS credentials configured (via `aws configure` or environment variables)

#### Build and Deploy

```bash
# Clone the repository
git clone https://github.com/yourusername/porkDyn.git
cd porkDyn

# Build for production
cargo lambda build --release

# Deploy to AWS Lambda
cargo lambda deploy

# Note the Function URL that's printed - you'll need this!
```

The deployment will:
- Create an IAM role for the Lambda function
- Deploy the function with a public Function URL
- Set up necessary permissions

**Save the Function URL** - it will look like: `https://xxxxxxxxxx.lambda-url.us-east-1.on.aws/`

### 4. Configure Your Router or DDNS Client

#### Using the Lambda Function URL

Your DDNS update URL format (choose one):

**IPv4 only:**
```
https://YOUR-LAMBDA-URL/?apikey=<YOUR_API_KEY>&secretapikey=<YOUR_SECRET_KEY>&domain=<SUBDOMAIN>&ip=<IPV4_ADDRESS>
```

**IPv6 only:**
```
https://YOUR-LAMBDA-URL/?apikey=<YOUR_API_KEY>&secretapikey=<YOUR_SECRET_KEY>&domain=<SUBDOMAIN>&ipv6=<IPV6_ADDRESS>
```

**Dual stack (IPv4 + IPv6):**
```
https://YOUR-LAMBDA-URL/?apikey=<YOUR_API_KEY>&secretapikey=<YOUR_SECRET_KEY>&domain=<SUBDOMAIN>&ip=<IPV4_ADDRESS>&ipv6=<IPV6_ADDRESS>
```

Replace the placeholders:
- `YOUR-LAMBDA-URL` with your actual Lambda Function URL
- `<YOUR_API_KEY>` with your Porkbun API key
- `<YOUR_SECRET_KEY>` with your Porkbun secret API key
- `<SUBDOMAIN>` with your subdomain (e.g., `home.example.com`)
- `<IPV4_ADDRESS>` with your IPv4 address (e.g., `1.2.3.4`)
- `<IPV6_ADDRESS>` with your IPv6 address (e.g., `2001:db8::1`)

#### FRITZ!Box Setup

1. Navigate to: **Internet → Shares → DynDNS**
2. Select **Custom** as the provider
3. Configure:
   - **Update URL** (choose one):
     - **IPv4 only**: `https://YOUR-LAMBDA-URL/?apikey=<username>&secretapikey=<pass>&domain=<domain>&ip=<ipaddr>`
     - **Dual stack (IPv4 + IPv6)**: `https://YOUR-LAMBDA-URL/?apikey=<username>&secretapikey=<pass>&domain=<domain>&ip=<ipaddr>&ipv6=<ip6addr>`
   - **Domain name**: Your subdomain (e.g., `home.example.com`)
   - **Username**: Your Porkbun API key
   - **Password**: Your Porkbun secret API key
4. Click **Apply**

Note: FRITZ!Box will automatically replace `<username>` with the Username field, `<pass>` with the Password field, `<domain>` with the Domain name field, `<ipaddr>` with your IPv4, and `<ip6addr>` with your IPv6 address.

#### Other Routers

Most routers that support custom DDNS providers will work. Configure them to send an HTTP GET request to your Lambda URL with the appropriate query parameters.

### 5. Test Your Setup

Test manually with curl:

```bash
# IPv4 only
curl "https://YOUR-LAMBDA-URL/?apikey=xxx&secretapikey=yyy&domain=home.example.com&ip=1.2.3.4"

# IPv6 only
curl "https://YOUR-LAMBDA-URL/?apikey=xxx&secretapikey=yyy&domain=home.example.com&ipv6=2001:db8::1"

# Dual stack
curl "https://YOUR-LAMBDA-URL/?apikey=xxx&secretapikey=yyy&domain=home.example.com&ip=1.2.3.4&ipv6=2001:db8::1"
```

You should receive a JSON response indicating success or any errors.

## API Reference

### Query Parameters

| Parameter | Required | Description |
|-----------|----------|-------------|
| `apikey` | Yes | Your Porkbun API key |
| `secretapikey` | Yes | Your Porkbun secret API key |
| `domain` | Yes | Fully qualified domain name (e.g., `home.example.com`) |
| `ip` | No* | IPv4 address to update (A record) |
| `ipv6` | No* | IPv6 address to update (AAAA record) |

\* At least one IP address (`ip` or `ipv6`) must be provided.

### URL Examples

**IPv4 Only:**
```
?apikey=xxx&secretapikey=yyy&domain=home.example.com&ip=192.168.1.100
```

**IPv6 Only:**
```
?apikey=xxx&secretapikey=yyy&domain=home.example.com&ipv6=2001:db8::1
```

**Dual Stack:**
```
?apikey=xxx&secretapikey=yyy&domain=home.example.com&ip=192.168.1.100&ipv6=2001:db8::1
```

### How It Works

1. **Validates** required query parameters, all supplied IP addresses and their families, and the subdomain format before contacting Porkbun. Porkbun checks credential validity.
2. **Reconciles** each supplied address, processing A before AAAA for dual-stack requests.
3. **Skips** writes when the existing record's address text exactly matches the requested text. Equivalent IPv6 spellings can trigger an update.
4. **Creates** absent records or **updates** changed records, using a TTL of 600 seconds for writes.
5. **Returns** a message-only JSON response. Successful dual-stack messages are joined with `; ` in A-then-AAAA order.

Processing stops at the first upstream failure. An A failure prevents AAAA processing. If AAAA fails after A was changed, the A change remains and the response contains only the error.

### Response Format

Responses use `Content-Type: application/json` and contain only `message`. The HTTP status communicates the result: **200** for success, **400** for invalid input, and **502** for upstream failures (including network and response-decoding errors).

Example success (HTTP 200):

```json
{
  "message": "A record '\"home.example.com\"' updated successfully; AAAA record \"home.example.com\" is already up to date"
}
```

Example creation (HTTP 200):

```json
{
  "message": "A record for subdomain '\"home\"' successfully created"
}
```

Example validation error (HTTP 400):

```json
{
  "message": "Missing query-parameter 'apikey'"
}
```

Example upstream error (HTTP 502):

```json
{
  "message": "Upstream DNS service error: Failed to retrieve DNS record: Access denied"
}
```

## Development

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) toolchain
- [Cargo Lambda](https://www.cargo-lambda.info/guide/installation.html) CLI
- AWS credentials configured (for deployment)

### Building

```bash
# Development build
cargo lambda build

# Production build (optimized)
cargo lambda build --release
```

Read more in [the Cargo Lambda build documentation](https://www.cargo-lambda.info/commands/build.html).

## Testing

Run the validation and local HTTP integration tests with `cargo test --locked`. The reconciliation and handler tests use a local mock Porkbun server through the real client; they need no Porkbun credentials or AWS deployment.

The repository checks are:

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo build --locked
cargo test --locked
```

To exercise the Lambda runtime locally, use `cargo lambda watch` and `cargo lambda invoke`. Valid DNS-update requests through that runtime contact the real Porkbun API.

First, run `cargo lambda watch` to start a local server. When you make changes to the code, the server will automatically restart.

Second, you'll need a way to pass the event data to the lambda function.

You can use the existent [event payloads](https://github.com/awslabs/aws-lambda-rust-runtime/tree/main/lambda-events/src/fixtures) in the Rust Runtime repository if your lambda function is using one of the supported event types.

You can use those examples directly with the `--data-example` flag, where the value is the name of the file in the [lambda-events](https://github.com/awslabs/aws-lambda-rust-runtime/tree/main/lambda-events/src/fixtures) repository without the `example_` prefix and the `.json` extension.

```bash
cargo lambda invoke --data-example apigw-request
```

For generic events, where you define the event data structure, you can create a JSON file with the data you want to test with. For example:

```json
{
  "command": "test"
}
```

Then, run `cargo lambda invoke --data-file ./data.json` to invoke the function with the data in `data.json`.

For HTTP events, you can also call the function directly with cURL or any other HTTP client. For example:

```bash
curl https://localhost:9000
```

Read more about running the local server in [the Cargo Lambda documentation for the `watch` command](https://www.cargo-lambda.info/commands/watch.html).
Read more about invoking the function in [the Cargo Lambda documentation for the `invoke` command](https://www.cargo-lambda.info/commands/invoke.html).

## Deploying

To deploy the project, run `cargo lambda deploy`. This will create an IAM role and a Lambda function in your AWS account.

Read more about deploying your lambda function in [the Cargo Lambda documentation](https://www.cargo-lambda.info/commands/deploy.html).
