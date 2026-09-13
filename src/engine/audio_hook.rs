use crate::engine::acoustics::get_current_acoustics;
use crate::engine::native_hrtf::NATIVE_HRTF_ENGINE;
use crate::engine::sound_hook::is_hrtf_pcm_bypassed;
use crate::utils::cvars::{
    HRTF_ACOUSTICS, HRTF_DEBUG, HRTF_ENABLE, HRTF_REVERB_SCALE, HRTF_VOLUME,
};
use std::ffi::{c_char, c_void, CStr, CString};
use std::fs;
use std::os::raw::c_int;
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};
use std::sync::Mutex;

type SdlAudioDeviceId = u32;
type SdlAudioFormat = u16;

const AUDIO_S16LSB: SdlAudioFormat = 0x8010;
const AUDIO_F32LSB: SdlAudioFormat = 0x8120;
const DT_NULL: u64 = 0;
const DT_STRTAB: u64 = 5;
const DT_SYMTAB: u64 = 6;
const DT_REL: u64 = 17;
const DT_RELSZ: u64 = 18;
const DT_PLTRELSZ: u64 = 2;
const DT_JMPREL: u64 = 23;

#[cfg(target_endian = "little")]
const AUDIO_S16SYS: SdlAudioFormat = AUDIO_S16LSB;
#[cfg(target_endian = "little")]
const AUDIO_F32SYS: SdlAudioFormat = AUDIO_F32LSB;

type SdlAudioCallback = unsafe extern "C" fn(*mut c_void, *mut u8, c_int);
type SdlOpenAudioDevice = unsafe extern "C" fn(
    *const c_char,
    c_int,
    *const SdlAudioSpec,
    *mut SdlAudioSpec,
    c_int,
) -> SdlAudioDeviceId;
type SdlOpenAudio = unsafe extern "C" fn(*const SdlAudioSpec, *mut SdlAudioSpec) -> c_int;
type SdlQueueAudio = unsafe extern "C" fn(SdlAudioDeviceId, *const c_void, u32) -> c_int;
type SdlCloseAudioDevice = unsafe extern "C" fn(SdlAudioDeviceId);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SdlAudioSpec {
    pub freq: c_int,
    pub format: SdlAudioFormat,
    pub channels: u8,
    pub silence: u8,
    pub samples: u16,
    pub padding: u16,
    pub size: u32,
    pub callback: Option<SdlAudioCallback>,
    pub userdata: *mut c_void,
}

#[derive(Clone, Copy, Debug)]
struct DeviceAudioFormat {
    device_id: SdlAudioDeviceId,
    sample_rate: c_int,
    format: SdlAudioFormat,
    channels: u8,
}

static SDL_DEVICE_FORMATS: Mutex<Vec<DeviceAudioFormat>> = Mutex::new(Vec::new());
macro_rules! ffi_ptr { ($(#[$meta:meta])* static $name:ident) => { $(#[$meta])* static $name: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut()); }; }
ffi_ptr!(static ORIG_SDL_OPEN_AUDIO_DEVICE);
ffi_ptr!(static ORIG_SDL_OPEN_AUDIO);
ffi_ptr!(static ORIG_SDL_QUEUE_AUDIO);
ffi_ptr!(static ORIG_SDL_CLOSE_AUDIO_DEVICE);
static AUDIO_HOOK_PATCHES: AtomicU32 = AtomicU32::new(0);
static AUDIO_DEVICE_OPENS: AtomicU32 = AtomicU32::new(0);
static AUDIO_QUEUE_HITS: AtomicU32 = AtomicU32::new(0);
static AUDIO_CALLBACK_HITS: AtomicU32 = AtomicU32::new(0);
static LAST_AUDIO_HOOK_SCAN: Mutex<f32> = Mutex::new(-9999.0);
static SDL_CALLBACK_HOOKS: Mutex<Vec<CallbackHookEntry>> = Mutex::new(Vec::new());

#[derive(Clone, Copy)]
struct HookSpec {
    name: &'static [u8],
    replacement: *mut c_void,
}

struct CallbackHookEntry {
    device_id: SdlAudioDeviceId,
    state: *mut HookedAudioCallbackState,
}

unsafe impl Send for CallbackHookEntry {}

struct HookedAudioCallbackState {
    original_callback: SdlAudioCallback,
    original_userdata: *mut c_void,
    sample_rate: c_int,
    format: SdlAudioFormat,
    channels: u8,
}

fn console_print(message: &str) {
    crate::utils::console::con_print_str(message);
}

fn debug_log(message: &str) {
    if HRTF_DEBUG.value_or(0.0) != 0.0 {
        console_print(message);
    }
}

fn register_callback_hook(device_id: SdlAudioDeviceId, state: *mut HookedAudioCallbackState) {
    if device_id == 0 || state.is_null() {
        return;
    }

    if let Ok(mut hooks) = SDL_CALLBACK_HOOKS.lock() {
        if let Some(existing) = hooks.iter_mut().find(|entry| entry.device_id == device_id) {
            if existing.state != state && !existing.state.is_null() {
                unsafe {
                    drop(Box::from_raw(existing.state));
                }
            }
            existing.state = state;
        } else {
            hooks.push(CallbackHookEntry { device_id, state });
        }
    }
}

fn unregister_callback_hook(device_id: SdlAudioDeviceId) {
    if let Ok(mut hooks) = SDL_CALLBACK_HOOKS.lock() {
        if let Some(idx) = hooks.iter().position(|entry| entry.device_id == device_id) {
            let entry = hooks.swap_remove(idx);
            if !entry.state.is_null() {
                unsafe {
                    drop(Box::from_raw(entry.state));
                }
            }
        }
    }
}

unsafe extern "C" fn hrtf_sdl_audio_callback(userdata: *mut c_void, stream: *mut u8, len: c_int) {
    if userdata.is_null() || stream.is_null() || len <= 0 {
        return;
    }

    let state = &mut *(userdata as *mut HookedAudioCallbackState);
    (state.original_callback)(state.original_userdata, stream, len);

    if is_hrtf_pcm_bypassed() {
        return;
    }

    let hits = AUDIO_CALLBACK_HITS.fetch_add(1, Ordering::Relaxed) + 1;
    if hits == 1 {
        debug_log(&format!(
            "[HRTF PCM] first SDL callback intercept rate={} fmt=0x{:x} channels={} bytes={}\n",
            state.sample_rate, state.format, state.channels, len
        ));
    }

    match state.format {
        AUDIO_S16SYS
            if state.channels == 2 && (len as usize).is_multiple_of(std::mem::size_of::<i16>()) =>
        {
            let sample_count = len as usize / std::mem::size_of::<i16>();
            let buffer = std::slice::from_raw_parts_mut(stream as *mut i16, sample_count);
            let _ = process_interleaved_i16(buffer, state.sample_rate as f32);
        }
        AUDIO_F32SYS
            if state.channels == 2 && (len as usize).is_multiple_of(std::mem::size_of::<f32>()) =>
        {
            let sample_count = len as usize / std::mem::size_of::<f32>();
            let buffer = std::slice::from_raw_parts_mut(stream as *mut f32, sample_count);
            let _ = process_interleaved_f32(buffer, state.sample_rate as f32);
        }
        _ => {}
    }
}

fn remember_device_format(device_id: SdlAudioDeviceId, spec: &SdlAudioSpec) {
    if device_id == 0 || spec.channels < 2 || spec.freq <= 0 {
        return;
    }

    AUDIO_DEVICE_OPENS.fetch_add(1, Ordering::Relaxed);

    if let Ok(mut formats) = SDL_DEVICE_FORMATS.lock() {
        if let Some(existing) = formats
            .iter_mut()
            .find(|entry| entry.device_id == device_id)
        {
            existing.sample_rate = spec.freq;
            existing.format = spec.format;
            existing.channels = spec.channels;
        } else {
            formats.push(DeviceAudioFormat {
                device_id,
                sample_rate: spec.freq,
                format: spec.format,
                channels: spec.channels,
            });
        }
    }

    debug_log(&format!(
        "[HRTF PCM] device={} rate={} fmt=0x{:x} channels={} registered\n",
        device_id, spec.freq, spec.format, spec.channels
    ));
}

fn forget_device_format(device_id: SdlAudioDeviceId) {
    if let Ok(mut formats) = SDL_DEVICE_FORMATS.lock() {
        formats.retain(|entry| entry.device_id != device_id);
    }
}

fn get_device_format(device_id: SdlAudioDeviceId) -> Option<DeviceAudioFormat> {
    SDL_DEVICE_FORMATS.lock().ok().and_then(|formats| {
        formats
            .iter()
            .find(|entry| entry.device_id == device_id)
            .copied()
    })
}

fn process_pcm_i16(device: DeviceAudioFormat, data: *const c_void, len: usize) -> Option<Vec<u8>> {
    if device.channels < 2 || len == 0 || !len.is_multiple_of(2) {
        return None;
    }

    let frame_width = usize::from(device.channels) * std::mem::size_of::<i16>();
    if frame_width == 0 || !len.is_multiple_of(frame_width) {
        return None;
    }

    let bytes = unsafe { std::slice::from_raw_parts(data as *const u8, len) };
    let mut samples = Vec::with_capacity(len / 2);
    for chunk in bytes.chunks_exact(2) {
        samples.push(i16::from_ne_bytes([chunk[0], chunk[1]]));
    }

    let channels = usize::from(device.channels);
    let mut stereo = Vec::with_capacity((samples.len() / channels) * 2);
    for frame in samples.chunks_exact(channels) {
        stereo.push(frame[0]);
        stereo.push(frame[1]);
    }

    if !process_interleaved_i16(&mut stereo, device.sample_rate as f32) {
        return None;
    }

    for (frame, processed) in samples
        .chunks_exact_mut(channels)
        .zip(stereo.chunks_exact(2))
    {
        frame[0] = processed[0];
        frame[1] = processed[1];
    }

    let mut out = Vec::with_capacity(len);
    for sample in samples {
        out.extend_from_slice(&sample.to_ne_bytes());
    }
    Some(out)
}

fn process_pcm_f32(device: DeviceAudioFormat, data: *const c_void, len: usize) -> Option<Vec<u8>> {
    if device.channels < 2 || len == 0 || !len.is_multiple_of(4) {
        return None;
    }

    let frame_width = usize::from(device.channels) * std::mem::size_of::<f32>();
    if frame_width == 0 || !len.is_multiple_of(frame_width) {
        return None;
    }

    let bytes = unsafe { std::slice::from_raw_parts(data as *const u8, len) };
    let mut samples = Vec::with_capacity(len / 4);
    for chunk in bytes.chunks_exact(4) {
        samples.push(f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }

    let channels = usize::from(device.channels);
    let mut stereo = Vec::with_capacity((samples.len() / channels) * 2);
    for frame in samples.chunks_exact(channels) {
        stereo.push(frame[0]);
        stereo.push(frame[1]);
    }

    if !process_interleaved_f32(&mut stereo, device.sample_rate as f32) {
        return None;
    }

    for (frame, processed) in samples
        .chunks_exact_mut(channels)
        .zip(stereo.chunks_exact(2))
    {
        frame[0] = processed[0];
        frame[1] = processed[1];
    }

    let mut out = Vec::with_capacity(len);
    for sample in samples {
        out.extend_from_slice(&sample.to_ne_bytes());
    }
    Some(out)
}

fn update_hrtf_spatial_for_listener(engine: &mut crate::engine::native_hrtf::NativeHrtfEngine) {
    let listener = crate::engine::sound_hook::get_listener_state();
    let now = crate::engine::cdll_int::CLIENT_TIME
        .lock()
        .map(|t| *t)
        .unwrap_or(0.0);

    let active_node = if let Ok(nodes) = crate::engine::acoustics_debug::ACTIVE_SOUND_NODES.lock() {
        nodes
            .iter()
            .filter(|n: &&super::acoustics_debug::SoundDebugNode| !n.is_expired(now))
            .max_by(|a, b| {
                let dist_a = ((a.origin[0] - listener.origin[0]).powi(2)
                    + (a.origin[1] - listener.origin[1]).powi(2))
                .sqrt()
                .max(30.0);
                let dist_b = ((b.origin[0] - listener.origin[0]).powi(2)
                    + (b.origin[1] - listener.origin[1]).powi(2))
                .sqrt()
                .max(30.0);

                let weight_a = crate::engine::sound_hook::sound_threat_weight(&a.sound_name);
                let weight_b = crate::engine::sound_hook::sound_threat_weight(&b.sound_name);

                let age_a = (now - a.spawn_time).max(0.0);
                let age_b = (now - b.spawn_time).max(0.0);
                let freshness_a = (1.0 - age_a / 2.5).clamp(0.1, 1.0);
                let freshness_b = (1.0 - age_b / 2.5).clamp(0.1, 1.0);

                let score_a = weight_a * (1000.0 / dist_a) * freshness_a;
                let score_b = weight_b * (1000.0 / dist_b) * freshness_b;

                score_a.total_cmp(&score_b)
            })
            .map(|n| n.origin)
    } else {
        None
    };

    if let Some(pos) = active_node {
        let prop =
            unsafe { crate::engine::acoustics::evaluate_sound_propagation(pos, listener.origin) };
        let active_origin = if prop.is_diffracted {
            prop.diffracted_origin
        } else {
            pos
        };
        engine.compute_spatial_params(
            active_origin,
            &listener,
            prop.occlusion,
            prop.reverb_send,
            prop.is_diffracted,
            prop.diffraction_angle_deg,
        );
    } else {
        let held_pos = [
            listener.origin[0] + 120.0 * listener.forward[0],
            listener.origin[1] + 120.0 * listener.forward[1],
            listener.origin[2] + 120.0 * listener.forward[2],
        ];
        engine.compute_spatial_params(held_pos, &listener, 0.0, 1.0, false, 0.0);
    }
}
fn process_interleaved_i16(buffer: &mut [i16], sample_rate: f32) -> bool {
    if is_hrtf_pcm_bypassed() {
        return false;
    }

    let acoustics_enabled = HRTF_ACOUSTICS.enabled();
    let acoustics = get_current_acoustics();
    let mix = if acoustics_enabled {
        acoustics.reflection_mix
    } else {
        0.0
    };

    if let Ok(mut guard) = NATIVE_HRTF_ENGINE.lock() {
        if let Some(engine) = guard.as_mut() {
            engine.set_sample_rate(sample_rate);
            if acoustics_enabled {
                engine.update_acoustics(&acoustics);
            }
            update_hrtf_spatial_for_listener(engine);
            engine.process_buffer_interleaved_i16(buffer, mix);
            return true;
        }
    }

    false
}

fn process_interleaved_f32(buffer: &mut [f32], sample_rate: f32) -> bool {
    if is_hrtf_pcm_bypassed() {
        return false;
    }

    let acoustics_enabled = HRTF_ACOUSTICS.enabled();
    let acoustics = get_current_acoustics();
    let mix = if acoustics_enabled {
        acoustics.reflection_mix
    } else {
        0.0
    };

    if let Ok(mut guard) = NATIVE_HRTF_ENGINE.lock() {
        if let Some(engine) = guard.as_mut() {
            engine.set_sample_rate(sample_rate);
            if acoustics_enabled {
                engine.update_acoustics(&acoustics);
            }
            update_hrtf_spatial_for_listener(engine);
            engine.process_buffer_interleaved_f32(buffer, mix);
            return true;
        }
    }

    false
}

pub fn print_audio_hook_status() {
    let acoustics = get_current_acoustics();
    let device_count = SDL_DEVICE_FORMATS
        .lock()
        .map(|items| items.len())
        .unwrap_or(0);
    let callback_devices = SDL_CALLBACK_HOOKS
        .lock()
        .map(|items| items.len())
        .unwrap_or(0);
    let spatial = NATIVE_HRTF_ENGINE
        .lock()
        .ok()
        .and_then(|guard| {
            guard.as_ref().map(|engine| {
                (
                    engine.sample_rate,
                    engine.last_spatial.azimuth,
                    engine.last_spatial.distance,
                    engine.last_spatial.gain_l,
                    engine.last_spatial.gain_r,
                    engine.last_spatial.occlusion,
                    engine.last_spatial.reverb_send,
                )
            })
        })
        .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0));
    let message = format!(
        "[HRTF PCM] patched_slots={} device_opens={} known_devices={} callback_devices={} queue_hits={} callback_hits={} hrtf_enable={:.2} hrtf_volume={:.2} hrtf_reverb_scale={:.2} bypass={} acoustics={} room={:?} t60={:.2} mix={:.2} engine_sr={:.0} last_azimuth={:.2} last_dist={:.1} gain_l={:.2} gain_r={:.2} occlusion={:.2} reverb_send={:.2}\n",
        AUDIO_HOOK_PATCHES.load(Ordering::Relaxed),
        AUDIO_DEVICE_OPENS.load(Ordering::Relaxed),
        device_count,
        callback_devices,
        AUDIO_QUEUE_HITS.load(Ordering::Relaxed),
        AUDIO_CALLBACK_HITS.load(Ordering::Relaxed),
        HRTF_ENABLE.value_or(1.0),
        HRTF_VOLUME.value_or(1.0),
        HRTF_REVERB_SCALE.value_or(1.0),
        if is_hrtf_pcm_bypassed() { 1 } else { 0 },
        if HRTF_ACOUSTICS.enabled() { 1 } else { 0 },
        acoustics.room_type,
        acoustics.t60_reverb_time,
        acoustics.reflection_mix,
        spatial.0,
        spatial.1,
        spatial.2,
        spatial.3,
        spatial.4,
        spatial.5,
        spatial.6,
    );
    console_print(&message);
}

pub fn refresh_audio_hooks(client_time: f32) {
    #[cfg(all(unix, target_pointer_width = "32"))]
    {
        if AUDIO_QUEUE_HITS.load(Ordering::Relaxed) > 0
            || AUDIO_CALLBACK_HITS.load(Ordering::Relaxed) > 0
        {
            return;
        }

        if let Ok(mut last_scan) = LAST_AUDIO_HOOK_SCAN.lock() {
            if client_time - *last_scan < 5.0 {
                return;
            }
            *last_scan = client_time;
        }

        install_audio_hooks();
    }
}

unsafe fn load_symbol_from_libraries(symbol: &CStr, libs: &[&str]) -> *mut c_void {
    for lib in libs {
        let Ok(c_lib) = CString::new(*lib) else {
            continue;
        };
        let handle = libc::dlopen(c_lib.as_ptr(), libc::RTLD_NOW);
        if handle.is_null() {
            continue;
        }
        let sym = libc::dlsym(handle, symbol.as_ptr());
        if !sym.is_null() {
            return sym;
        }
    }
    std::ptr::null_mut()
}

unsafe fn resolve_sdl_symbol(symbol: &CStr) -> *mut c_void {
    let next = libc::dlsym(libc::RTLD_NEXT, symbol.as_ptr());
    if !next.is_null() {
        return next;
    }

    load_symbol_from_libraries(symbol, &["libSDL2-2.0.so.0", "libSDL2.so", "SDL2.dll"])
}

unsafe fn ensure_original_symbol(slot: &AtomicPtr<c_void>, symbol: &CStr) -> *mut c_void {
    let ptr = slot.load(Ordering::Relaxed);
    if !ptr.is_null() {
        return ptr;
    }

    let resolved = resolve_sdl_symbol(symbol);
    if !resolved.is_null() {
        slot.store(resolved, Ordering::Relaxed);
    }
    resolved
}

#[cfg(unix)]
fn page_protection_for(address: usize) -> Option<c_int> {
    let maps = fs::read_to_string("/proc/self/maps").ok()?;
    for line in maps.lines() {
        let mut parts = line.split_whitespace();
        let range = parts.next()?;
        let perms = parts.next()?;
        let (start_hex, end_hex) = range.split_once('-')?;
        let start = usize::from_str_radix(start_hex, 16).ok()?;
        let end = usize::from_str_radix(end_hex, 16).ok()?;
        if !(start..end).contains(&address) {
            continue;
        }

        let mut prot = 0;
        let bytes = perms.as_bytes();
        if bytes.first() == Some(&b'r') {
            prot |= libc::PROT_READ;
        }
        if bytes.get(1) == Some(&b'w') {
            prot |= libc::PROT_WRITE;
        }
        if bytes.get(2) == Some(&b'x') {
            prot |= libc::PROT_EXEC;
        }
        return Some(prot);
    }

    None
}

#[cfg(all(unix, target_pointer_width = "32"))]
mod unix_elf {
    use super::{
        HookSpec, ORIG_SDL_CLOSE_AUDIO_DEVICE, ORIG_SDL_OPEN_AUDIO, ORIG_SDL_OPEN_AUDIO_DEVICE,
        ORIG_SDL_QUEUE_AUDIO,
    };
    use std::ffi::{c_char, c_void, CStr};
    use std::os::raw::c_int;

    const R_386_GLOB_DAT: u32 = 6;
    const R_386_JMP_SLOT: u32 = 7;
    const R_386_PC32: u32 = 2;

    #[repr(C)]
    struct Elf32Dyn {
        d_tag: i32,
        d_val_ptr: u32,
    }

    #[repr(C)]
    struct Elf32Sym {
        st_name: u32,
        st_value: u32,
        st_size: u32,
        st_info: u8,
        st_other: u8,
        st_shndx: u16,
    }

    #[repr(C)]
    struct Elf32Rel {
        r_offset: u32,
        r_info: u32,
    }

    fn elf32_r_sym(info: u32) -> usize {
        (info >> 8) as usize
    }

    fn elf32_r_type(info: u32) -> u32 {
        info & 0xff
    }

    fn resolve_runtime_ptr(base_addr: usize, value: u32) -> usize {
        let raw = value as usize;
        if base_addr != 0 && raw < base_addr {
            base_addr + raw
        } else {
            raw
        }
    }

    fn should_patch_object(info: &libc::dl_phdr_info) -> bool {
        if info.dlpi_name.is_null() {
            return true;
        }

        let name = unsafe { CStr::from_ptr(info.dlpi_name) };
        let bytes = name.to_bytes();
        if bytes.is_empty() {
            return true;
        }

        let basename = bytes.rsplit(|byte| *byte == b'/').next().unwrap_or(bytes);
        matches!(
            basename,
            b"hl_linux"
                | b"hw.so"
                | b"sw.so"
                | b"engine_i486.so"
                | b"client.so"
                | b"client.so_orig"
        )
    }

    unsafe fn patch_memory(slot: *mut usize, value: usize) -> bool {
        if slot.is_null() || *slot == value {
            return false;
        }

        let page_size = libc::sysconf(libc::_SC_PAGESIZE);
        if page_size <= 0 {
            return false;
        }

        let page_size = page_size as usize;
        let page_start = (slot as usize) & !(page_size - 1);
        let original_prot =
            super::page_protection_for(slot as usize).unwrap_or(libc::PROT_READ | libc::PROT_WRITE);
        if libc::mprotect(
            page_start as *mut c_void,
            page_size,
            original_prot | libc::PROT_WRITE,
        ) != 0
        {
            return false;
        }

        *slot = value;
        let _ = libc::mprotect(page_start as *mut c_void, page_size, original_prot);
        true
    }

    unsafe fn patch_relative_call(slot: *mut i32, value: usize) -> bool {
        if slot.is_null() {
            return false;
        }

        let page_size = libc::sysconf(libc::_SC_PAGESIZE);
        if page_size <= 0 {
            return false;
        }

        let page_size = page_size as usize;
        let page_start = (slot as usize) & !(page_size - 1);
        let original_prot = super::page_protection_for(slot as usize)
            .unwrap_or(libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC);
        if libc::mprotect(
            page_start as *mut c_void,
            page_size,
            original_prot | libc::PROT_WRITE,
        ) != 0
        {
            return false;
        }

        let disp = value as isize - (slot as isize + std::mem::size_of::<i32>() as isize);
        *slot = disp as i32;
        let _ = libc::mprotect(page_start as *mut c_void, page_size, original_prot);
        true
    }

    unsafe fn patch_rel_table(
        base_addr: usize,
        rel_ptr: *const Elf32Rel,
        rel_count: usize,
        symtab: *const Elf32Sym,
        strtab: *const c_char,
        hooks: &[HookSpec],
    ) -> u32 {
        if rel_ptr.is_null() || symtab.is_null() || strtab.is_null() {
            return 0;
        }

        let mut patches = 0u32;

        for idx in 0..rel_count {
            let rel = &*rel_ptr.add(idx);
            let rel_type = elf32_r_type(rel.r_info);
            if !matches!(rel_type, R_386_GLOB_DAT | R_386_JMP_SLOT | R_386_PC32)
                || rel.r_offset == 0
            {
                continue;
            }

            let sym_idx = elf32_r_sym(rel.r_info);
            if sym_idx == 0 {
                continue;
            }

            let sym = &*symtab.add(sym_idx);
            let name_ptr = strtab.add(sym.st_name as usize);
            if name_ptr.is_null() {
                continue;
            }

            let name = CStr::from_ptr(name_ptr).to_bytes();

            for hook in hooks {
                if name == hook.name {
                    if rel_type == R_386_PC32 {
                        let slot = (base_addr + rel.r_offset as usize) as *mut i32;
                        let opcode = *((slot as *const u8).sub(1));
                        if (opcode == 0xE8 || opcode == 0xE9)
                            && patch_relative_call(slot, hook.replacement as usize)
                        {
                            patches += 1;
                        }
                    } else {
                        let slot = (base_addr + rel.r_offset as usize) as *mut usize;
                        if patch_memory(slot, hook.replacement as usize) {
                            patches += 1;
                        }
                    }
                }
            }
        }

        patches
    }

    unsafe fn patch_object(info: &libc::dl_phdr_info, hooks: &[HookSpec]) -> u32 {
        if info.dlpi_phdr.is_null() || info.dlpi_phnum == 0 || !should_patch_object(info) {
            return 0;
        }

        let base_addr = info.dlpi_addr as usize;
        let phdrs = info.dlpi_phdr as *const libc::Elf32_Phdr;
        let mut dyn_ptr = std::ptr::null::<Elf32Dyn>();
        let mut dyn_count = 0usize;

        for idx in 0..info.dlpi_phnum as usize {
            let phdr = &*phdrs.add(idx);
            if phdr.p_type == libc::PT_DYNAMIC {
                dyn_ptr = (base_addr + phdr.p_vaddr as usize) as *const Elf32Dyn;
                dyn_count = phdr.p_memsz as usize / std::mem::size_of::<Elf32Dyn>();
                break;
            }
        }

        if dyn_ptr.is_null() || dyn_count == 0 {
            return 0;
        }

        let mut symtab = std::ptr::null::<Elf32Sym>();
        let mut strtab = std::ptr::null::<c_char>();
        let mut rel_ptr = std::ptr::null::<Elf32Rel>();
        let mut rel_count = 0usize;
        let mut jmprel_ptr = std::ptr::null::<Elf32Rel>();
        let mut jmprel_count = 0usize;

        for idx in 0..dyn_count {
            let dyn_entry = &*dyn_ptr.add(idx);
            match dyn_entry.d_tag as u64 {
                super::DT_NULL => break,
                super::DT_SYMTAB => {
                    symtab = resolve_runtime_ptr(base_addr, dyn_entry.d_val_ptr) as *const Elf32Sym;
                }
                super::DT_STRTAB => {
                    strtab = resolve_runtime_ptr(base_addr, dyn_entry.d_val_ptr) as *const c_char;
                }
                super::DT_REL => {
                    rel_ptr =
                        resolve_runtime_ptr(base_addr, dyn_entry.d_val_ptr) as *const Elf32Rel;
                }
                super::DT_RELSZ => {
                    rel_count = dyn_entry.d_val_ptr as usize / std::mem::size_of::<Elf32Rel>();
                }
                super::DT_JMPREL => {
                    jmprel_ptr =
                        resolve_runtime_ptr(base_addr, dyn_entry.d_val_ptr) as *const Elf32Rel;
                }
                super::DT_PLTRELSZ => {
                    jmprel_count = dyn_entry.d_val_ptr as usize / std::mem::size_of::<Elf32Rel>();
                }
                _ => {}
            }
        }

        patch_rel_table(base_addr, rel_ptr, rel_count, symtab, strtab, hooks)
            + patch_rel_table(base_addr, jmprel_ptr, jmprel_count, symtab, strtab, hooks)
    }

    unsafe extern "C" fn patch_callback(
        info: *mut libc::dl_phdr_info,
        _size: usize,
        data: *mut c_void,
    ) -> c_int {
        if info.is_null() || data.is_null() {
            return 0;
        }

        let hooks = &*(data as *const Vec<HookSpec>);
        let patched = patch_object(&*info, hooks);
        if patched > 0 {
            super::AUDIO_HOOK_PATCHES.fetch_add(patched, std::sync::atomic::Ordering::Relaxed);
        }
        0
    }

    pub unsafe fn install_elf_hooks() {
        let hooks = vec![
            HookSpec {
                name: b"SDL_OpenAudioDevice",
                replacement: super::SDL_OpenAudioDevice as *mut c_void,
            },
            HookSpec {
                name: b"SDL_OpenAudio",
                replacement: super::SDL_OpenAudio as *mut c_void,
            },
            HookSpec {
                name: b"SDL_QueueAudio",
                replacement: super::SDL_QueueAudio as *mut c_void,
            },
            HookSpec {
                name: b"SDL_CloseAudioDevice",
                replacement: super::SDL_CloseAudioDevice as *mut c_void,
            },
        ];

        let _ = ORIG_SDL_OPEN_AUDIO_DEVICE.load(std::sync::atomic::Ordering::Relaxed);
        let _ = ORIG_SDL_OPEN_AUDIO.load(std::sync::atomic::Ordering::Relaxed);
        let _ = ORIG_SDL_QUEUE_AUDIO.load(std::sync::atomic::Ordering::Relaxed);
        let _ = ORIG_SDL_CLOSE_AUDIO_DEVICE.load(std::sync::atomic::Ordering::Relaxed);

        let prev_patches = super::AUDIO_HOOK_PATCHES.load(std::sync::atomic::Ordering::Relaxed);
        libc::dl_iterate_phdr(Some(patch_callback), &hooks as *const _ as *mut c_void);
        let new_patches =
            super::AUDIO_HOOK_PATCHES.load(std::sync::atomic::Ordering::Relaxed) - prev_patches;

        if new_patches > 0 && super::HRTF_DEBUG.value_or(0.0) != 0.0 {
            super::console_print(&format!(
                "[HRTF PCM] hooked {} new audio slots (total {})\n",
                new_patches,
                super::AUDIO_HOOK_PATCHES.load(std::sync::atomic::Ordering::Relaxed)
            ));
        }
    }
}

#[cfg(not(all(unix, target_pointer_width = "32")))]
pub fn install_audio_hooks() {}

#[cfg(all(unix, target_pointer_width = "32"))]
pub fn install_audio_hooks() {
    unsafe {
        unix_elf::install_elf_hooks();
    }
}

#[no_mangle]
pub unsafe extern "C" fn SDL_OpenAudioDevice(
    device: *const c_char,
    iscapture: c_int,
    desired: *const SdlAudioSpec,
    obtained: *mut SdlAudioSpec,
    allowed_changes: c_int,
) -> SdlAudioDeviceId {
    let orig = ensure_original_symbol(&ORIG_SDL_OPEN_AUDIO_DEVICE, c"SDL_OpenAudioDevice");
    if orig.is_null() {
        return 0;
    }

    let func: SdlOpenAudioDevice = std::mem::transmute(orig);
    let mut hooked_spec = desired.as_ref().copied();
    let mut callback_state = std::ptr::null_mut::<HookedAudioCallbackState>();
    let desired_ptr = if iscapture == 0 {
        if let Some(spec) = hooked_spec.as_mut() {
            if let Some(original_callback) = spec.callback {
                let state = Box::new(HookedAudioCallbackState {
                    original_callback,
                    original_userdata: spec.userdata,
                    sample_rate: spec.freq,
                    format: spec.format,
                    channels: spec.channels,
                });
                callback_state = Box::into_raw(state);
                spec.callback = Some(hrtf_sdl_audio_callback);
                spec.userdata = callback_state as *mut c_void;
                spec as *const SdlAudioSpec
            } else {
                desired
            }
        } else {
            desired
        }
    } else {
        desired
    };

    let dev = func(device, iscapture, desired_ptr, obtained, allowed_changes);
    if dev != 0 && iscapture == 0 {
        if !obtained.is_null() {
            remember_device_format(dev, &*obtained);
        } else if !desired.is_null() {
            remember_device_format(dev, &*desired);
        }

        if !callback_state.is_null() {
            let spec = if !obtained.is_null() {
                &*obtained
            } else {
                &*desired_ptr
            };
            (*callback_state).sample_rate = spec.freq;
            (*callback_state).format = spec.format;
            (*callback_state).channels = spec.channels;
            register_callback_hook(dev, callback_state);
            debug_log(&format!(
                "[HRTF PCM] wrapped SDL_OpenAudioDevice callback dev={} rate={} fmt=0x{:x} channels={}\n",
                dev, spec.freq, spec.format, spec.channels
            ));
        }
    } else if !callback_state.is_null() {
        drop(Box::from_raw(callback_state));
    }
    dev
}

#[no_mangle]
pub unsafe extern "C" fn SDL_OpenAudio(
    desired: *const SdlAudioSpec,
    obtained: *mut SdlAudioSpec,
) -> c_int {
    let orig = ensure_original_symbol(&ORIG_SDL_OPEN_AUDIO, c"SDL_OpenAudio");
    if orig.is_null() {
        return -1;
    }

    let func: SdlOpenAudio = std::mem::transmute(orig);
    let mut hooked_spec = desired.as_ref().copied();
    let mut callback_state = std::ptr::null_mut::<HookedAudioCallbackState>();
    let desired_ptr = if let Some(spec) = hooked_spec.as_mut() {
        if let Some(original_callback) = spec.callback {
            let state = Box::new(HookedAudioCallbackState {
                original_callback,
                original_userdata: spec.userdata,
                sample_rate: spec.freq,
                format: spec.format,
                channels: spec.channels,
            });
            callback_state = Box::into_raw(state);
            spec.callback = Some(hrtf_sdl_audio_callback);
            spec.userdata = callback_state as *mut c_void;
            spec as *const SdlAudioSpec
        } else {
            desired
        }
    } else {
        desired
    };

    let result = func(desired_ptr, obtained);
    if result == 0 {
        if !obtained.is_null() {
            remember_device_format(1, &*obtained);
        } else if !desired.is_null() {
            remember_device_format(1, &*desired);
        }

        if !callback_state.is_null() {
            let spec = if !obtained.is_null() {
                &*obtained
            } else {
                &*desired_ptr
            };
            (*callback_state).sample_rate = spec.freq;
            (*callback_state).format = spec.format;
            (*callback_state).channels = spec.channels;
            register_callback_hook(1, callback_state);
            debug_log(&format!(
                "[HRTF PCM] wrapped SDL_OpenAudio callback rate={} fmt=0x{:x} channels={}\n",
                spec.freq, spec.format, spec.channels
            ));
        }
    } else if !callback_state.is_null() {
        drop(Box::from_raw(callback_state));
    }
    result
}

#[no_mangle]
pub unsafe extern "C" fn SDL_QueueAudio(
    dev: SdlAudioDeviceId,
    data: *const c_void,
    len: u32,
) -> c_int {
    let orig = ensure_original_symbol(&ORIG_SDL_QUEUE_AUDIO, c"SDL_QueueAudio");
    if orig.is_null() {
        return -1;
    }

    let func: SdlQueueAudio = std::mem::transmute(orig);
    if data.is_null() || len == 0 || is_hrtf_pcm_bypassed() {
        return func(dev, data, len);
    }

    let hits = AUDIO_QUEUE_HITS.fetch_add(1, Ordering::Relaxed) + 1;
    if hits == 1 {
        debug_log(&format!(
            "[HRTF PCM] first SDL_QueueAudio intercept dev={} bytes={}\n",
            dev, len
        ));
    }

    let processed = match get_device_format(dev) {
        Some(format) if format.format == AUDIO_S16SYS => {
            process_pcm_i16(format, data, len as usize)
        }
        Some(format) if format.format == AUDIO_F32SYS => {
            process_pcm_f32(format, data, len as usize)
        }
        _ => None,
    };

    if let Some(bytes) = processed {
        return func(dev, bytes.as_ptr() as *const c_void, bytes.len() as u32);
    }

    func(dev, data, len)
}

#[no_mangle]
pub unsafe extern "C" fn SDL_CloseAudioDevice(dev: SdlAudioDeviceId) {
    forget_device_format(dev);
    unregister_callback_hook(dev);

    let orig = ensure_original_symbol(&ORIG_SDL_CLOSE_AUDIO_DEVICE, c"SDL_CloseAudioDevice");
    if orig.is_null() {
        return;
    }

    let func: SdlCloseAudioDevice = std::mem::transmute(orig);
    func(dev);
}
