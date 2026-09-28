use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::atomic::{AtomicPtr, Ordering};

use crate::engine::cdll_int::{CLIENT_TIME, G_ENGFUNCS, IS_CZERO};
use crate::engine::sdk::ClEntity;
use crate::game::shell::hk_msg_func_brass as process_brass;
use crate::game::state::hk_msg_func_team_info as process_team_info;
use crate::hud::crosshair::{
    hk_msg_func_cur_weapon as process_cur_weapon, hk_msg_func_hide_weapon as process_hide_weapon,
    hk_msg_func_nvg_toggle as process_nvg_toggle,
};
use crate::hud::fov::hk_msg_func_set_fov as process_set_fov;
use crate::utils::cvars::FOV_LERP;

type PfnUserMsgHook = unsafe extern "C" fn(*const c_char, c_int, *mut c_void) -> c_int;

macro_rules! make_og {
    ($($name:ident),* $(,)?) => {
        $(
            static $name: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
        )*
    };
}

macro_rules! call_og {
    ($og:ident, $name:expr, $size:expr, $pbuf:expr) => {{
        let orig = $og.load(Ordering::Relaxed);
        if !orig.is_null() {
            let f: PfnUserMsgHook = std::mem::transmute(orig);
            f($name, $size, $pbuf)
        } else {
            0
        }
    }};
}

macro_rules! store_og_and_hook {
    ($og:ident, $pfn:expr, $hook:ident) => {{
        if $og.load(Ordering::Relaxed).is_null() {
            $og.store($pfn as *mut c_void, Ordering::Relaxed);
        }
        $hook
    }};
}

macro_rules! user_msg_hook {
    ($name:ident, $og:ident, $process:expr) => {
        pub unsafe extern "C" fn $name(
            msg_name: *const c_char,
            size: c_int,
            pbuf: *mut c_void,
        ) -> c_int {
            if !pbuf.is_null() && size > 0 {
                let slice = std::slice::from_raw_parts(pbuf as *const u8, size as usize);
                $process(slice);
            }
            call_og!($og, msg_name, size, pbuf)
        }
    };
}

make_og!(
    OG_SET_FOV,
    OG_CUR_WEAPON,
    OG_TEAM_INFO,
    OG_BRASS,
    OG_HIDE_WEAPON,
    OG_NVG_TOGGLE,
    OG_SCREEN_FADE,
);

pub unsafe extern "C" fn hk_msg_func_screen_fade(
    name: *const c_char,
    size: c_int,
    pbuf: *mut c_void,
) -> c_int {
    if !pbuf.is_null() && size >= 8 {
        let slice = std::slice::from_raw_parts(pbuf as *const u8, size as usize);
        let mut reader = crate::utils::msg::MsgRead::new(slice);
        let duration = reader.read_short().unwrap_or(0) as f32 * (1.0 / 4096.0);
        let hold_time = reader.read_short().unwrap_or(0) as f32 * (1.0 / 4096.0);
        let _flags = reader.read_short().unwrap_or(0);
        let r = reader.read_byte().unwrap_or(0);
        let g = reader.read_byte().unwrap_or(0);
        let b = reader.read_byte().unwrap_or(0);
        let a = reader.read_byte().unwrap_or(0);

        // Flashbang screenfade is white (255, 255, 255) with high alpha
        if r > 200 && g > 200 && b > 200 && a > 150 {
            let total_dur = (duration + hold_time + 3.0).clamp(3.0, 9.5);
            let intensity = (a as f32 / 255.0).clamp(0.5, 1.0);
            crate::audio::hrtf::trigger_grenade_concussion(intensity, total_dur);
        }
    }
    call_og!(OG_SCREEN_FADE, name, size, pbuf)
}

user_msg_hook!(hk_msg_func_cur_weapon, OG_CUR_WEAPON, process_cur_weapon);
user_msg_hook!(hk_msg_func_hide_weapon, OG_HIDE_WEAPON, process_hide_weapon);
user_msg_hook!(hk_msg_func_nvg_toggle, OG_NVG_TOGGLE, process_nvg_toggle);

pub unsafe extern "C" fn hk_msg_func_set_fov(
    name: *const c_char,
    size: c_int,
    pbuf: *mut c_void,
) -> c_int {
    if !pbuf.is_null() && size > 0 {
        let slice = std::slice::from_raw_parts(pbuf as *const u8, size as usize);
        let client_time = CLIENT_TIME.lock().map(|t| *t).unwrap_or(0.0);
        let lerp = FOV_LERP.value();
        process_set_fov(slice, client_time, lerp);
    }
    call_og!(OG_SET_FOV, name, size, pbuf)
}

pub unsafe extern "C" fn hk_msg_func_team_info(
    name: *const c_char,
    size: c_int,
    pbuf: *mut c_void,
) -> c_int {
    if !pbuf.is_null() && size > 0 {
        let slice = std::slice::from_raw_parts(pbuf as *const u8, size as usize);
        let local_player_index = if let Some(get_local) = G_ENGFUNCS.GetLocalPlayer {
            let local_ptr = get_local() as *const ClEntity;
            if !local_ptr.is_null() {
                (*local_ptr).index
            } else {
                1
            }
        } else {
            1
        };
        process_team_info(slice, local_player_index);
    }
    call_og!(OG_TEAM_INFO, name, size, pbuf)
}

pub unsafe extern "C" fn hk_msg_func_brass(
    name: *const c_char,
    size: c_int,
    pbuf: *mut c_void,
) -> c_int {
    if !pbuf.is_null() && size > 0 {
        let slice = std::slice::from_raw_parts(pbuf as *const u8, size as usize);
        let is_czero = IS_CZERO.load(Ordering::Relaxed);
        process_brass(slice, is_czero, false);
    }
    call_og!(OG_BRASS, name, size, pbuf)
}

pub unsafe extern "C" fn hk_hook_user_msg(msg_name: *const c_char, pfn: PfnUserMsgHook) -> c_int {
    if msg_name.is_null() {
        return 0;
    }
    let name_str = CStr::from_ptr(msg_name).to_string_lossy();
    let hook_fn: PfnUserMsgHook = match name_str.as_ref() {
        "SetFOV" => store_og_and_hook!(OG_SET_FOV, pfn, hk_msg_func_set_fov),
        "CurWeapon" => store_og_and_hook!(OG_CUR_WEAPON, pfn, hk_msg_func_cur_weapon),
        "TeamInfo" => store_og_and_hook!(OG_TEAM_INFO, pfn, hk_msg_func_team_info),
        "Brass" => store_og_and_hook!(OG_BRASS, pfn, hk_msg_func_brass),
        "ScreenFade" => store_og_and_hook!(OG_SCREEN_FADE, pfn, hk_msg_func_screen_fade),
        "HideWeapon" => store_og_and_hook!(OG_HIDE_WEAPON, pfn, hk_msg_func_hide_weapon),
        "NVGToggle" => store_og_and_hook!(OG_NVG_TOGGLE, pfn, hk_msg_func_nvg_toggle),
        _ => pfn,
    };

    if !G_ENGFUNCS.pfnHookUserMsg.is_null() {
        let f: unsafe extern "C" fn(*const c_char, PfnUserMsgHook) -> c_int =
            std::mem::transmute(G_ENGFUNCS.pfnHookUserMsg);
        f(msg_name, hook_fn)
    } else {
        0
    }
}
