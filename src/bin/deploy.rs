use std::fs;
use std::path::{Path, PathBuf};

fn find_steam_cs_path_linux() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let candidates = [
        format!("{home}/.local/share/Steam/steamapps/common/Half-Life/cstrike/cl_dlls"),
        format!("{home}/.steam/steam/steamapps/common/Half-Life/cstrike/cl_dlls"),
        format!("{home}/.var/app/com.valvesoftware.Steam/data/Steam/steamapps/common/Half-Life/cstrike/cl_dlls"),
    ];
    for c in candidates {
        let p = PathBuf::from(&c);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

#[cfg(windows)]
fn find_steam_cs_path_windows() -> Option<PathBuf> {
    // use std::io::Read;

    let default_paths = [
        "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Half-Life",
        "C:\\Program Files\\Steam\\steamapps\\common\\Half-Life",
        "D:\\SteamLibrary\\steamapps\\common\\Half-Life",
        "E:\\SteamLibrary\\steamapps\\common\\Half-Life",
        "F:\\SteamLibrary\\steamapps\\common\\Half-Life",
        "D:\\Steam\\steamapps\\common\\Half-Life",
        "E:\\Steam\\steamapps\\common\\Half-Life",
        "C:\\Games\\CS 1.6",
        "C:\\Games\\Counter-Strike 1.6",
        "D:\\Games\\CS 1.6",
        "D:\\Games\\Counter-Strike 1.6",
    ];

    for base in default_paths {
        let p = Path::new(base).join("cstrike").join("cl_dlls");
        if p.exists() {
            return Some(p);
        }
    }

    let output = std::process::Command::new("reg")
        .args(["query", r"HKCU\Software\Valve\Steam", "/v", "SteamPath"])
        .output()
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.contains("SteamPath") {
                if let Some(pos) = line.find("REG_SZ") {
                    let steam_dir = line[pos + 6..].trim();
                    let hl_dir = Path::new(steam_dir)
                        .join("steamapps")
                        .join("common")
                        .join("Half-Life");
                    let cl_dlls = hl_dir.join("cstrike").join("cl_dlls");
                    if cl_dlls.exists() {
                        return Some(cl_dlls);
                    }
                }
            }
        }
    }

    None
}

#[cfg(not(windows))]
fn find_steam_cs_path_windows() -> Option<PathBuf> {
    None
}

fn main() {
    println!("[deploy] Locating CS 1.6 / cstrike directory...");

    let dest_dir = if cfg!(windows) {
        find_steam_cs_path_windows()
    } else {
        find_steam_cs_path_linux()
    };

    let Some(target_dir) = dest_dir else {
        eprintln!(
            "[deploy] Warning: Could not automatically locate CS 1.6 cstrike/cl_dlls/ directory."
        );
        eprintln!("[deploy] Please specify path manually or copy target binary into your cstrike/cl_dlls/ folder.");
        return;
    };

    println!("[deploy] Found destination: {}", target_dir.display());

    let (src_file, dst_file, orig_backup) = if cfg!(windows) {
        let src_candidates = [
            "target/i686-pc-windows-msvc/release/csldr.dll",
            "target/i686-pc-windows-gnu/release/csldr.dll",
            "target/release/csldr.dll",
            "target/i686-pc-windows-msvc/debug/csldr.dll",
            "target/i686-pc-windows-gnu/debug/csldr.dll",
        ];
        let found = src_candidates
            .iter()
            .find(|s| Path::new(s).exists())
            .copied()
            .unwrap_or("target/i686-pc-windows-msvc/release/csldr.dll");

        (
            PathBuf::from(found),
            target_dir.join("client.dll"),
            target_dir.join("client_orig.dll"),
        )
    } else {
        let src_candidates = [
            "target/i686-unknown-linux-gnu/release/libcsldr.so",
            "target/release/libcsldr.so",
            "target/i686-unknown-linux-gnu/debug/libcsldr.so",
        ];
        let found = src_candidates
            .iter()
            .find(|s| Path::new(s).exists())
            .copied()
            .unwrap_or("target/i686-unknown-linux-gnu/release/libcsldr.so");

        (
            PathBuf::from(found),
            target_dir.join("client.so"),
            target_dir.join("client_orig.so"),
        )
    };

    if !src_file.exists() {
        eprintln!(
            "[deploy] Source file '{}' does not exist yet. Run cargo build first.",
            src_file.display()
        );
        return;
    }

    if dst_file.exists() && !orig_backup.exists() {
        let _ = fs::copy(&dst_file, &orig_backup);
        println!(
            "[deploy] Backed up original client to {}",
            orig_backup.display()
        );
    }

    match fs::copy(&src_file, &dst_file) {
        Ok(_) => println!(
            "[deploy] Successfully deployed '{}' -> '{}'",
            src_file.display(),
            dst_file.display()
        ),
        Err(e) => eprintln!(
            "[deploy] Failed to copy '{}' to '{}': {}",
            src_file.display(),
            dst_file.display(),
            e
        ),
    }
}
