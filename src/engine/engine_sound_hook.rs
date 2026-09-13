use crate::engine::sound_hook::{
    is_local_entity_or_near, process_hrtf_sound, resolve_sound_world_origin, scaled_sound_volume,
};
use std::ffi::{c_char, c_int, c_void, CStr};
use std::sync::atomic::AtomicPtr;

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
    let sound_name = if sfx.is_null() {
        std::borrow::Cow::Borrowed("")
    } else {
        CStr::from_ptr(sfx as *const c_char).to_string_lossy()
    };

    let orig_raw = (!origin.is_null()).then(|| [*origin, *origin.add(1), *origin.add(2)]);

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

    let is_local_player = is_local_entity_or_near(entnum, orig_raw);
    let sound_pos = if is_local_player {
        None
    } else {
        resolve_sound_world_origin(origin, entnum)
    };

    let effective_vol = scaled_sound_volume(fvol);
    let final_vol = if sound_name.is_empty() {
        effective_vol
    } else {
        process_hrtf_sound(&sound_name, effective_vol, sound_pos)
    };

    if final_vol <= 0.005 {
        return;
    }

    call_engine_trampoline!(
        trampoline,
        FnStartSound,
        entnum,
        entchannel,
        sfx,
        origin,
        final_vol.max(0.01),
        attenuation,
        flags,
        pitch
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

pub unsafe fn install_engine_sound_hooks() {}
