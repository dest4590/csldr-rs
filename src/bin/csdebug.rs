use std::env;
use std::ffi::c_void;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::os::raw::c_int;
use std::path::Path;
use std::process;
use std::ptr;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
struct MemoryMap {
    start: usize,
    end: usize,
    perms: String,
    offset: usize,
    pathname: String,
}

fn parse_maps(pid: c_int) -> Vec<MemoryMap> {
    let maps_path = format!("/proc/{}/maps", pid);
    let mut maps = Vec::new();

    if let Ok(file) = File::open(&maps_path) {
        let reader = BufReader::new(file);
        for line in reader.lines().flatten() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 {
                let range_parts: Vec<&str> = parts[0].split('-').collect();
                if range_parts.len() == 2 {
                    let start = usize::from_str_radix(range_parts[0], 16).unwrap_or(0);
                    let end = usize::from_str_radix(range_parts[1], 16).unwrap_or(0);
                    let perms = parts[1].to_string();
                    let offset = usize::from_str_radix(parts[2], 16).unwrap_or(0);
                    let pathname = if parts.len() >= 6 {
                        parts[5..].join(" ")
                    } else {
                        String::new()
                    };

                    maps.push(MemoryMap {
                        start,
                        end,
                        perms,
                        offset,
                        pathname,
                    });
                }
            }
        }
    }

    maps
}

fn resolve_address(maps: &[MemoryMap], addr: usize) -> String {
    for map in maps {
        if addr >= map.start && addr < map.end {
            let rel = addr - map.start + map.offset;
            let name = if map.pathname.is_empty() {
                "[anonymous]"
            } else {
                Path::new(&map.pathname)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&map.pathname)
            };
            return format!("0x{:x} ({}+0x{:x} perms={})", addr, name, rel, map.perms);
        }
    }
    format!("0x{:x} [unmapped]", addr)
}

fn read_proc_mem(pid: c_int, addr: usize, len: usize) -> Vec<u8> {
    let mem_path = format!("/proc/{}/mem", pid);
    let mut buffer = vec![0u8; len];
    if let Ok(mut file) = File::open(&mem_path) {
        if file.seek(SeekFrom::Start(addr as u64)).is_ok() {
            let _ = file.read_exact(&mut buffer);
        }
    }
    buffer
}

fn find_pids_by_name(name: &str) -> Vec<c_int> {
    let mut pids = Vec::new();
    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(pid_str) = path.file_name().and_then(|n| n.to_str()) {
                if let Ok(pid) = pid_str.parse::<c_int>() {
                    let cmdline_path = format!("/proc/{}/cmdline", pid);
                    if let Ok(cmdline) = fs::read_to_string(&cmdline_path) {
                        if cmdline.contains(name) {
                            pids.push(pid);
                        }
                    }
                }
            }
        }
    }
    pids
}

fn get_thread_tids(pid: c_int) -> Vec<c_int> {
    let mut tids = Vec::new();
    let task_dir = format!("/proc/{}/task", pid);
    if let Ok(entries) = fs::read_dir(&task_dir) {
        for entry in entries.flatten() {
            if let Some(tid_str) = entry.file_name().to_str() {
                if let Ok(tid) = tid_str.parse::<c_int>() {
                    tids.push(tid);
                }
            }
        }
    }
    if tids.is_empty() {
        tids.push(pid);
    }
    tids
}

fn log_msg(log_file: &mut Option<File>, msg: &str) {
    println!("{}", msg);
    if let Some(f) = log_file {
        let _ = writeln!(f, "{}", msg);
    }
}

fn ptrace_attach(tid: c_int) -> bool {
    let res = unsafe {
        libc::ptrace(
            libc::PTRACE_ATTACH,
            tid,
            ptr::null_mut::<c_void>(),
            ptr::null_mut::<c_void>(),
        )
    };
    res == 0
}

fn ptrace_cont(tid: c_int, sig: c_int) {
    unsafe {
        libc::ptrace(
            libc::PTRACE_CONT,
            tid,
            ptr::null_mut::<c_void>(),
            sig as *mut c_void,
        );
    }
}

#[cfg(target_arch = "x86_64")]
fn dump_registers_and_stack(
    tid: c_int,
    pid: c_int,
    maps: &[MemoryMap],
    log_file: &mut Option<File>,
) {
    let mut regs: libc::user_regs_struct = unsafe { std::mem::zeroed() };
    let res = unsafe {
        libc::ptrace(
            libc::PTRACE_GETREGS,
            tid,
            ptr::null_mut::<c_void>(),
            &mut regs as *mut _ as *mut c_void,
        )
    };

    if res != 0 {
        log_msg(
            log_file,
            &format!("[CSDEBUG] Failed to get registers for TID {}", tid),
        );
        return;
    }

    let rip = regs.rip as usize;
    let rsp = regs.rsp as usize;
    let rbp = regs.rbp as usize;

    log_msg(
        log_file,
        "================================================================================",
    );
    log_msg(log_file, &format!("[CSDEBUG] REGISTERS (TID {}):", tid));
    log_msg(
        log_file,
        &format!(
            "  RIP: {} | RSP: 0x{:016x} | RBP: 0x{:016x}",
            resolve_address(maps, rip),
            rsp,
            rbp
        ),
    );
    log_msg(
        log_file,
        &format!(
            "  RAX: 0x{:016x} | RBX: 0x{:016x} | RCX: 0x{:016x}",
            regs.rax, regs.rbx, regs.rcx
        ),
    );
    log_msg(
        log_file,
        &format!(
            "  RDX: 0x{:016x} | RSI: 0x{:016x} | RDI: 0x{:016x}",
            regs.rdx, regs.rsi, regs.rdi
        ),
    );
    log_msg(
        log_file,
        &format!(
            "  R8:  0x{:016x} | R9:  0x{:016x} | R10: 0x{:016x}",
            regs.r8, regs.r9, regs.r10
        ),
    );
    log_msg(
        log_file,
        &format!(
            "  R11: 0x{:016x} | R12: 0x{:016x} | R13: 0x{:016x}",
            regs.r11, regs.r12, regs.r13
        ),
    );
    log_msg(
        log_file,
        &format!(
            "  R14: 0x{:016x} | R15: 0x{:016x} | EFLAGS: 0x{:08x}",
            regs.r14, regs.r15, regs.eflags
        ),
    );

    let ins_bytes = read_proc_mem(pid, rip, 16);
    let bytes_hex: Vec<String> = ins_bytes.iter().map(|b| format!("{:02x}", b)).collect();
    log_msg(
        log_file,
        &format!("  INSTRUCTIONS AT RIP: [{}]", bytes_hex.join(" ")),
    );

    log_msg(log_file, "\n[CSDEBUG] STACK BACKTRACE:");
    let stack_data = read_proc_mem(pid, rsp, 512);
    let mut frame_count = 0;
    for chunk in stack_data.chunks_exact(8) {
        let val = usize::from_ne_bytes(chunk.try_into().unwrap());
        for map in maps {
            if val >= map.start && val < map.end && map.perms.contains('x') {
                let resolved = resolve_address(maps, val);
                log_msg(log_file, &format!("  #{:<2} {}", frame_count, resolved));
                frame_count += 1;
                if frame_count >= 32 {
                    break;
                }
            }
        }
    }
    log_msg(
        log_file,
        "================================================================================",
    );
}

#[cfg(target_arch = "x86")]
fn dump_registers_and_stack(
    tid: c_int,
    pid: c_int,
    maps: &[MemoryMap],
    log_file: &mut Option<File>,
) {
    let mut regs: libc::user_regs_struct = unsafe { std::mem::zeroed() };
    let res = unsafe {
        libc::ptrace(
            libc::PTRACE_GETREGS,
            tid,
            ptr::null_mut::<c_void>(),
            &mut regs as *mut _ as *mut c_void,
        )
    };

    if res != 0 {
        log_msg(
            log_file,
            &format!("[CSDEBUG] Failed to get registers for TID {}", tid),
        );
        return;
    }

    let eip = regs.eip as usize;
    let esp = regs.esp as usize;
    let ebp = regs.ebp as usize;

    log_msg(
        log_file,
        "================================================================================",
    );
    log_msg(log_file, &format!("[CSDEBUG] REGISTERS (TID {}):", tid));
    log_msg(
        log_file,
        &format!(
            "  EIP: {} | ESP: 0x{:08x} | EBP: 0x{:08x}",
            resolve_address(maps, eip),
            esp,
            ebp
        ),
    );
    log_msg(
        log_file,
        &format!(
            "  EAX: 0x{:08x} | EBX: 0x{:08x} | ECX: 0x{:08x}",
            regs.eax, regs.ebx, regs.ecx
        ),
    );
    log_msg(
        log_file,
        &format!(
            "  EDX: 0x{:08x} | ESI: 0x{:08x} | EDI: 0x{:08x}",
            regs.edx, regs.esi, regs.edi
        ),
    );
    log_msg(log_file, &format!("  EFLAGS: 0x{:08x}", regs.eflags));

    let ins_bytes = read_proc_mem(pid, eip, 16);
    let bytes_hex: Vec<String> = ins_bytes.iter().map(|b| format!("{:02x}", b)).collect();
    log_msg(
        log_file,
        &format!("  INSTRUCTIONS AT EIP: [{}]", bytes_hex.join(" ")),
    );

    log_msg(log_file, "\n[CSDEBUG] STACK BACKTRACE:");
    let stack_data = read_proc_mem(pid, esp, 256);
    let mut frame_count = 0;
    for chunk in stack_data.chunks_exact(4) {
        let val = u32::from_ne_bytes(chunk.try_into().unwrap()) as usize;
        for map in maps {
            if val >= map.start && val < map.end && map.perms.contains('x') {
                let resolved = resolve_address(maps, val);
                log_msg(log_file, &format!("  #{:<2} {}", frame_count, resolved));
                frame_count += 1;
                if frame_count >= 32 {
                    break;
                }
            }
        }
    }
    log_msg(
        log_file,
        "================================================================================",
    );
}

fn signal_name(sig: c_int) -> &'static str {
    match sig {
        libc::SIGSEGV => "SIGSEGV (Segmentation Fault)",
        libc::SIGBUS => "SIGBUS (Bus Error)",
        libc::SIGILL => "SIGILL (Illegal Instruction)",
        libc::SIGFPE => "SIGFPE (Floating Point Exception)",
        libc::SIGABRT => "SIGABRT (Aborted)",
        libc::SIGTRAP => "SIGTRAP (Trace/Breakpoint)",
        libc::SIGINT => "SIGINT (Interrupt)",
        libc::SIGKILL => "SIGKILL (Killed)",
        libc::SIGTERM => "SIGTERM (Terminated)",
        _ => "Unknown Signal",
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut target_pid: Option<c_int> = None;
    let mut run_command: Option<Vec<String>> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-p" | "--pid" => {
                if i + 1 < args.len() {
                    target_pid = args[i + 1].parse().ok();
                    i += 1;
                }
            }
            "-n" | "--name" => {
                if i + 1 < args.len() {
                    let name = &args[i + 1];
                    let found = find_pids_by_name(name);
                    if let Some(&pid) = found.first() {
                        println!("[CSDEBUG] Found process '{}' with PID {}", name, pid);
                        target_pid = Some(pid);
                    } else {
                        eprintln!("[CSDEBUG ERROR] No process matching '{}' found!", name);
                        process::exit(1);
                    }
                    i += 1;
                }
            }
            "-r" | "--run" => {
                if i + 1 < args.len() {
                    run_command = Some(args[i + 1..].to_vec());
                    break;
                }
            }
            arg => {
                if let Ok(pid) = arg.parse::<c_int>() {
                    target_pid = Some(pid);
                }
            }
        }
        i += 1;
    }

    let mut _child_process: Option<std::process::Child> = None;

    if let Some(cmd) = run_command {
        if cmd.is_empty() {
            eprintln!("[CSDEBUG ERROR] --run requires a program name");
            process::exit(1);
        }

        let program = &cmd[0];
        let program_args = &cmd[1..];

        let run_dir = std::path::Path::new(program)
            .canonicalize()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));

        println!(
            "[CSDEBUG] Launching '{}' with args: {:?}",
            program, program_args
        );
        if let Some(ref dir) = run_dir {
            println!("[CSDEBUG] Working directory: {}", dir.display());
        }

        let mut command = std::process::Command::new(program);
        command.args(program_args);
        if let Some(ref dir) = run_dir {
            command.current_dir(dir);

            let game_dir = dir.to_string_lossy().to_string();
            let steam_home = std::env::var("HOME").unwrap_or_default();
            let paths = [
                game_dir,
                format!(
                    "{}/.local/share/Steam/ubuntu12_32/steam-runtime/usr/lib/i386-linux-gnu",
                    steam_home
                ),
                format!(
                    "{}/.local/share/Steam/ubuntu12_32/steam-runtime.old/usr/lib/i386-linux-gnu",
                    steam_home
                ),
                format!("{}/.local/share/Steam/ubuntu12_32", steam_home),
                format!("{}/.local/share/Steam/linux32", steam_home),
                "/usr/lib/i386-linux-gnu".to_string(),
                "/usr/lib32".to_string(),
                "/lib/i386-linux-gnu".to_string(),
                "/lib32".to_string(),
            ];
            let new_path = paths.join(":");
            command.env("LD_LIBRARY_PATH", &new_path);
            println!("[CSDEBUG] LD_LIBRARY_PATH={}", new_path);

            let forward_vars = [
                "DISPLAY",
                "WAYLAND_DISPLAY",
                "XAUTHORITY",
                "XDG_RUNTIME_DIR",
                "SDL_AUDIODRIVER",
                "PULSE_SERVER",
                "PULSE_SINK",
                "PULSE_SOURCE",
                "DBUS_SESSION_BUS_ADDRESS",
                "PIPEWIRE_RUNTIME_DIR",
                "PIPEWIRE_LATENCY",
            ];
            for var in &forward_vars {
                if let Ok(val) = std::env::var(var) {
                    command.env(var, &val);
                }
            }
        }

        match command.spawn() {
            Ok(child) => {
                let pid = child.id() as c_int;
                println!("[CSDEBUG] Launched process with PID {}", pid);
                target_pid = Some(pid);
                _child_process = Some(child);
            }
            Err(e) => {
                eprintln!("[CSDEBUG ERROR] Failed to launch '{}': {}", program, e);
                process::exit(1);
            }
        }
    }

    let pid = match target_pid {
        Some(p) => p,
        None => {
            println!(
                "Usage: csdebug <PID> | -p <PID> | -n <process_name> | -r <program> [args...]"
            );
            println!("Examples:");
            println!("  csdebug -r hl_linux -game czero -windowed");
            println!("  csdebug -n hl_linux");
            process::exit(1);
        }
    };

    let mut log_file = File::create("csdebug.log").ok();
    log_msg(
        &mut log_file,
        &format!("[CSDEBUG] Attaching to PID {}...", pid),
    );

    let tids = get_thread_tids(pid);
    let mut attached_count = 0;
    for &tid in &tids {
        if ptrace_attach(tid) {
            attached_count += 1;
        } else {
            log_msg(
                &mut log_file,
                &format!("[CSDEBUG WARNING] Failed to attach to TID {}", tid),
            );
        }
    }

    if attached_count == 0 {
        log_msg(&mut log_file, "[CSDEBUG ERROR] Could not attach to any thread! Check permissions (sudo / ptrace_scope).");
        process::exit(1);
    }

    log_msg(&mut log_file, &format!("[CSDEBUG SUCCESS] Attached to {} thread(s). Monitoring signals (press Ctrl+C to detach)...", attached_count));

    for &tid in &tids {
        let mut status: c_int = 0;
        unsafe {
            libc::waitpid(tid, &mut status, 0);
        }
        ptrace_cont(tid, 0);
    }

    loop {
        let mut status: c_int = 0;
        let wtid = unsafe { libc::waitpid(-1, &mut status, 0) };
        if wtid <= 0 {
            thread::sleep(Duration::from_millis(10));
            continue;
        }

        if libc::WIFSTOPPED(status) {
            let sig = libc::WSTOPSIG(status);
            if sig == libc::SIGSEGV
                || sig == libc::SIGBUS
                || sig == libc::SIGILL
                || sig == libc::SIGFPE
                || sig == libc::SIGABRT
            {
                log_msg(&mut log_file, "\n********************************************************************************");
                log_msg(
                    &mut log_file,
                    &format!(
                        "[CSDEBUG CRASH DETECTED!] Thread {} received signal {} ({})",
                        wtid,
                        sig,
                        signal_name(sig)
                    ),
                );
                log_msg(&mut log_file, "********************************************************************************");

                let maps = parse_maps(pid);
                dump_registers_and_stack(wtid, pid, &maps, &mut log_file);

                log_msg(
                    &mut log_file,
                    "[CSDEBUG] Crash log written to 'csdebug.log'. Forwarding signal...",
                );
                ptrace_cont(wtid, sig);
            } else if sig == libc::SIGTRAP {
                ptrace_cont(wtid, 0);
            } else {
                ptrace_cont(wtid, sig);
            }
        } else if libc::WIFEXITED(status) {
            log_msg(
                &mut log_file,
                &format!(
                    "[CSDEBUG] Thread {} exited with status {}",
                    wtid,
                    libc::WEXITSTATUS(status)
                ),
            );
        } else if libc::WIFSIGNALED(status) {
            log_msg(
                &mut log_file,
                &format!(
                    "[CSDEBUG] Thread {} terminated by signal {}",
                    wtid,
                    libc::WTERMSIG(status)
                ),
            );
        }
    }
}
