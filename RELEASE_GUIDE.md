# 🚀 Release Guide for tsh-rs

This guide explains how to create releases for tsh-rs using GitHub Actions.

## 📋 Prerequisites

1. **Repository Setup**: Ensure the repository has GitHub Actions enabled
2. **Permissions**: You need write access to create releases
3. **Secrets**: No additional secrets required (uses `GITHUB_TOKEN`)

## 🔄 Release Process

### Automated Release (Recommended)

#### 1. Create and Push a Tag
```bash
# Make sure you're on the main branch
git checkout main
git pull origin main

# Create a new tag (follow semver: v1.0.0, v1.1.0, v2.0.0)
git tag v1.0.0

# Push the tag to trigger the release workflow
git push origin v1.0.0
```

#### 2. Monitor the Release Workflow
- Go to your repository's **Actions** tab
- Watch the "Release" workflow progress
- The workflow will:
  - Build binaries for all platforms
  - Create a GitHub release
  - Upload all artifacts
  - Generate checksums

### Manual Release (Alternative)

If you need to create a release without creating a tag:

1. Go to **Actions** tab in your repository
2. Select the "Release" workflow
3. Click "Run workflow"
4. Enter the release tag (e.g., `v1.0.0`)
5. Click "Run workflow"

## 📦 What Gets Built

The release workflow automatically builds binaries for:

| Platform | Architecture | File Name |
|----------|-------------|-----------|
| Linux | x86_64 | `tsh-linux-x64.tar.gz` |
| Linux | ARM64 | `tsh-linux-arm64.tar.gz` |
| Windows | x86_64 | `tsh-windows-x64.zip` |
| macOS | x86_64 | `tsh-macos-x64.tar.gz` |
| macOS | ARM64 | `tsh-macos-arm64.tar.gz` |

## 📁 Release Contents

Each release archive contains:
- `tsh` / `tsh.exe` - Client binary
- `tshd` / `tshd.exe` - Server binary
- `README.md` - Project documentation
- `demo.md` - Usage examples
- `Makefile` - Build instructions
- `install.sh` / `install.bat` - Installation script
- `checksums.txt` - SHA256 checksums

## 🔍 Quality Checks

Before release, the CI workflow runs:
- ✅ Code formatting check (`cargo fmt`)
- ✅ Linting (`cargo clippy`)
- ✅ Tests (`cargo test`)
- ✅ Security audit (`cargo audit`)
- ✅ Documentation check (`cargo doc`)

## 🏷️ Version Naming

Follow [Semantic Versioning](https://semver.org/):

- **MAJOR** version (`v2.0.0`): Incompatible API changes
- **MINOR** version (`v1.1.0`): New functionality (backward compatible)
- **PATCH** version (`v1.0.1`): Bug fixes (backward compatible)

### Examples:
```bash
# First release
git tag v1.0.0

# New feature added
git tag v1.1.0

# Bug fix
git tag v1.0.1

# Breaking changes
git tag v2.0.0
```

## 🔧 Pre-Release Checklist

Before creating a release:

- [ ] Update version in `Cargo.toml` if needed
- [ ] Update `CHANGELOG.md` (if you have one)
- [ ] Test the build locally: `cargo build --release`
- [ ] Run security audit: `cargo audit`
- [ ] Verify documentation: `cargo doc`
- [ ] Test on different platforms if possible

## 🐛 Troubleshooting

### Build Failures

**Problem**: Cross-compilation fails for ARM64 Linux
```
Solution: The workflow installs gcc-aarch64-linux-gnu automatically
```

**Problem**: Windows build fails
```
Solution: Check if any Unix-specific code was added
```

**Problem**: macOS build fails
```
Solution: Ensure no Linux-specific dependencies
```

### Release Creation Fails

**Problem**: Permission denied when creating release
```
Solution: Check repository settings -> Actions -> General -> Workflow permissions
Ensure "Read and write permissions" is selected
```

**Problem**: Tag already exists
```bash
# Delete local tag
git tag -d v1.0.0

# Delete remote tag
git push origin --delete v1.0.0

# Create new tag
git tag v1.0.0
git push origin v1.0.0
```

## 📞 Support

If you encounter issues with releases:

1. Check the **Actions** tab for error details
2. Review this guide for common solutions
3. Create an issue in the repository
4. Contact the maintainers

## 🔒 Security Note

All releases are automatically scanned for:
- Known vulnerabilities in dependencies
- License compliance
- Binary security features

The release includes checksums for integrity verification:
```bash
# Verify download integrity
sha256sum -c checksums.txt
```

## 📈 Post-Release

After a successful release:

1. **Announcement**: Consider announcing on relevant platforms
2. **Documentation**: Update any external documentation
3. **Testing**: Test the released binaries on clean systems
4. **Feedback**: Monitor for user reports and feedback

---

**Remember**: Each release represents the project to users. Take time to ensure quality and test thoroughly!
