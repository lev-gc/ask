use crate::cli::EnvLevel;
use std::fmt::Write as _;
use std::path::Path;

#[derive(Debug, Default)]
pub struct EnvInfo {
    pub distro: Option<String>,
    pub distro_version: Option<String>,
    pub kernel: Option<String>,
    pub shell: Option<String>,
    pub root: bool,
    pub user: Option<String>,
    pub sudo_nopasswd: Option<bool>,
    pub in_git: Option<bool>,
    pub cwd: Option<String>,
    pub tools: Vec<(&'static str, bool)>,
}

pub fn probe(level: EnvLevel) -> EnvInfo {
    let mut info = EnvInfo::default();
    read_os_release(&mut info);
    info.kernel = kernel_release();
    info.shell = std::env::var("SHELL")
        .ok()
        .and_then(|p| Path::new(&p).file_name().map(|f| f.to_string_lossy().into_owned()));
    info.root = unsafe { libc_geteuid() } == 0;

    if matches!(level, EnvLevel::Full | EnvLevel::Tools) {
        info.user = std::env::var("USER").ok();
        info.cwd = std::env::current_dir()
            .ok()
            .map(|p| p.to_string_lossy().into_owned());
        info.in_git = Some(in_git_repo());
        if !info.root {
            info.sudo_nopasswd = Some(has_nopasswd_sudo());
        }
    }
    if matches!(level, EnvLevel::Tools) {
        const PROBES: &[&str] = &[
            "apt", "dnf", "yum", "pacman", "zypper", "brew", "systemctl", "docker", "kubectl",
            "git", "curl", "wget",
        ];
        let path_dirs: Vec<_> = std::env::var("PATH")
            .ok()
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default();
        info.tools = PROBES
            .iter()
            .map(|t| (*t, which_in(t, &path_dirs)))
            .collect();
    }
    info
}

pub fn format_for_prompt(info: &EnvInfo) -> String {
    let mut s = String::new();
    if let (Some(d), Some(v)) = (&info.distro, &info.distro_version) {
        let _ = writeln!(s, "distro={} {}", d, v);
    } else if let Some(d) = &info.distro {
        let _ = writeln!(s, "distro={}", d);
    }
    if let Some(k) = &info.kernel {
        let _ = writeln!(s, "kernel={}", k);
    }
    if let Some(sh) = &info.shell {
        let _ = writeln!(s, "shell={}", sh);
    }
    let _ = writeln!(s, "root={}", info.root);
    if let Some(u) = &info.user {
        let _ = writeln!(s, "user={}", u);
    }
    if let Some(b) = info.sudo_nopasswd {
        let _ = writeln!(s, "sudo_nopasswd={}", b);
    }
    if let Some(b) = info.in_git {
        let _ = writeln!(s, "in_git_repo={}", b);
    }
    if let Some(c) = &info.cwd {
        let _ = writeln!(s, "cwd={}", c);
    }
    if !info.tools.is_empty() {
        let available: Vec<_> = info.tools.iter().filter(|(_, v)| *v).map(|(n, _)| *n).collect();
        let missing: Vec<_> = info.tools.iter().filter(|(_, v)| !*v).map(|(n, _)| *n).collect();
        if !available.is_empty() {
            let _ = writeln!(s, "tools_available={}", available.join(","));
        }
        if !missing.is_empty() {
            let _ = writeln!(s, "tools_missing={}", missing.join(","));
        }
    }
    s
}

fn read_os_release(info: &mut EnvInfo) {
    let text = match std::fs::read_to_string("/etc/os-release") {
        Ok(t) => t,
        Err(_) => return,
    };
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("ID=") {
            info.distro = Some(strip_quotes(v).to_string());
        } else if let Some(v) = line.strip_prefix("VERSION_ID=") {
            info.distro_version = Some(strip_quotes(v).to_string());
        }
    }
}

fn strip_quotes(s: &str) -> &str {
    let s = s.trim();
    s.trim_matches('"').trim_matches('\'')
}

fn kernel_release() -> Option<String> {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .ok()
        .map(|s| s.trim().to_string())
}

fn in_git_repo() -> bool {
    let mut cur = match std::env::current_dir() {
        Ok(p) => p,
        Err(_) => return false,
    };
    loop {
        if cur.join(".git").exists() {
            return true;
        }
        if !cur.pop() {
            return false;
        }
    }
}

fn has_nopasswd_sudo() -> bool {
    use std::process::{Command, Stdio};
    Command::new("sudo")
        .args(["-n", "true"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn which_in(name: &str, dirs: &[std::path::PathBuf]) -> bool {
    dirs.iter().any(|d| {
        let p = d.join(name);
        p.is_file()
    })
}

// Minimal libc binding to avoid pulling the `libc` crate.
unsafe fn libc_geteuid() -> u32 {
    extern "C" {
        fn geteuid() -> u32;
    }
    geteuid()
}
