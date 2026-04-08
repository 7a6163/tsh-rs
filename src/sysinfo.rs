use serde_json::{json, Value};
use std::env;

#[derive(Debug)]
pub struct SystemInfo {
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub username: String,
    pub home_dir: String,
    pub current_dir: String,
    pub pid: u32,
    pub is_elevated: bool,
}

impl SystemInfo {
    pub fn collect() -> Self {
        Self {
            hostname: get_hostname(),
            os: format!("{} {}", env::consts::OS, env::consts::FAMILY),
            arch: env::consts::ARCH.to_string(),
            username: get_username(),
            home_dir: get_home_dir(),
            current_dir: env::current_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "unknown".to_string()),
            pid: std::process::id(),
            is_elevated: is_elevated(),
        }
    }

    pub fn to_json_bytes(&self) -> Vec<u8> {
        let value = json!({
            "hostname": self.hostname,
            "os": self.os,
            "arch": self.arch,
            "username": self.username,
            "home_dir": self.home_dir,
            "current_dir": self.current_dir,
            "pid": self.pid,
            "is_elevated": self.is_elevated,
        });
        serde_json::to_vec(&value).unwrap_or_default()
    }

    pub fn from_json_bytes(bytes: &[u8]) -> Option<Self> {
        let v: Value = serde_json::from_slice(bytes).ok()?;
        Some(Self {
            hostname: v["hostname"].as_str()?.to_string(),
            os: v["os"].as_str()?.to_string(),
            arch: v["arch"].as_str()?.to_string(),
            username: v["username"].as_str()?.to_string(),
            home_dir: v["home_dir"].as_str()?.to_string(),
            current_dir: v["current_dir"].as_str()?.to_string(),
            pid: v["pid"].as_u64()? as u32,
            is_elevated: v["is_elevated"].as_bool()?,
        })
    }

    pub fn display(&self) -> String {
        let privilege = if self.is_elevated {
            "root/admin"
        } else {
            "normal"
        };
        format!(
            "\n--- Agent System Info ---\n\
             Hostname : {}\n\
             OS       : {}\n\
             Arch     : {}\n\
             User     : {} ({})\n\
             Home     : {}\n\
             CWD      : {}\n\
             PID      : {}\n\
             ----------------------------\n",
            self.hostname,
            self.os,
            self.arch,
            self.username,
            privilege,
            self.home_dir,
            self.current_dir,
            self.pid,
        )
    }
}

fn get_hostname() -> String {
    #[cfg(unix)]
    {
        let mut buf = [0u8; 256];
        let ret = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
        if ret == 0 {
            let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
            String::from_utf8_lossy(&buf[..len]).into_owned()
        } else {
            "unknown".to_string()
        }
    }
    #[cfg(windows)]
    {
        env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown".to_string())
    }
}

fn get_username() -> String {
    #[cfg(unix)]
    {
        env::var("USER").unwrap_or_else(|_| "unknown".to_string())
    }
    #[cfg(windows)]
    {
        env::var("USERNAME").unwrap_or_else(|_| "unknown".to_string())
    }
}

fn get_home_dir() -> String {
    #[cfg(unix)]
    {
        env::var("HOME").unwrap_or_else(|_| "unknown".to_string())
    }
    #[cfg(windows)]
    {
        env::var("USERPROFILE").unwrap_or_else(|_| "unknown".to_string())
    }
}

fn is_elevated() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(windows)]
    {
        std::fs::metadata("C:\\Windows\\System32\\config\\SAM").is_ok()
    }
}
