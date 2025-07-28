# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.0] - 2025-07-28

**🏗️ Architecture Consolidation & Security Hardening**

### 🔧 Added
- Single unified binary architecture (`tsh` with `server`/`client` subcommands)
- Security hardening compilation flags (PIE, RELRO, stack protection)
- Cross-platform test compatibility (Windows, Linux, macOS)
- Comprehensive documentation updates
- Pre-shared key (PSK) authentication with challenge-response

### 🗑️ Removed
- Separate `tshd` binary (consolidated into `tsh server`)
- Dead code cleanup (unused authentication functions, PTY fields)
- Removed legacy dependencies (aes, cbc, hmac, sha1)
- Removed entire PEL (Packet Encryption Layer) module

### 🔧 Changed
- **BREAKING**: Command line interface now uses subcommands
  - Old: `tshd -p 1234` → New: `tsh server --psk key --port 1234`
  - Old: `tsh -p 1234 host` → New: `tsh client --psk key host:1234`
- Enhanced security configuration via `.cargo/config.toml`
- Updated all documentation and GitHub templates
- Improved error messages and cross-platform compatibility

### 🔒 Security
- Added compilation security hardening flags
- Improved binary security analysis results
- Enhanced PSK-based authentication over encrypted channel
- Removed potential attack surface by consolidating binaries

### 🐛 Fixed
- Windows compatibility issues in integration tests
- Cross-platform shell command execution
- Temporary file handling across different operating systems
- All clippy warnings and formatting issues

---

## [1.0.0] - 2025-07-26

**🚀 Major Release - Noise Protocol Integration**

### 🔒 Security Enhancements
- **BREAKING**: Replaced AES-128-CBC with Noise Protocol Framework
- Implemented Noise_XX_25519_ChaChaPoly_BLAKE2s pattern
- Added ChaCha20-Poly1305 AEAD encryption for authenticated encryption
- Integrated X25519 key exchange for perfect forward secrecy
- Added BLAKE2s hashing for improved performance
- Enhanced quantum resistance for key exchange operations

### 🛠️ Infrastructure Improvements
- Updated GitHub Actions workflows (deprecated actions/upload-artifact@v3 → v4)
- Fixed deprecated release workflow actions
- Updated thiserror dependency (v1.0.69 → v2.0.12)
- Fixed cargo-deny configuration for modern standards
- Resolved all clippy warnings and compilation errors

### 📋 Breaking Changes
- Removed shared secret authentication (replaced with public key cryptography)
- Updated command line interface (removed `-s/--secret` flags)
- Changed file transfer syntax (get:source:dest, put:source:dest)
- Consolidated to single unified binary (tsh with server/client subcommands)

### 🏗️ Technical Details
- Cross-platform signal handling improvements (Unix/Windows)
- Enhanced error handling and logging
- Maintained backward compatibility in core functionality
- Zero external runtime dependencies

### 🔧 Added
- New `noise.rs` module with modern cryptographic implementation
- Dependency Review workflow for pull requests
- Comprehensive documentation and examples
- MIT License file

### 🗑️ Deprecated
- Legacy AES-128-CBC implementation (moved to `*_legacy.rs` files)

### ❌ Removed
- Shared secret authentication system
- `-s/--secret` command line options

### 🔧 Fixed
- GitHub Actions workflow deprecation warnings
- Rust compilation errors and clippy warnings
- Cross-platform compatibility issues
- Memory safety improvements with Rust implementation

## [Unreleased]

### Security
- Consider implementing post-quantum cryptography when standards mature
- Evaluate additional hardening measures for production deployments
