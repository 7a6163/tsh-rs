# tsh-rs Demo

## Quick Start

### 1. Build the project
```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Or use make
make dev
```

### 2. Run the server
```bash
# Listen mode - wait for connections
./target/release/tshd -s mysecret -p 8080

# Or run directly with cargo
cargo run --bin tshd -- -s mysecret -p 8080
```

### 3. Run the client
```bash
# Connect to server
./target/release/tsh -s mysecret -p 8080 127.0.0.1

# Or run directly with cargo
cargo run --bin tsh -- -s mysecret -p 8080 127.0.0.1
```

## Key Improvements over Go Version

### 1. Memory Safety
- **No buffer overflows**: Rust prevents common memory corruption bugs
- **No use-after-free**: Ownership system ensures memory safety
- **No data races**: Thread safety guaranteed at compile time

### 2. Modern Error Handling
```rust
// Structured error types with context
pub enum TshError {
    Network(String),
    Encryption(String),
    Authentication,
    ConnectionClosed,
    // ... more specific error types
}
```

### 3. Async/Await Performance
- Built on Tokio for high-performance async I/O
- Efficient handling of concurrent connections
- Non-blocking operations throughout

### 4. Enhanced Security
- Uses modern crypto libraries with safer APIs
- Strong typing prevents many security bugs
- Comprehensive error handling for crypto operations

### 5. Better Development Experience
- Cargo for dependency management
- Built-in testing framework
- Excellent tooling (clippy, rustfmt)
- Cross-compilation support

## Examples

### File Transfer
```bash
# Download file
./tsh -s secret 192.168.1.100 get /remote/file.txt ./local/

# Upload file
./tsh -s secret 192.168.1.100 put ./local/file.txt /remote/
```

### Command Execution
```bash
# Execute single command
./tsh -s secret 192.168.1.100 "ls -la"

# Interactive shell
./tsh -s secret 192.168.1.100
```

### Connect-back Mode
```bash
# Client waits for server connection
./tsh -s secret -p 8080 cb

# Server connects back to client
./tshd -s secret -c 192.168.1.100 -p 8080 -d 5
```

## Cross-Platform Builds

```bash
# Install targets
make install-targets

# Build for different platforms
make linux      # Linux x64
make windows    # Windows x64
make macos      # macOS (both x64 and ARM64)

# Custom target
make unix TARGET=aarch64-unknown-linux-gnu
```

## Testing

```bash
# Run tests
make test

# Format code
make fmt

# Run linter
make clippy
```

This Rust implementation provides the same functionality as the Go version while adding memory safety, better performance, and modern development practices.