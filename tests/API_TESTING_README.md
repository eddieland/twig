# API Client Testing Strategy

This document outlines the testing strategy for the API client functionality in the Twig project.

## Overview

The API client tests are organized into several categories:

1. **Basic Unit Tests** - Tests for client creation and basic functionality
2. **Model Tests** - Tests for data models and serialization/deserialization
3. **Endpoint Tests** - Tests for specific API endpoint functionality
4. **Integration Tests** - Tests that simulate real API interactions using WireMock
5. **Mocking Examples** - Reference implementations for HTTP mocking with WireMock

## Test Files

- `api_client_test.rs` - Basic unit tests for API client creation and configuration
- `api_models_test.rs` - Tests for API data models and structures
- `api_endpoints_test.rs` - Tests for API endpoint functionality
- `api_client_integration_test.rs` - Integration tests using WireMock
- `api_mocking_example.rs` - Example of using WireMock for HTTP mocking

## Testing Approach

### Unit Testing

Unit tests focus on testing individual components in isolation:

- Client creation and configuration
- Authentication handling
- Data model serialization/deserialization
- URL formatting
- Response parsing

### Integration Testing

Integration tests focus on testing the interaction between components:

- Client making HTTP requests
- Handling different HTTP response codes
- Processing response data
- Error handling

### HTTP Mocking with WireMock

To avoid making actual API calls during testing, we use WireMock for HTTP request mocking:

1. Start a WireMock server for the test
2. Configure stub mappings for specific requests
3. Make API calls to the WireMock server
4. Verify the requests and responses

WireMock provides several advantages over our previous simple mocking approach:
- More realistic HTTP simulation
- Built-in request verification
- Support for complex request matching
- Stateful behavior simulation
- Response templating

## Test Coverage

The tests cover the following API clients:

### GitHub API Client

- Client creation and configuration
- Authentication
- Pull request endpoints
- Check run endpoints
- User endpoints
- Error handling

### Jira API Client

- Client creation and configuration
- Authentication
- Issue endpoints
- Transition endpoints
- Error handling

## Running Tests

Run all tests:

```bash
cargo test
```

Run specific test files:

```bash
cargo test --test api_client_test
cargo test --test api_models_test
cargo test --test api_endpoints_test
cargo test --test api_client_integration_test
```

## Future Improvements

1. Expand WireMock usage to cover more complex scenarios
2. Implement more detailed assertions in tests
3. Add tests for edge cases and error conditions
4. Add tests for rate limiting and pagination
5. Add tests for concurrent API calls
6. Implement scenario-based testing with WireMock

## Setting Up WireMock

### Dependencies

Add the following dependencies to your `Cargo.toml`:

```toml
[dev-dependencies]
wiremock = "0.5"
tokio = { version = "1", features = ["full"] }
```

### Basic Usage

Here's a basic example of using WireMock in a test:

```rust
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

#[tokio::test]
async fn test_api_client() {
    // Start a mock server
    let mock_server = MockServer::start().await;

    // Create a mock
    Mock::given(method("GET"))
        .and(path("/users/octocat"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(serde_json::json!({
                "login": "octocat",
                "id": 1,
                "name": "The Octocat"
            })))
        .mount(&mock_server)
        .await;

    // Create your API client with the mock server URL
    let client = GitHubClient::new(&mock_server.uri());

    // Make the API call
    let user = client.get_user("octocat").await.unwrap();

    // Verify the response
    assert_eq!(user.login, "octocat");
    assert_eq!(user.id, 1);
    assert_eq!(user.name, Some("The Octocat".to_string()));
}
```

### Advanced Features

WireMock supports many advanced features:

1. **Request Matching**: Match requests based on method, path, headers, query parameters, and body content
2. **Response Templating**: Generate dynamic responses based on request data
3. **Stateful Behavior**: Configure different responses for subsequent requests
4. **Request Verification**: Verify that specific requests were made
5. **Proxying**: Forward requests to another server
6. **Fault Injection**: Simulate network failures and delays
