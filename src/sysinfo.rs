use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Serialize, Deserialize)]
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
        serde_json::to_vec(self).unwrap_or_default()
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
