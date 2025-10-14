# Introduction

porkDyn 🐷 is a Dynamic DNS (DDNS) Updater for https://porkbun.com, written in Rust.  
It is designed to run in an AWS Lambda and can be used in your router (for example FRITZ!Box).

![logo](./images/logos/porkDynWithCrabSmaller.jpg)

## Features

- **IPv4 Support**: Updates A records for IPv4 addresses
- **IPv6 Support**: Updates AAAA records for IPv6 addresses
- **Dual Stack Support**: Update both IPv4 and IPv6 records in a single request
- **Flexible IP Parameters**: IPv4 required, IPv6 optional for dual-stack setups
- **AWS Lambda**: Designed to run efficiently in AWS Lambda
- **Porkbun API**: Full integration with Porkbun DNS API

## Usage

The Lambda function accepts the following query parameters:

- `apikey`: Your Porkbun API key (required)
- `secretapikey`: Your Porkbun secret API key (required)
- `domain`: The fully qualified domain name (required, e.g., `subdomain.example.com`)
- `ip`: The IPv4 address for A record (optional, but at least one IP must be provided)
- `ipv6`: The IPv6 address for AAAA record (optional)

### Examples:

**IPv4 Only:**

```
?apikey=xxx&secretapikey=yyy&domain=home.example.com&ip=192.168.1.100
```

**IPv6 Only:**

```
?apikey=xxx&secretapikey=yyy&domain=home.example.com&ipv6=2001:db8::1
```

**Dual Stack (IPv4 + IPv6):**

```
?apikey=xxx&secretapikey=yyy&domain=home.example.com&ip=192.168.1.100&ipv6=2001:db8::1
```

### Behavior:

The function will:

1. Validate all provided IP address formats
2. Process each IP type separately:
   - IPv4 addresses create/update A records
   - IPv6 addresses create/update AAAA records
3. Check if records already exist with the same IPs (skips update if unchanged)
4. Create new records or update existing ones as needed
5. Return status for all processed records

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)
- [Cargo Lambda](https://www.cargo-lambda.info/guide/installation.html)

## Building

To build the project for production, run `cargo lambda build --release`. Remove the `--release` flag to build for development.

Read more about building your lambda function in [the Cargo Lambda documentation](https://www.cargo-lambda.info/commands/build.html).

## Testing

You can run regular Rust unit tests with `cargo test`.

If you want to run integration tests locally, you can use the `cargo lambda watch` and `cargo lambda invoke` commands to do it.

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
