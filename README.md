# tsh-rs

A Rust implementation of Tiny Shell (tsh) - a remote shell access tool for secure command execution and file transfers.

## Features

- **Secure Communication**: AES-CBC encryption with HMAC authentication
- **Cross-platform**: Supports Linux, Windows, and macOS
- **Multiple Operation Modes**:
  - Interactive shell access
  - File download (`get`)
  - File upload (`put`)
  - Direct command execution
- **Connection Modes**:
  - Direct connection to server
  - Connect-back mode (server connects to client)
- **Modern Rust Implementation**: 
  - Memory safety
  - Async/await with Tokio
  - Strong error handling
  - Zero-cost abstractions

## Components

- **`tsh`** - Client application for connecting to remote systems
- **`tshd`** - Server daemon that provides shell access

## Building

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install cross-compilation targets (optional)
make install-targets
```

### Build Commands

```bash
# Build Linux x64 binaries
make linux

# Build Windows x64 binaries  
make windows

# Build macOS binaries (both x64 and ARM64)
make macos

# Build for custom target
make unix TARGET=aarch64-unknown-linux-gnu

# Development build
make dev

# Clean build artifacts
make clean
```

See `make help` for all available commands.

## Usage

### Server (tshd)

```bash
# Listen mode - wait for connections on port 1234
./tshd -s mysecret -p 1234

# Connect-back mode - connect to client every 5 seconds
./tshd -s mysecret -c 192.168.1.100 -p 1234 -d 5
```

### Client (tsh)

```bash
# Interactive shell
./tsh -s mysecret -p 1234 192.168.1.100

# Connect-back mode (wait for server)
./tsh -s mysecret -p 1234 cb

# Download file
./tsh -s mysecret 192.168.1.100 get /remote/file.txt ./local/

# Upload file  
./tsh -s mysecret 192.168.1.100 put ./local/file.txt /remote/

# Execute command
./tsh -s mysecret 192.168.1.100 "ls -la"
```

## Command Line Options

### tsh (Client)
- `-s, --secret <SECRET>` - Authentication secret (default: "1234")
- `-p, --port <PORT>` - Port number (default: 1234)
- `<TARGET>` - Target hostname or "cb" for connect-back mode
- `[ACTION]` - Action to perform (get/put/command)

### tshd (Server)
- `-s, --secret <SECRET>` - Authentication secret (default: "1234")  
- `-p, --port <PORT>` - Port number (default: 1234)
- `-c, --connect-back <HOST>` - Connect back to host (client mode)
- `-d, --delay <SECONDS>` - Connect back delay in seconds (default: 5)

## Security Features

- **AES-CBC Encryption**: All communication is encrypted
- **HMAC Authentication**: Message integrity verification
- **Challenge-Response**: Mutual authentication between client/server
- **Secure Random IVs**: Cryptographically secure initialization vectors

## Cross-Platform Support

| Platform | Architecture | Status |
|----------|-------------|---------|
| Linux | x86_64 | ✅ |
| Linux | ARM64 | ✅ |
| Windows | x86_64 | ✅ |
| macOS | x86_64 | ✅ |
| macOS | ARM64 | ✅ |
| FreeBSD | x86_64 | ✅ |
| OpenBSD | x86_64 | ✅ |

## Development

```bash
# Run tests
make test

# Format code
make fmt

# Run linter
make clippy

# Run client in development
make run-client ARGS="-s test 127.0.0.1"

# Run server in development  
make run-server ARGS="-s test"
```

## License

MIT License - see LICENSE file for details.

## Architecture

### Core Components

- **PEL (Packet Encryption Layer)**: Custom encrypted communication protocol
- **PTY Abstraction**: Cross-platform pseudo-terminal interface  
- **Error Handling**: Comprehensive error types with context
- **Async I/O**: Built on Tokio for high performance

### Security Design

1. **Handshake**: Client/server exchange random IVs and authenticate
2. **Key Derivation**: SHA-1 based key derivation from shared secret + IV
3. **Encryption**: AES-128-CBC for data confidentiality
4. **Authentication**: HMAC-SHA1 for message integrity
5. **Packet Framing**: Length-prefixed encrypted packets

## Improvements over Go Version

- **Memory Safety**: Rust prevents buffer overflows and memory corruption
- **Async Performance**: Tokio provides efficient async I/O
- **Error Handling**: Result types force explicit error handling
- **Type Safety**: Strong typing prevents many runtime errors
- **Zero-Cost Abstractions**: High-level features with no runtime overhead
- **Better Tooling**: Cargo provides excellent dependency management