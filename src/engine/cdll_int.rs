use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Mutex;

use crate::engine::proxy::CL_FUNCS;
use crate::engine::sdk::{ClEngineFuncs, ClientData};
use crate::hooks::user_msg::hk_hook_user_msg;
use crate::hud::fov::{fov_think, get_current_fov};
use crate::render::studio_cache::studio_config_flush;
use crate::utils::cvars::{init_cvars, set_engfuncs};

pub static mut G_ENGFUNCS: ClEngineFuncs = unsafe { std::mem::zeroed() };
pub static IS_CZERO: AtomicBool = AtomicBool::new(false);
pub static CLIENT_TIME: Mutex<f32> = Mutex::new(0.0);
pub static SCREEN_WIDTH: AtomicI32 = AtomicI32::new(640);
pub static SCREEN_HEIGHT: AtomicI32 = AtomicI32::new(480);

#[inline]
pub fn get_client_time() -> f32 {
    CLIENT_TIME.lock().map_or(0.0, |t| *t)
}

#[inline]
pub fn cmd_argc() -> i32 {
    unsafe {
        if !G_ENGFUNCS.Cmd_Argc.is_null() {
            let f: unsafe extern "C" fn() -> c_int = std::mem::transmute(G_ENGFUNCS.Cmd_Argc);
            f()
        } else {
            0
        }
    }
}

#[inline]
pub fn cmd_argv(index: i32) -> String {
    unsafe {
        if !G_ENGFUNCS.Cmd_Argv.is_null() {
            let f: unsafe extern "C" fn(c_int) -> *const c_char =
                std::mem::transmute(G_ENGFUNCS.Cmd_Argv);
            let ptr = f(index);
            if !ptr.is_null() {
                return CStr::from_ptr(ptr).to_string_lossy().into_owned();
            }
        }
    }
    String::new()
}

pub unsafe extern "C" fn studio_config_flush_f() {
    studio_config_flush();
}

#[derive(Clone, Debug, Default)]
pub struct LoopingSoundSpawner {
    pub sound_name: String,
    pub origin: [f32; 3],
    pub interval: f32,
    pub next_play: f32,
    pub active: bool,
}

pub static G_SOUND_SPAWNER: Mutex<LoopingSoundSpawner> = Mutex::new(LoopingSoundSpawner {
    sound_name: String::new(),
    origin: [0.0, 0.0, 0.0],
    interval: 0.0,
    next_play: 0.0,
    active: false,
});

pub fn update_sound_spawner() {
    let now = get_client_time();
    let real_now = if now > 0.0 {
        now
    } else {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f32())
            .unwrap_or(0.0)
    };

    let mut to_play = None;
    if let Ok(mut spawner) = G_SOUND_SPAWNER.lock() {
        if spawner.active && !spawner.sound_name.is_empty() && real_now >= spawner.next_play {
            spawner.next_play = real_now + spawner.interval.max(0.1);
            to_play = Some((spawner.sound_name.clone(), spawner.origin));
        }
    }

    if let Some((sound, pos)) = to_play {
        let final_vol = crate::hooks::sound::process_hrtf_sound(&sound, 1.0, Some(pos));
        if let Ok(c_path) = CString::new(sound) {
            unsafe {
                if !crate::engine::cdll_int::G_ENGFUNCS
                    .pfnPlaySoundByNameAtLocation
                    .is_null()
                {
                    let play_fn: unsafe extern "C" fn(*const c_char, f32, *const f32) =
                        std::mem::transmute(
                            crate::engine::cdll_int::G_ENGFUNCS.pfnPlaySoundByNameAtLocation,
                        );
                    play_fn(c_path.as_ptr(), final_vol.max(0.4), pos.as_ptr());
                } else if !crate::engine::cdll_int::G_ENGFUNCS
                    .pfnPlaySoundByName
                    .is_null()
                {
                    let play_fn: unsafe extern "C" fn(*const c_char, f32) =
                        std::mem::transmute(crate::engine::cdll_int::G_ENGFUNCS.pfnPlaySoundByName);
                    play_fn(c_path.as_ptr(), final_vol.max(0.4));
                }
            }
        }
    }
}

pub unsafe extern "C" fn spawn_sound_f() {
    let argc = cmd_argc();
    if argc < 2 {
        crate::con_println!("Usage: spawn_sound <filename> [delay_seconds] (e.g. spawn_sound weapons/he_bounce-1.wav 1.5)");
        crate::con_println!("       spawn_sound stop");
        return;
    }

    let arg1 = cmd_argv(1);
    if arg1 == "stop" || arg1 == "0" {
        if let Ok(mut sp) = G_SOUND_SPAWNER.lock() {
            sp.active = false;
        }
        crate::con_println!("[CSLDR] Sound spawner stopped.");
        return;
    }

    let delay: f32 = if argc >= 3 {
        cmd_argv(2).parse::<f32>().unwrap_or(1.0).max(0.1)
    } else {
        1.0
    };

    let listener = crate::hooks::sound::get_listener_state();

    if let Ok(mut sp) = G_SOUND_SPAWNER.lock() {
        sp.sound_name = arg1.clone();
        sp.origin = listener.origin;
        sp.interval = delay;
        sp.next_play = 0.0;
        sp.active = true;
    }

    crate::con_println!(
        "[CSLDR] Spawning looping 3D sound '{}' at ({:.1}, {:.1}, {:.1}) every {:.2}s ('spawn_sound stop' to disable)",
        arg1,
        listener.origin[0],
        listener.origin[1],
        listener.origin[2],
        delay
    );
}

pub unsafe extern "C" fn hrtf_status_f() {
    crate::hooks::alsa_sdl::print_audio_hook_status();
}

pub unsafe extern "C" fn hrtf_rays_f() {
    crate::audio::acoustics::print_ray_state();
}

pub unsafe extern "C" fn freecam_toggle_f() {
    let new_state = crate::render::camera::toggle_freecam();
    let cvar_val = if new_state { 1.0 } else { 0.0 };

    let cvar_set_ptr = crate::engine::cdll_int::G_ENGFUNCS.Cvar_SetValue;
    if !cvar_set_ptr.is_null() {
        let cvar_set: unsafe extern "C" fn(*const c_char, f32) = std::mem::transmute(cvar_set_ptr);
        if let Ok(cvar_name) = std::ffi::CString::new("freecam") {
            cvar_set(cvar_name.as_ptr(), cvar_val);
        }
    }

    let status = if new_state {
        "ENABLED [Fly with WASD / Space / Ctrl]"
    } else {
        "DISABLED"
    };
    crate::con_println!("[CSLDR] Freecam {status}");
}

pub unsafe fn register_command(name: &str, func: unsafe extern "C" fn()) {
    if let Some(add_cmd_fn) = G_ENGFUNCS.pfnAddCommand {
        let c_name = Box::leak(CString::new(name).unwrap().into_boxed_c_str());
        add_cmd_fn(c_name.as_ptr(), func);
    }
}

pub unsafe extern "C" fn hk_initialize(penginefuncs: *mut ClEngineFuncs, iversion: c_int) -> c_int {
    crate::engine::proxy::proxy_init();

    if !penginefuncs.is_null() {
        G_ENGFUNCS = std::ptr::read(penginefuncs);
        (*penginefuncs).pfnHookUserMsg = hk_hook_user_msg as *mut c_void;
        crate::hooks::sound::init_sound_hooks(penginefuncs);
        crate::hooks::commands::init_cmd_hooks(penginefuncs);
    }

    if let Some(init_fn) = CL_FUNCS.pInitFunc {
        init_fn(penginefuncs, iversion)
    } else {
        1
    }
}

pub unsafe fn hk_hud_init() {
    crate::hooks::alsa_sdl::install_audio_hooks();

    if let Some(hud_init_fn) = CL_FUNCS.pHudInitFunc {
        hud_init_fn();
    }

    set_engfuncs(&raw const G_ENGFUNCS);
    init_cvars();

    crate::audio::hrtf::init_native_hrtf_engine();

    register_command("studio_config_flush", studio_config_flush_f);
    register_command("hrtf_status", hrtf_status_f);
    register_command("hrtf_rays", hrtf_rays_f);
    register_command("freecam", freecam_toggle_f);
    register_command("spawn_sound", spawn_sound_f);

    if let Some(get_gamedir) = G_ENGFUNCS.pfnGetGameDirectory {
        let gamedir_ptr = get_gamedir();
        if !gamedir_ptr.is_null() {
            let gamedir = std::ffi::CStr::from_ptr(gamedir_ptr).to_string_lossy();
            if gamedir == "czero" {
                IS_CZERO.store(true, Ordering::Relaxed);
            }
        }
    }
}

#[repr(C)]
pub struct ScreenInfo {
    pub size: c_int,
    pub width: c_int,
    pub height: c_int,
    pub flags: c_int,
    pub char_height: c_int,
    pub char_widths: [i16; 256],
}

pub unsafe fn update_screen_size() {
    if !G_ENGFUNCS.pfnGetScreenInfo.is_null() {
        let mut scr: ScreenInfo = std::mem::zeroed();
        scr.size = std::mem::size_of::<ScreenInfo>() as c_int;
        let get_scr_fn: unsafe extern "C" fn(*mut ScreenInfo) =
            std::mem::transmute(G_ENGFUNCS.pfnGetScreenInfo);
        get_scr_fn(&mut scr);
        if scr.width > 0 && scr.height > 0 {
            SCREEN_WIDTH.store(scr.width, Ordering::Relaxed);
            SCREEN_HEIGHT.store(scr.height, Ordering::Relaxed);
        }
    }
}

pub unsafe fn hk_hud_frame(time: f64) {
    update_screen_size();
    crate::hooks::sound::check_and_hook_event_api();

    if let Some(get_time) = G_ENGFUNCS.GetClientTime {
        let ct = get_time();
        if let Ok(mut g_time) = CLIENT_TIME.lock() {
            *g_time = ct;
        }
        crate::hooks::alsa_sdl::refresh_audio_hooks(ct);
        crate::audio::dsp::prune_stale_trackers(ct as f32);
        fov_think(ct);
    }

    if let Some(hud_frame_fn) = CL_FUNCS.pHudFrame {
        hud_frame_fn(time);
    }
}

pub unsafe fn hk_update_client_data(pcldata: *mut c_void, fl_time: f32) -> c_int {
    let res = if let Some(update_fn) = CL_FUNCS.pHudUpdateClientDataFunc {
        update_fn(pcldata, fl_time)
    } else {
        1
    };

    if !pcldata.is_null() {
        let cdata = pcldata as *mut ClientData;
        let w = SCREEN_WIDTH.load(Ordering::Relaxed);
        let h = SCREEN_HEIGHT.load(Ordering::Relaxed);
        (*cdata).fov = get_current_fov(w, h);
    }

    res
}
