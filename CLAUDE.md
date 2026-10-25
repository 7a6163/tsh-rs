# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

tsh-rs is a Rust implementation of Tiny Shell — a secure remote access tool. Single binary (`tsh`) with two subcommands: `server` (agent on target) and `client` (operator). All traffic is encrypted with Noise Protocol (XX pattern, X25519, ChaCha20-Poly1305, BLAKE2s) plus PSK challenge-response.

## Build & test commands

```bash
cargo build                          # dev build
cargo build --release                # release build
cargo test                           # all tests
cargo test --lib tests::noise_tests  # one test module
cargo test --lib test_noise_handshake # one test by name
cargo test --lib -- --nocapture      # with stdout
cargo clippy -- -D warnings          # lint (CI enforced)
cargo fmt                            # format
```

Cross-compile targets: `make linux`, `make windows`, `make macos`, `make unix TARGET=<triple>`.

## Architecture

### Transport abstraction

`AsyncStream` (trait alias for `AsyncRead + AsyncWrite + Unpin + Send`) is the core abstraction. Everything flows through `NoiseLayer`, which wraps any `AsyncStream` and adds encryption. Two transports plug in:

- **Raw TCP** — default, `TcpStream` implements `AsyncStream` directly
- **WebSocket** — `WsByteStream` in `c2_https.rs` adapts `WebSocketStream` to `AsyncStream`

### Message framing

`NoiseLayer::write`/`read` handle framing: `[4-byte BE length][encrypted payload + 16-byte tag]`. The `NoiseLayerExt` trait in `helpers.rs` adds `write_all`/`read_exact` convenience methods.

### Operation modes

After Noise handshake + PSK auth, the client sends a 1-byte `OperationMode` (defined in `constants.rs`): GetFile(1), PutFile(2), RunShell(3), RunCommand(4), SysInfo(5), Socks5(6). The server dispatches on this byte in `handle_client_connection` (`server.rs`).

### Connection flow

1. `main.rs` — CLI parsing with clap, dispatches to server/client functions
2. **Direct**: client connects TCP/WS → Noise handshake → PSK auth → operation
3. **Connect-back**: server connects out to client's listener with jitter (±25%) on delay

### Platform-specific code

- `persistence.rs` — LaunchAgent (macOS), systemd (Linux), Registry (Windows), with `#[cfg]` gates
- `pty.rs` — uses `portable-pty` crate, unix-only resize via `nix`
- `sysinfo.rs` — hand-rolled JSON (no serde), platform-conditional collection

### Error handling

`TshError` (thiserror) in `error.rs`, with `TshResult<T>` alias. Constructor helpers like `TshError::network("msg")` used throughout.

## Test structure

Tests live in `src/tests/` as an in-crate `#[cfg(test)]` module (not a separate test crate). Each source file has a corresponding `*_tests.rs`. Tests are `#[tokio::test]` async. Use `--test-threads=1` if you hit port conflicts.

## Conventions

- No serde — JSON is hand-built in `sysinfo.rs` with manual `escape_json`/`extract_json_*` helpers. Use the same pattern.
- File path validation in `server.rs` (`validate_file_path`) blocks traversal attacks — always use it for paths from the network.
- `cargo-deny` (`deny.toml`) is configured for license and advisory checks.
