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
    OG_DAMAGE,
);

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

pub unsafe extern "C" fn hk_msg_func_damage(
    name: *const c_char,
    size: c_int,
    pbuf: *mut c_void,
) -> c_int {
    if !pbuf.is_null() && size >= 2 {
        let slice = std::slice::from_raw_parts(pbuf as *const u8, size as usize);
        let health_take = slice.get(1).copied().unwrap_or(0);
        if health_take > 0 {
            let listener = crate::engine::sound_hook::get_listener_state();
            let pain_sound = pick_pain_sound(health_take);
            let volume = if crate::utils::cvars::HRTF_ENABLE.enabled() {
                crate::utils::cvars::HRTF_VOLUME.value_or(1.0)
            } else {
                1.0
            };
            crate::engine::sound_hook::trigger_pain_sound(pain_sound, volume, listener.origin);
            if crate::utils::cvars::HRTF_DEBUG.value() != 0.0 {
                crate::con_println!(
                    "[HRTF Damage] Player took {} dmg -> Pain sound '{}'",
                    health_take,
                    pain_sound
                );
            }
        }
    }
    call_og!(OG_DAMAGE, name, size, pbuf)
}

static PAIN_SOUND_ROTATION: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
const PAIN_SOUNDS: [&str; 6] = [
    "player/pl_pain1.wav",
    "player/pl_pain2.wav",
    "player/pl_pain3.wav",
    "player/pl_pain4.wav",
    "player/pl_pain5.wav",
    "player/pl_pain6.wav",
];

fn pick_pain_sound(health_take: u8) -> &'static str {
    let rotation = PAIN_SOUND_ROTATION.fetch_add(1, Ordering::Relaxed);
    let idx = (health_take as u32).wrapping_add(rotation) as usize % PAIN_SOUNDS.len();
    PAIN_SOUNDS[idx]
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
        "HideWeapon" => store_og_and_hook!(OG_HIDE_WEAPON, pfn, hk_msg_func_hide_weapon),
        "NVGToggle" => store_og_and_hook!(OG_NVG_TOGGLE, pfn, hk_msg_func_nvg_toggle),
        "Damage" => store_og_and_hook!(OG_DAMAGE, pfn, hk_msg_func_damage),
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
