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
        let json = format!(
            r#"{{"hostname":"{}","os":"{}","arch":"{}","username":"{}","home_dir":"{}","current_dir":"{}","pid":{},"is_elevated":{}}}"#,
            escape_json(&self.hostname),
            escape_json(&self.os),
            escape_json(&self.arch),
            escape_json(&self.username),
            escape_json(&self.home_dir),
            escape_json(&self.current_dir),
            self.pid,
            self.is_elevated,
        );
        json.into_bytes()
    }

    pub fn from_json_bytes(bytes: &[u8]) -> Option<Self> {
        let s = std::str::from_utf8(bytes).ok()?;
        Some(Self {
            hostname: extract_json_string(s, "hostname")?,
            os: extract_json_string(s, "os")?,
            arch: extract_json_string(s, "arch")?,
            username: extract_json_string(s, "username")?,
            home_dir: extract_json_string(s, "home_dir")?,
            current_dir: extract_json_string(s, "current_dir")?,
            pid: extract_json_number(s, "pid")? as u32,
            is_elevated: extract_json_bool(s, "is_elevated")?,
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

// ─── Minimal JSON helpers (no serde dependency) ─────────────────────────────

pub fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

pub fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\":", key);
    let after_colon = json.find(&pattern)? + pattern.len();
    let rest = json[after_colon..].trim_start();
    if !rest.starts_with('"') {
        return None; // null or non-string
    }
    let rest = &rest[1..]; // skip opening quote
    let mut end = 0;
    let mut escaped = false;
    for ch in rest.chars() {
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            break;
        }
        end += ch.len_utf8();
    }
    Some(
        rest[..end]
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
            .replace("\\n", "\n")
            .replace("\\r", "\r")
            .replace("\\t", "\t"),
    )
}

pub fn extract_json_number(json: &str, key: &str) -> Option<u64> {
    let pattern = format!("\"{}\":", key);
    let start = json.find(&pattern)? + pattern.len();
    let rest = json[start..].trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

pub fn extract_json_bool(json: &str, key: &str) -> Option<bool> {
    let pattern = format!("\"{}\":", key);
    let start = json.find(&pattern)? + pattern.len();
    let rest = json[start..].trim_start();
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

// ─── Platform-specific helpers ──────────────────────────────────────────────

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
