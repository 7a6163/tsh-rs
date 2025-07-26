# Makefile for tsh-rs

# Default target
.PHONY: all
all: linux

# Build for Linux x64
.PHONY: linux
linux:
	@echo "Building Linux x64 binaries..."
	@mkdir -p build/linux-x64
	cargo build --release --target x86_64-unknown-linux-gnu
	cp target/x86_64-unknown-linux-gnu/release/tsh build/linux-x64/
	cp target/x86_64-unknown-linux-gnu/release/tshd build/linux-x64/

# Build for Windows x64
.PHONY: windows
windows:
	@echo "Building Windows x64 binaries..."
	@mkdir -p build/windows-x64
	cargo build --release --target x86_64-pc-windows-gnu
	cp target/x86_64-pc-windows-gnu/release/tsh.exe build/windows-x64/
	cp target/x86_64-pc-windows-gnu/release/tshd.exe build/windows-x64/

# Build for macOS (both x64 and ARM64)
.PHONY: macos
macos: macos-x64 macos-arm64

.PHONY: macos-x64
macos-x64:
	@echo "Building macOS x64 binaries..."
	@mkdir -p build/macos-x64
	cargo build --release --target x86_64-apple-darwin
	cp target/x86_64-apple-darwin/release/tsh build/macos-x64/
	cp target/x86_64-apple-darwin/release/tshd build/macos-x64/

.PHONY: macos-arm64
macos-arm64:
	@echo "Building macOS ARM64 binaries..."
	@mkdir -p build/macos-arm64
	cargo build --release --target aarch64-apple-darwin
	cp target/aarch64-apple-darwin/release/tsh build/macos-arm64/
	cp target/aarch64-apple-darwin/release/tshd build/macos-arm64/

# Build for custom GOOS/GOARCH (use environment variables)
.PHONY: unix
unix:
	@echo "Building for custom target: $(TARGET)"
	@if [ -z "$(TARGET)" ]; then echo "Usage: make unix TARGET=<rust-target-triple>"; exit 1; fi
	@mkdir -p build/$(TARGET)
	cargo build --release --target $(TARGET)
	cp target/$(TARGET)/release/tsh build/$(TARGET)/ 2>/dev/null || cp target/$(TARGET)/release/tsh.exe build/$(TARGET)/ 2>/dev/null || true
	cp target/$(TARGET)/release/tshd build/$(TARGET)/ 2>/dev/null || cp target/$(TARGET)/release/tshd.exe build/$(TARGET)/ 2>/dev/null || true

# Development build (debug mode)
.PHONY: dev
dev:
	@echo "Building development binaries..."
	cargo build

# Run tests
.PHONY: test
test:
	@echo "Running tests..."
	cargo test

# Run client
.PHONY: run-client
run-client:
	cargo run --bin tsh -- $(ARGS)

# Run server
.PHONY: run-server
run-server:
	cargo run --bin tshd -- $(ARGS)

# Format code
.PHONY: fmt
fmt:
	cargo fmt

# Run clippy linter
.PHONY: clippy
clippy:
	cargo clippy -- -D warnings

# Check code without building
.PHONY: check
check:
	cargo check

# Clean build artifacts
.PHONY: clean
clean:
	@echo "Cleaning build artifacts..."
	cargo clean
	rm -rf build/

# Install required targets for cross-compilation
.PHONY: install-targets
install-targets:
	@echo "Installing cross-compilation targets..."
	rustup target add x86_64-unknown-linux-gnu
	rustup target add x86_64-pc-windows-gnu
	rustup target add x86_64-apple-darwin
	rustup target add aarch64-apple-darwin

# Show available Rust targets
.PHONY: list-targets
list-targets:
	@echo "Available Rust targets:"
	rustup target list | grep -E "(linux|windows|darwin|freebsd|openbsd|netbsd)"

# Help
.PHONY: help
help:
	@echo "Available targets:"
	@echo "  all          - Build Linux x64 binaries (default)"
	@echo "  linux        - Build Linux x64 binaries"
	@echo "  windows      - Build Windows x64 binaries"
	@echo "  macos        - Build macOS binaries (both x64 and ARM64)"
	@echo "  macos-x64    - Build macOS x64 binaries"
	@echo "  macos-arm64  - Build macOS ARM64 binaries"
	@echo "  unix         - Build for custom target (set TARGET env var)"
	@echo "  dev          - Build development binaries"
	@echo "  test         - Run tests"
	@echo "  run-client   - Run client (set ARGS for arguments)"
	@echo "  run-server   - Run server (set ARGS for arguments)"
	@echo "  fmt          - Format code"
	@echo "  clippy       - Run clippy linter"
	@echo "  check        - Check code without building"
	@echo "  clean        - Clean build artifacts"
	@echo "  install-targets - Install cross-compilation targets"
	@echo "  list-targets - Show available Rust targets"
	@echo "  help         - Show this help"
	@echo ""
	@echo "Examples:"
	@echo "  make linux"
	@echo "  make unix TARGET=aarch64-unknown-linux-gnu"
	@echo "  make run-client ARGS='-s mysecret -p 8080 192.168.1.100'"
	@echo "  make run-server ARGS='-s mysecret -p 8080'"
