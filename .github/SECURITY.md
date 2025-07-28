# Security Policy

## 🔒 Security Notice

**tsh-rs is a legitimate remote administration tool designed for authorized system administration and security testing.**

⚠️ **Important:** This tool provides remote shell access and may be flagged by antivirus software as potentially unwanted software. This is a false positive due to the nature of remote access tools.

## 🎯 Intended Use

This tool should **ONLY** be used for:

- ✅ System administration on systems you own
- ✅ Remote troubleshooting with proper authorization  
- ✅ Authorized security testing and penetration testing
- ✅ Development environment management
- ✅ Educational purposes in controlled environments

## ⛔ Prohibited Use

**DO NOT** use this tool for:

- ❌ Unauthorized access to systems
- ❌ Malicious activities or attacks
- ❌ Bypassing security controls without permission
- ❌ Any illegal activities

## 🛡️ Security Features

### Encryption
- AES-128-CBC encryption for data confidentiality
- HMAC-SHA1 for message integrity and authentication
- Random IV generation for each session
- Challenge-response authentication

### Implementation Security
- Memory-safe Rust implementation
- Structured error handling
- Input validation and sanitization
- No hardcoded credentials (except default examples)

## 🐛 Reporting Security Vulnerabilities

We take security seriously. If you discover a security vulnerability, please report it responsibly:

### Reporting Process

1. **DO NOT** create a public GitHub issue for security vulnerabilities
2. Email security reports to: [your-security-email@domain.com]
3. Include the following information:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested remediation (if any)

### Response Timeline

- **Initial Response**: Within 48 hours
- **Vulnerability Assessment**: Within 1 week
- **Fix Development**: Depends on severity and complexity
- **Public Disclosure**: After fix is released (coordinated disclosure)

## 🔍 Security Best Practices

### For Users

1. **Use Strong Secrets**: Never use default passwords in production
   ```bash
   # Good
   ./tsh -s "MySecurePassword123!" -p 8080 target
   
   # Bad
   ./tsh -s "1234" -p 8080 target
   ```

2. **Network Security**: Use firewalls and VPNs when possible
   ```bash
   # Bind to specific interface
   ./tsh server --psk "MySecureKey123!" --port 8080
   ```

3. **Monitor Connections**: Log and monitor all connections
   ```bash
   # Enable verbose logging
   RUST_LOG=info ./tsh server --psk "MySecureKey123!" --port 8080
   ```

4. **Regular Updates**: Keep tsh-rs updated to the latest version

### For Developers

1. **Code Review**: All security-related changes require review
2. **Static Analysis**: Use cargo clippy and cargo audit
3. **Dependency Management**: Regularly update dependencies
4. **Testing**: Include security test cases

## 🔐 Cryptographic Details

### Encryption Implementation
- **Protocol**: Noise Protocol Framework (Noise_XX_25519_ChaChaPoly_BLAKE2s)
- **Encryption**: ChaCha20-Poly1305 AEAD (Authenticated Encryption with Associated Data)
- **Key Exchange**: X25519 Elliptic Curve Diffie-Hellman
- **Hashing**: BLAKE2s cryptographic hash function
- **Authentication**: Pre-shared key (PSK) with challenge-response
- **Forward Secrecy**: Yes, through ephemeral key exchange

### Security Features
- Modern cryptographic protocols with proven security
- Resistance to quantum attacks on key exchange
- Memory-safe implementation in Rust
- No known cryptographic vulnerabilities

## 📋 Security Checklist

Before deploying tsh-rs:

- [ ] Generated strong pre-shared key (PSK)
- [ ] Configured appropriate firewall rules
- [ ] Enabled logging and monitoring (RUST_LOG=info)
- [ ] Tested in controlled environment
- [ ] Verified authorized use only
- [ ] Updated to latest version
- [ ] Reviewed cryptographic configuration
- [ ] Secured PSK storage and distribution
- [ ] Reviewed security configurations

## 📞 Contact

- **General Issues**: Create a GitHub issue
- **Security Issues**: [your-security-email@domain.com]
- **Project Maintainer**: [your-email@domain.com]

## 📄 Legal Notice

By using this software, you agree to use it only for legal and authorized purposes. The developers are not responsible for any misuse of this tool. Users are solely responsible for ensuring they have proper authorization before using this tool on any system.

---

**Remember: With great power comes great responsibility. Use tsh-rs ethically and legally.**