use crate::hooks::sound::{
    is_local_entity_or_near, origin_from_ptr, process_hrtf_sound, resolve_sound_world_origin,
    scaled_sound_volume,
};
use std::ffi::{c_char, c_int, c_void, CStr};
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

type FnStartSound = unsafe extern "C" fn(
    entnum: c_int,
    entchannel: c_int,
    sfx: *const c_void,
    origin: *const f32,
    fvol: f32,
    attenuation: f32,
    flags: c_int,
    pitch: c_int,
);

macro_rules! ffi_ptr { ($(#[$meta:meta])* static $name:ident) => {
    $(#[$meta])* static $name: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
}; }
macro_rules! call_engine_trampoline {
    ($tramp:expr, $sig:ty, $($arg:expr),*) => {
        {
            let ptr = $tramp.load(std::sync::atomic::Ordering::Relaxed);
            if !ptr.is_null() {
                let f: $sig = std::mem::transmute(ptr);
                f($($arg),*);
            }
        }
    };
}

ffi_ptr!(static TRAMPOLINE_DYNAMIC_SOUND);
ffi_ptr!(static TRAMPOLINE_STATIC_SOUND);
static ENGINE_SOUND_HOOKS_INSTALLED: AtomicBool = AtomicBool::new(false);

unsafe fn hook_start_sound_common(
    entnum: c_int,
    entchannel: c_int,
    sfx: *const c_void,
    origin: *const f32,
    fvol: f32,
    attenuation: f32,
    flags: c_int,
    pitch: c_int,
    trampoline: &AtomicPtr<c_void>,
    is_dynamic: bool,
) {
    let sound_name = if sfx.is_null() || (sfx as usize) < 0x10000 {
        std::borrow::Cow::Borrowed("")
    } else {
        let first_byte = *(sfx as *const u8);
        if (0x20..=0x7E).contains(&first_byte) {
            CStr::from_ptr(sfx as *const c_char).to_string_lossy()
        } else {
            let inner_ptr = *(sfx as *const *const c_char);
            if (inner_ptr as usize) > 0x10000
                && (0x20..=0x7E).contains(&(*(inner_ptr as *const u8)))
            {
                CStr::from_ptr(inner_ptr).to_string_lossy()
            } else {
                std::borrow::Cow::Borrowed("")
            }
        }
    };

    let orig_raw = origin_from_ptr(origin);

    if !sound_name.is_empty() {
        crate::con_println!(
            "[CSLDR-SND] {} sound: '{}', ent: {}, origin: {:?}, vol: {}",
            if is_dynamic { "Dynamic" } else { "Static" },
            sound_name,
            entnum,
            orig_raw,
            fvol
        );
    }

    if is_dynamic && sound_name.is_empty() {
        call_engine_trampoline!(
            trampoline,
            FnStartSound,
            entnum,
            entchannel,
            sfx,
            origin,
            fvol,
            attenuation,
            flags,
            pitch
        );
        return;
    }

    let is_local = is_local_entity_or_near(entnum, orig_raw);
    let effective_volume = scaled_sound_volume(fvol);
    let sound_pos = if is_local {
        None
    } else {
        resolve_sound_world_origin(origin, entnum)
    };

    // Calculate HRTF and acoustics, but preserve a sensible audible floor for gameplay sounds
    let final_vol = process_hrtf_sound(&sound_name, effective_volume, sound_pos);
    let mut final_pitch = pitch;

    if let Some(spos) = sound_pos {
        let listener = crate::hooks::sound::get_listener_state();
        let dx = listener.origin[0] - spos[0];
        let dy = listener.origin[1] - spos[1];
        let dz = listener.origin[2] - spos[2];
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        if dist > 1.0 {
            let dir = [dx / dist, dy / dist, dz / dist];
            if entnum > 0 {
                if let Some(get_ent) = crate::engine::cdll_int::G_ENGFUNCS.GetEntityByIndex {
                    let ent_ptr = get_ent(entnum) as *const crate::engine::sdk::ClEntity;
                    if (ent_ptr as usize) > 0x10000 {
                        let ent_vel = (*ent_ptr).curstate.velocity;
                        let rel_speed = (ent_vel[0] * dir[0]
                            + ent_vel[1] * dir[1]
                            + ent_vel[2] * dir[2]) as f32;
                        let speed_of_sound_units: f32 = 13500.0;
                        let doppler_shift =
                            (1.0_f32 + (rel_speed / speed_of_sound_units)).clamp(0.70, 1.45);
                        final_pitch = ((pitch.max(100) as f32) * doppler_shift).round() as c_int;
                    }
                }
            }
        }
    }

    // Never drop sound below 0.30 volume in engine trampoline to prevent dropouts
    let engine_vol = if final_vol > 0.0 {
        final_vol.max(0.30)
    } else {
        fvol
    };

    call_engine_trampoline!(
        trampoline,
        FnStartSound,
        entnum,
        entchannel,
        sfx,
        origin,
        engine_vol,
        attenuation,
        flags,
        final_pitch
    );
}

pub unsafe extern "C" fn hk_s_start_dynamic_sound(
    entnum: c_int,
    entchannel: c_int,
    sfx: *const c_void,
    origin: *const f32,
    fvol: f32,
    attenuation: f32,
    flags: c_int,
    pitch: c_int,
) {
    hook_start_sound_common(
        entnum,
        entchannel,
        sfx,
        origin,
        fvol,
        attenuation,
        flags,
        pitch,
        &TRAMPOLINE_DYNAMIC_SOUND,
        true,
    );
}

pub unsafe extern "C" fn hk_s_start_static_sound(
    entnum: c_int,
    entchannel: c_int,
    sfx: *const c_void,
    origin: *const f32,
    fvol: f32,
    attenuation: f32,
    flags: c_int,
    pitch: c_int,
) {
    hook_start_sound_common(
        entnum,
        entchannel,
        sfx,
        origin,
        fvol,
        attenuation,
        flags,
        pitch,
        &TRAMPOLINE_STATIC_SOUND,
        false,
    );
}

#[cfg(all(unix, target_pointer_width = "32"))]
mod inline_hook {
    use super::*;

    const JMP_SIZE: usize = 5;

    struct DlPhdrData {
        target_name: &'static str,
        base_addr: usize,
    }

    unsafe extern "C" fn find_module_callback(
        info: *mut libc::dl_phdr_info,
        _size: libc::size_t,
        data: *mut c_void,
    ) -> c_int {
        if info.is_null() || data.is_null() {
            return 0;
        }

        let info = &*info;
        let pdata = &mut *(data as *mut DlPhdrData);

        if info.dlpi_name.is_null() {
            return 0;
        }

        let name = CStr::from_ptr(info.dlpi_name).to_string_lossy();
        if name.ends_with(pdata.target_name) {
            pdata.base_addr = info.dlpi_addr as usize;
            return 1;
        }

        0
    }

    unsafe fn get_hw_module_base() -> Option<usize> {
        let mut data = DlPhdrData {
            target_name: "hw.so",
            base_addr: 0,
        };

        libc::dl_iterate_phdr(
            Some(find_module_callback),
            &mut data as *mut _ as *mut c_void,
        );

        if data.base_addr != 0 {
            Some(data.base_addr)
        } else {
            None
        }
    }

    unsafe fn create_inline_hook(
        target_addr: usize,
        hook_fn: usize,
        stolen_bytes_len: usize,
    ) -> Option<*mut c_void> {
        if stolen_bytes_len < JMP_SIZE {
            return None;
        }

        let tramp_size = stolen_bytes_len + JMP_SIZE;
        let tramp = libc::mmap(
            std::ptr::null_mut(),
            tramp_size,
            libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        );

        if tramp == libc::MAP_FAILED {
            return None;
        }

        let tramp_ptr = tramp as *mut u8;

        // copy stolen prologue instructions to trampoline
        std::ptr::copy_nonoverlapping(target_addr as *const u8, tramp_ptr, stolen_bytes_len);

        // write JMP back from trampoline to target_addr + stolen_bytes_len
        let jmp_back_src = (tramp_ptr as usize) + stolen_bytes_len;
        let jmp_back_dst = target_addr + stolen_bytes_len;
        let jmp_back_rel = (jmp_back_dst as isize) - (jmp_back_src as isize + 5);

        *tramp_ptr.add(stolen_bytes_len) = 0xE9;
        let rel_back_bytes = (jmp_back_rel as i32).to_le_bytes();
        std::ptr::copy_nonoverlapping(
            rel_back_bytes.as_ptr(),
            tramp_ptr.add(stolen_bytes_len + 1),
            4,
        );

        // patch target function with JMP to hook_fn
        let page_size = libc::sysconf(libc::_SC_PAGESIZE) as usize;
        let page_start = target_addr & !(page_size - 1);

        if libc::mprotect(
            page_start as *mut c_void,
            page_size * 2,
            libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
        ) != 0
        {
            libc::munmap(tramp, tramp_size);
            return None;
        }

        let target_ptr = target_addr as *mut u8;
        let jmp_rel = (hook_fn as isize) - (target_addr as isize + 5);

        *target_ptr = 0xE9;
        let rel_bytes = (jmp_rel as i32).to_le_bytes();
        std::ptr::copy_nonoverlapping(rel_bytes.as_ptr(), target_ptr.add(1), 4);

        // NOP any remaining stolen bytes
        for i in JMP_SIZE..stolen_bytes_len {
            *target_ptr.add(i) = 0x90;
        }

        libc::mprotect(
            page_start as *mut c_void,
            page_size * 2,
            libc::PROT_READ | libc::PROT_EXEC,
        );

        Some(tramp)
    }

    pub unsafe fn install() {
        let base = match get_hw_module_base() {
            Some(b) => b,
            None => return,
        };

        // Offsets from hw.so symbol table:
        // S_StartDynamicSound: 0x00197a00 (prologue: 83 ec 0c 83 3d e4 77 82 00 00 -> 10 bytes)
        // S_StartStaticSound:  0x00197f20 (prologue: 55 57 56 53 81 ec 8c 00 00 00 -> 10 bytes)
        let dynamic_addr = base + 0x00197a00;
        let static_addr = base + 0x00197f20;

        if let Some(tramp) = create_inline_hook(
            dynamic_addr,
            hk_s_start_dynamic_sound as *const () as usize,
            10,
        ) {
            TRAMPOLINE_DYNAMIC_SOUND.store(tramp, Ordering::Relaxed);
            crate::con_println!("[HRTF] S_StartDynamicSound hooked (Linux i386)");
        }

        if let Some(tramp) = create_inline_hook(
            static_addr,
            hk_s_start_static_sound as *const () as usize,
            10,
        ) {
            TRAMPOLINE_STATIC_SOUND.store(tramp, Ordering::Relaxed);
            crate::con_println!("[HRTF] S_StartStaticSound hooked (Linux i386)");
        }
    }
}

#[cfg(all(windows, target_pointer_width = "32"))]
mod win_inline_hook {
    use super::*;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;
    use windows_sys::Win32::System::Memory::{
        VirtualAlloc, VirtualProtect, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE,
    };

    const JMP_SIZE: usize = 5;

    unsafe fn create_inline_hook(
        target_addr: usize,
        hook_fn: usize,
        stolen_bytes_len: usize,
    ) -> Option<*mut c_void> {
        if stolen_bytes_len < JMP_SIZE {
            return None;
        }

        let tramp_size = stolen_bytes_len + JMP_SIZE;
        let tramp = VirtualAlloc(
            std::ptr::null_mut(),
            tramp_size,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_EXECUTE_READWRITE,
        );

        if tramp.is_null() {
            return None;
        }

        let tramp_ptr = tramp as *mut u8;

        // copy stolen prologue instructions to trampoline
        std::ptr::copy_nonoverlapping(target_addr as *const u8, tramp_ptr, stolen_bytes_len);

        // write JMP back from trampoline to target_addr + stolen_bytes_len
        let jmp_back_src = (tramp_ptr as usize) + stolen_bytes_len;
        let jmp_back_dst = target_addr + stolen_bytes_len;
        let jmp_back_rel = (jmp_back_dst as isize) - (jmp_back_src as isize + 5);

        *tramp_ptr.add(stolen_bytes_len) = 0xE9;
        let rel_back_bytes = (jmp_back_rel as i32).to_le_bytes();
        std::ptr::copy_nonoverlapping(
            rel_back_bytes.as_ptr(),
            tramp_ptr.add(stolen_bytes_len + 1),
            4,
        );

        // patch target function with JMP to hook_fn
        let mut old_protect = 0u32;
        if VirtualProtect(
            target_addr as *mut c_void,
            stolen_bytes_len,
            PAGE_EXECUTE_READWRITE,
            &mut old_protect,
        ) == 0
        {
            return None;
        }

        let target_ptr = target_addr as *mut u8;
        *target_ptr = 0xE9;
        let jmp_rel = (hook_fn as isize) - (target_addr as isize + 5);
        let rel_bytes = (jmp_rel as i32).to_le_bytes();
        std::ptr::copy_nonoverlapping(rel_bytes.as_ptr(), target_ptr.add(1), 4);

        // NOP remaining stolen bytes
        for i in 5..stolen_bytes_len {
            *target_ptr.add(i) = 0x90;
        }

        VirtualProtect(
            target_addr as *mut c_void,
            stolen_bytes_len,
            old_protect,
            &mut old_protect,
        );

        Some(tramp)
    }

    pub unsafe fn install() {
        let h_hw = GetModuleHandleA(b"hw.dll\0".as_ptr() as _);
        if h_hw.is_null() {
            return;
        }
        let base = h_hw as usize;

        // Get module size from PE header
        let dos_header = base as *const u8;
        let e_lfanew = *(dos_header.add(0x3C) as *const i32) as usize;
        let opt_header = (base + e_lfanew + 0x18) as *const u8;
        let size_of_image = *(opt_header.add(0x38) as *const u32) as usize;

        // Dynamic string xref & pattern scan with fallback to known offsets
        let dynamic_addr = crate::hooks::scanner::find_function_by_string_xref(
            base,
            size_of_image,
            b"S_StartDynamicSound:",
        )
        .or_else(|| {
            let pat = crate::hooks::scanner::Pattern::new(
                &[0x55, 0x8B, 0xEC, 0x83, 0xEC, 0x5C],
                b"xxxxxx",
            );
            pat.scan_in_range(base, size_of_image)
        })
        .unwrap_or(base + 0x1FE3E0);

        let static_addr = crate::hooks::scanner::find_function_by_string_xref(
            base,
            size_of_image,
            b"S_StartStaticSound:",
        )
        .or_else(|| {
            let pat = crate::hooks::scanner::Pattern::new(
                &[0x55, 0x8B, 0xEC, 0x83, 0xEC, 0x50],
                b"xxxxxx",
            );
            pat.scan_in_range(base, size_of_image)
        })
        .unwrap_or(base + 0x1FE780);

        // Verify function prologs (55 8b ec -> push ebp; mov ebp, esp)
        let is_valid_prolog = |addr: usize| {
            let bytes = std::slice::from_raw_parts(addr as *const u8, 3);
            bytes == [0x55, 0x8B, 0xEC] || bytes == [0x55, 0x89, 0xE5]
        };

        if is_valid_prolog(dynamic_addr) {
            if let Some(tramp) = create_inline_hook(
                dynamic_addr,
                hk_s_start_dynamic_sound as *const () as usize,
                6,
            ) {
                TRAMPOLINE_DYNAMIC_SOUND.store(tramp, Ordering::Relaxed);
                crate::con_println!(
                    "[HRTF] S_StartDynamicSound hooked (Windows x86 at 0x{:X})",
                    dynamic_addr
                );
            }
        }

        if is_valid_prolog(static_addr) {
            if let Some(tramp) = create_inline_hook(
                static_addr,
                hk_s_start_static_sound as *const () as usize,
                6,
            ) {
                TRAMPOLINE_STATIC_SOUND.store(tramp, Ordering::Relaxed);
                crate::con_println!(
                    "[HRTF] S_StartStaticSound hooked (Windows x86 at 0x{:X})",
                    static_addr
                );
            }
        }
    }
}

pub unsafe fn install_engine_sound_hooks() {
    if ENGINE_SOUND_HOOKS_INSTALLED.swap(true, Ordering::Relaxed) {
        return;
    }

    #[cfg(all(unix, target_pointer_width = "32"))]
    inline_hook::install();

    #[cfg(all(windows, target_pointer_width = "32"))]
    win_inline_hook::install();
}
