# tsh-rs

[![CI](https://github.com/7a6163/tsh-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/7a6163/tsh-rs/actions/workflows/ci.yml)
[![Tests](https://github.com/7a6163/tsh-rs/actions/workflows/test.yml/badge.svg)](https://github.com/7a6163/tsh-rs/actions/workflows/test.yml)

A Rust implementation of Tiny Shell (tsh) - a remote shell access tool for secure command execution and file transfers.

## Features

- **Secure Communication**: Noise Protocol with ChaCha20-Poly1305 AEAD encryption
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

- **`tsh`** - Unified binary with both client and server modes
  - `tsh server` - Server daemon mode that provides shell access
  - `tsh client` - Client mode for connecting to remote systems

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

### Server Mode

```bash
# Listen mode - wait for connections on port 1234
./tsh server --psk your-secret-key --port 1234

# Connect-back mode - connect to client every 5 seconds
./tsh server --psk your-secret-key --connect-back 192.168.1.100 --port 1234 --delay 5
```

### Client Mode

```bash
# Interactive shell
./tsh client --psk your-secret-key 192.168.1.100:1234

# Connect-back mode (wait for server)
./tsh client --psk your-secret-key cb --port 1234

# Download file
./tsh client --psk your-secret-key 192.168.1.100:1234 get:/remote/file.txt:./local/

# Upload file
./tsh client --psk your-secret-key 192.168.1.100:1234 put:./local/file.txt:/remote/

# Execute command
./tsh client --psk your-secret-key 192.168.1.100:1234 "ls -la"
```

## Command Line Options

### Server Mode (`tsh server`)
- `--psk <PSK>` - Pre-shared key for authentication (required)
- `-p, --port <PORT>` - Port number (default: 1234)
- `-c, --connect-back <HOST>` - Connect back to client host
- `-d, --delay <SECONDS>` - Connect back delay in seconds (default: 5)

### Client Mode (`tsh client`)
- `--psk <PSK>` - Pre-shared key for authentication (required)
- `-p, --port <PORT>` - Port number (default: 1234)
- `<HOST>` - Target hostname or "cb" for connect-back mode
- `[ACTION]` - Action to perform (get:remote:local, put:local:remote, or command)

## Security Features

- **Noise Protocol Framework**: Modern cryptographic protocol with proven security
- **ChaCha20-Poly1305 AEAD**: Authenticated encryption with associated data
- **X25519 Key Exchange**: Elliptic curve Diffie-Hellman key agreement
- **BLAKE2s Hashing**: Fast and secure cryptographic hash function
- **Perfect Forward Secrecy**: Each session uses ephemeral keys
- **Quantum Resistance**: X25519 provides resistance to quantum attacks on key exchange

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
make run-client ARGS="127.0.0.1"

# Run server in development
make run-server ARGS=""
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

```
MIT License

Copyright (c) 2025 Zac

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Architecture

### Core Components

- **Noise Protocol Layer**: Modern encrypted communication using Noise_XX_25519_ChaChaPoly_BLAKE2s
- **PTY Abstraction**: Cross-platform pseudo-terminal interface
- **Error Handling**: Comprehensive error types with context
- **Async I/O**: Built on Tokio for high performance

### Security Design

1. **Key Exchange**: X25519 elliptic curve Diffie-Hellman
2. **Handshake**: Noise XX pattern with mutual authentication
3. **Encryption**: ChaCha20-Poly1305 AEAD for confidentiality and integrity
4. **Hashing**: BLAKE2s for fast cryptographic operations
5. **Message Framing**: Length-prefixed encrypted messages with authentication

## Improvements over Go Version

- **Memory Safety**: Rust prevents buffer overflows and memory corruption
- **Async Performance**: Tokio provides efficient async I/O
- **Error Handling**: Result types force explicit error handling
- **Type Safety**: Strong typing prevents many runtime errors
- **Zero-Cost Abstractions**: High-level features with no runtime overhead
- **Better Tooling**: Cargo provides excellent dependency management

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for detailed version history and release notes.
