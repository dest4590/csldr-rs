use std::ffi::{c_char, c_int, CStr, CString};
use std::sync::atomic::{AtomicPtr, Ordering};

use crate::engine::sdk::ClEngineFuncs;

static ORIG_CLIENT_CMD: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
static ORIG_FILTERED_CLIENT_CMD: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

pub unsafe fn init_cmd_hooks(penginefuncs: *mut ClEngineFuncs) {
    if penginefuncs.is_null() {
        return;
    }

    let funcs = &mut *penginefuncs;

    if !funcs.pfnClientCmd.is_null() {
        ORIG_CLIENT_CMD.store(funcs.pfnClientCmd, Ordering::Relaxed);
        funcs.pfnClientCmd = hk_pfn_client_cmd as *mut _;
        crate::engine::cdll_int::G_ENGFUNCS.pfnClientCmd = hk_pfn_client_cmd as *mut _;
    }

    if !funcs.pfnFilteredClientCmd.is_null() {
        ORIG_FILTERED_CLIENT_CMD.store(funcs.pfnFilteredClientCmd, Ordering::Relaxed);
        funcs.pfnFilteredClientCmd = hk_pfn_filtered_client_cmd as *mut _;
        crate::engine::cdll_int::G_ENGFUNCS.pfnFilteredClientCmd =
            hk_pfn_filtered_client_cmd as *mut _;
    }
}

pub unsafe extern "C" fn hk_pfn_client_cmd(sz_cmd_string: *const c_char) -> c_int {
    let ptr = ORIG_CLIENT_CMD.load(Ordering::Relaxed);
    if ptr.is_null() {
        return 0;
    }
    let orig: unsafe extern "C" fn(*const c_char) -> c_int = std::mem::transmute(ptr);

    if sz_cmd_string.is_null() {
        return orig(sz_cmd_string);
    }

    if let Ok(cmd_cstr) = CStr::from_ptr(sz_cmd_string).to_str() {
        if let Some(new_cmd) = process_command(cmd_cstr) {
            if let Ok(c_str) = CString::new(new_cmd) {
                return orig(c_str.as_ptr());
            }
        }
    }

    orig(sz_cmd_string)
}

pub unsafe extern "C" fn hk_pfn_filtered_client_cmd(sz_cmd_string: *const c_char) -> c_int {
    let ptr = ORIG_FILTERED_CLIENT_CMD.load(Ordering::Relaxed);
    if ptr.is_null() {
        return 0;
    }
    let orig: unsafe extern "C" fn(*const c_char) -> c_int = std::mem::transmute(ptr);

    if sz_cmd_string.is_null() {
        return orig(sz_cmd_string);
    }

    if let Ok(cmd_cstr) = CStr::from_ptr(sz_cmd_string).to_str() {
        if let Some(new_cmd) = process_command(cmd_cstr) {
            if let Ok(c_str) = CString::new(new_cmd) {
                return orig(c_str.as_ptr());
            }
        }
    }

    orig(sz_cmd_string)
}

pub fn process_command(cmd_str: &str) -> Option<String> {
    if cmd_str.is_empty() {
        return None;
    }

    let first_cmd_end = cmd_str.find([';', '\n', '\r']).unwrap_or(cmd_str.len());
    let first_cmd_raw = &cmd_str[..first_cmd_end];
    let rest = &cmd_str[first_cmd_end..];

    let trimmed = first_cmd_raw.trim().trim_matches(|c| c == '"' || c == '\'');

    if is_server_address(trimmed) {
        let mut new_cmd = String::with_capacity(cmd_str.len() + 16);
        new_cmd.push_str("connect ");
        new_cmd.push_str(trimmed);
        new_cmd.push_str(rest);
        if !new_cmd.ends_with('\n') {
            new_cmd.push('\n');
        }
        Some(new_cmd)
    } else {
        None
    }
}

pub fn is_server_address(token: &str) -> bool {
    let token = token.trim().trim_matches(|c| c == '"' || c == '\'');
    if token.is_empty() || token.chars().any(|c| c.is_whitespace()) {
        return false;
    }

    if token.contains("://") {
        return false;
    }

    if token.parse::<std::net::Ipv4Addr>().is_ok() {
        return true;
    }

    if token.parse::<std::net::Ipv6Addr>().is_ok() {
        return true;
    }

    if let Some((host_part, port_part)) = token.rsplit_once(':') {
        if let Ok(port) = port_part.parse::<u16>() {
            if port > 0 {
                if host_part.parse::<std::net::Ipv4Addr>().is_ok() {
                    return true;
                }

                if host_part.starts_with('[')
                    && host_part.ends_with(']')
                    && host_part.len() > 2
                    && host_part[1..host_part.len() - 1]
                        .parse::<std::net::Ipv6Addr>()
                        .is_ok()
                {
                    return true;
                }

                if is_valid_hostname(host_part) {
                    return true;
                }
            }
        }
    }

    false
}

fn is_valid_hostname(host: &str) -> bool {
    if host.is_empty()
        || host.starts_with('.')
        || host.ends_with('.')
        || host.starts_with('-')
        || host.ends_with('-')
    {
        return false;
    }

    let mut has_letter_or_dot = false;
    for c in host.chars() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_alphabetic() {
                has_letter_or_dot = true;
            }
        } else if c == '.' {
            has_letter_or_dot = true;
        } else if c != '-' {
            return false;
        }
    }

    has_letter_or_dot
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_server_address() {
        assert!(is_server_address("91.211.118.77:27025"));
        assert!(is_server_address("91.211.118.77"));
        assert!(is_server_address("127.0.0.1:27015"));
        assert!(is_server_address("192.168.1.1"));
        assert!(is_server_address("[::1]:27015"));
        assert!(is_server_address("cs.example.com:27015"));
        assert!(is_server_address("play.server.ru:27025"));
        assert!(is_server_address("  91.211.118.77:27025  "));
        assert!(is_server_address("\"91.211.118.77:27025\""));
        assert!(is_server_address("localhost:27015"));

        assert!(!is_server_address("connect 91.211.118.77:27025"));
        assert!(!is_server_address("say 91.211.118.77:27025"));
        assert!(!is_server_address("fps_max 100"));
        assert!(!is_server_address("cl_crosshair_color 255"));
        assert!(!is_server_address("sensitivity 2.5"));
        assert!(!is_server_address("quit"));
        assert!(!is_server_address("status"));
        assert!(!is_server_address("100"));
        assert!(!is_server_address("1.0"));
        assert!(!is_server_address("1.2.3"));
        assert!(!is_server_address("http://example.com"));
        assert!(!is_server_address(""));
    }

    #[test]
    fn test_process_command() {
        assert_eq!(
            process_command("91.211.118.77:27025"),
            Some("connect 91.211.118.77:27025\n".to_string())
        );
        assert_eq!(
            process_command("91.211.118.77:27025\n"),
            Some("connect 91.211.118.77:27025\n".to_string())
        );
        assert_eq!(
            process_command("91.211.118.77:27025; password 123"),
            Some("connect 91.211.118.77:27025; password 123\n".to_string())
        );
        assert_eq!(process_command("connect 91.211.118.77:27025"), None);
        assert_eq!(process_command("fps_max 100"), None);
    }
}
