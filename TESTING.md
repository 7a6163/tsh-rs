# Testing Guide for tsh-rs

This document describes how to run and understand the tests for tsh-rs.

## Test Structure

The test suite is organized into three main categories:

### 1. Noise Protocol Tests (`src/tests/noise_tests.rs`)
- Key generation
- Handshake protocol
- Data transmission
- PSK authentication
- Large message handling

### 2. Protocol Tests (`src/tests/protocol_tests.rs`)
- Operation mode serialization
- Command execution protocol
- File transfer protocols
- Helper function tests

### 3. Integration Tests (`src/tests/integration_tests.rs`)
- End-to-end command execution
- File operations
- Multiple connections
- Error handling

## Running Tests Locally

### Run all tests
```bash
cargo test
```

### Run only unit tests
```bash
cargo test --lib
```

### Run specific test module
```bash
cargo test --lib tests::noise_tests
cargo test --lib tests::protocol_tests
cargo test --lib tests::integration_tests
```

### Run tests with output
```bash
cargo test --lib -- --nocapture
```

### Run tests in single thread (useful for debugging)
```bash
cargo test --lib -- --test-threads=1
```

### Run specific test
```bash
cargo test --lib test_noise_handshake
```

## Test Coverage

To generate test coverage report:

```bash
# Install cargo-llvm-cov
cargo install cargo-llvm-cov

# Generate coverage report
cargo llvm-cov --html

# Open coverage report
open target/llvm-cov/html/index.html
```

## Continuous Integration

Tests run automatically on:
- Every push to main branch
- Every pull request

The CI workflow includes:
- Unit tests on multiple platforms (Linux, macOS, Windows)
- Code formatting checks
- Clippy linting
- Documentation generation
- Functional tests

## Writing New Tests

When adding new features, please include:
1. Unit tests for core functionality
2. Integration tests for feature interactions
3. Error case testing

Example test structure:
```rust
#[tokio::test]
async fn test_my_feature() {
    // Arrange
    let test_data = prepare_test_data();
    
    // Act
    let result = my_feature(test_data).await;
    
    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), expected_value);
}
```

## Troubleshooting

### Tests fail with "Device not configured"
This happens when tests try to use TTY features in non-TTY environment. These tests should be skipped in CI.

### Tests timeout
Some tests involve network operations. Ensure no firewall is blocking local connections.

### Random test failures
Some tests use random ports. If a test fails randomly, it might be due to port conflicts. Try running tests with `--test-threads=1`.

## Performance Testing

For performance-sensitive code, use criterion for benchmarking:

```bash
# Add to Cargo.toml dev-dependencies
# criterion = "0.5"

# Run benchmarks
cargo bench
```