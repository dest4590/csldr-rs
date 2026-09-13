use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Mutex;

use crate::engine::proxy::CL_FUNCS;
use crate::engine::sdk::{ClEngineFuncs, ClientData};
use crate::engine::user_msg::hk_hook_user_msg;
use crate::hud::fov::{fov_think, get_current_fov};
use crate::render::studio_cache::studio_config_flush;
use crate::utils::cvars::{init_cvars, set_engfuncs};

pub static mut G_ENGFUNCS: ClEngineFuncs = unsafe { std::mem::zeroed() };
pub static IS_CZERO: AtomicBool = AtomicBool::new(false);
pub static CLIENT_TIME: Mutex<f32> = Mutex::new(0.0);
pub static SCREEN_WIDTH: AtomicI32 = AtomicI32::new(640);
pub static SCREEN_HEIGHT: AtomicI32 = AtomicI32::new(480);

pub unsafe extern "C" fn studio_config_flush_f() {
    studio_config_flush();
}

pub unsafe extern "C" fn hrtf_status_f() {
    crate::engine::audio_hook::print_audio_hook_status();
}

pub unsafe extern "C" fn hrtf_rays_f() {
    crate::engine::acoustics::print_ray_state();
}

pub unsafe extern "C" fn freecam_toggle_f() {
    let new_state = crate::render::camera::toggle_freecam();
    let cvar_val = if new_state { 1.0 } else { 0.0 };

    let cvar_set_ptr = crate::engine::cdll_int::G_ENGFUNCS.Cvar_SetValue;
    if !cvar_set_ptr.is_null() {
        let cvar_set: unsafe extern "C" fn(*const c_char, f32) = std::mem::transmute(cvar_set_ptr);
        cvar_set(c"hrtf_freecam".as_ptr(), cvar_val);
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
        crate::engine::sound_hook::init_sound_hooks(penginefuncs);
        crate::engine::cmd_hook::init_cmd_hooks(penginefuncs);
    }

    if let Some(init_fn) = CL_FUNCS.pInitFunc {
        init_fn(penginefuncs, iversion)
    } else {
        1
    }
}

pub unsafe fn hk_hud_init() {
    crate::engine::audio_hook::install_audio_hooks();

    if let Some(hud_init_fn) = CL_FUNCS.pHudInitFunc {
        hud_init_fn();
    }

    set_engfuncs(&raw const G_ENGFUNCS);
    init_cvars();

    crate::engine::native_hrtf::init_native_hrtf_engine();

    register_command("studio_config_flush", studio_config_flush_f);
    register_command("hrtf_status", hrtf_status_f);
    register_command("hrtf_rays", hrtf_rays_f);
    register_command("freecam", freecam_toggle_f);

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
    crate::engine::sound_hook::check_and_hook_event_api();

    if let Some(get_time) = G_ENGFUNCS.GetClientTime {
        let ct = get_time();
        if let Ok(mut g_time) = CLIENT_TIME.lock() {
            *g_time = ct;
        }
        crate::engine::audio_hook::refresh_audio_hooks(ct);
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
