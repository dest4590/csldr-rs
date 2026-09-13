use crate::engine::sdk::ClEngineFuncs;
use crate::utils::cvars::{HRTF_ACOUSTICS, HRTF_ENABLE, HRTF_VOLUME};
use std::ffi::{c_char, c_void, CStr};
use std::os::raw::c_int;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Mutex;

macro_rules! ffi_ptr { ($(#[$meta:meta])* $($vis:ident)? static $name:ident) => {
    $(#[$meta])* $($vis)? static $name: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
}; }

pub type PfnPlaySoundByName = unsafe extern "C" fn(sz_sound: *const c_char, volume: f32);
pub type PfnPlaySoundByNameAtLocation =
    unsafe extern "C" fn(sz_sound: *const c_char, volume: f32, origin: *const f32);
pub type PfnPlaySoundByIndex = unsafe extern "C" fn(sound_index: c_int, volume: f32);
pub type PfnPlaySoundByNameAtPitch =
    unsafe extern "C" fn(sz_sound: *const c_char, volume: f32, pitch: c_int);
pub type PfnPlaySoundVoiceByName =
    unsafe extern "C" fn(sz_sound: *const c_char, volume: f32, pitch: c_int);

pub type PfnEVPlaySound = unsafe extern "C" fn(
    ent: c_int,
    origin: *const f32,
    channel: c_int,
    sample: *const c_char,
    volume: f32,
    attenuation: f32,
    f_flags: c_int,
    pitch: c_int,
);

pub type PfnPlaybackEvent = unsafe extern "C" fn(
    flags: c_int,
    p_invoker: *const c_void,
    eventindex: u16,
    delay: f32,
    origin: *const f32,
    angles: *const f32,
    fparam1: f32,
    fparam2: f32,
    iparam1: c_int,
    iparam2: c_int,
    bparam1: c_int,
    bparam2: c_int,
);

pub type FnPMPlaySound = unsafe extern "C" fn(
    channel: c_int,
    sample: *const c_char,
    volume: f32,
    attenuation: f32,
    f_flags: c_int,
    pitch: c_int,
);

pub type FnPMPlaybackEventFull = unsafe extern "C" fn(
    flags: c_int,
    clientindex: c_int,
    eventindex: u16,
    delay: f32,
    origin: *const f32,
    angles: *const f32,
    fparam1: f32,
    fparam2: f32,
    iparam1: c_int,
    iparam2: c_int,
    bparam1: c_int,
    bparam2: c_int,
);

pub type FnTempEntSoundCallback = unsafe extern "C" fn(ptemp: *mut TempEntityT, damp: f32);

pub const BOUNCE_GLASS: c_int = 0x01;
pub const BOUNCE_METAL: c_int = 0x02;
pub const BOUNCE_FLESH: c_int = 0x04;
pub const BOUNCE_WOOD: c_int = 0x08;
pub const BOUNCE_SHRAP: c_int = 0x10;
pub const BOUNCE_SHELL: c_int = 0x20;
pub const BOUNCE_CONCRETE: c_int = 0x40;
pub const BOUNCE_SHOTSHELL: c_int = 0x80;

#[repr(C)]
pub struct TempEntityT {
    pub flags: c_int,
    pub die: f32,
    pub frame_max: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub fade_speed: f32,
    pub bounce_factor: f32,
    pub hit_sound: c_int,
    pub hitcallback: *mut c_void,
    pub callback: *mut c_void,
    pub next: *mut TempEntityT,
    pub priority: c_int,
    pub client_index: i16,
    pub tent_offset: [f32; 3],
    pub entity: crate::engine::sdk::ClEntity,
}

#[repr(C)]
pub struct EventArgsT {
    pub flags: c_int,
    pub entindex: c_int,
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub velocity: [f32; 3],
    pub ducking: c_int,
    pub fparam1: f32,
    pub fparam2: f32,
    pub iparam1: c_int,
    pub iparam2: c_int,
    pub bparam1: c_int,
    pub bparam2: c_int,
}

pub type FnEventCallback = unsafe extern "C" fn(args: *mut EventArgsT);
pub type PfnHookEvent = unsafe extern "C" fn(name: *const c_char, pfn_event: FnEventCallback);
pub type PfnPrecacheEvent = unsafe extern "C" fn(type_: c_int, psz: *const c_char) -> u16;

pub type PfnEVPlaybackEvent = unsafe extern "C" fn(
    flags: c_int,
    p_invoker: *const c_void,
    eventindex: u16,
    delay: f32,
    origin: *const f32,
    angles: *const f32,
    fparam1: f32,
    fparam2: f32,
    iparam1: c_int,
    iparam2: c_int,
    bparam1: c_int,
    bparam2: c_int,
);

ffi_ptr!(pub static ORIG_PLAY_SOUND_BY_NAME);
ffi_ptr!(pub static ORIG_PLAY_SOUND_BY_NAME_AT_LOCATION);
ffi_ptr!(pub static ORIG_PLAY_SOUND_BY_INDEX);
ffi_ptr!(pub static ORIG_PLAY_SOUND_BY_NAME_AT_PITCH);
ffi_ptr!(pub static ORIG_PLAY_SOUND_VOICE_BY_NAME);
ffi_ptr!(pub static ORIG_EV_PLAY_SOUND);
ffi_ptr!(pub static ORIG_EV_PLAYBACK_EVENT);
ffi_ptr!(pub static ORIG_PLAYBACK_EVENT);
ffi_ptr!(pub static ORIG_PM_PLAY_SOUND);
ffi_ptr!(pub static ORIG_PM_PLAYBACK_EVENT_FULL);
ffi_ptr!(pub static CURRENT_CB_SOUND);
ffi_ptr!(pub static ORIG_HOOK_EVENT);
ffi_ptr!(pub static ORIG_PRECACHE_EVENT);
ffi_ptr!(pub static ORIG_EV_PRECACHE_EVENT);

pub static LAST_PMMOVE_ORIGIN: Mutex<[f32; 3]> = Mutex::new([0.0, 0.0, 0.0]);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PhysentT {
    pub name: [c_char; 32],
    pub player: c_int,
    pub origin: [f32; 3],
    pub model: *mut c_void,
    pub studiomodel: *mut c_void,
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
    pub info: c_int,
    pub angles: [f32; 3],
    pub solid: c_int,
    pub skin: c_int,
    pub rendermode: c_int,
    pub frame: f32,
    pub sequence: c_int,
    pub controller: [u8; 4],
    pub blending: [u8; 2],
    pub movetype: c_int,
    pub takedamage: c_int,
    pub blooddecal: c_int,
    pub team: c_int,
    pub classnumber: c_int,
    pub iuser1: c_int,
    pub iuser2: c_int,
    pub iuser3: c_int,
    pub iuser4: c_int,
    pub fuser1: f32,
    pub fuser2: f32,
    pub fuser3: f32,
    pub fuser4: f32,
    pub vuser1: [f32; 3],
    pub vuser2: [f32; 3],
    pub vuser3: [f32; 3],
    pub vuser4: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct UserCmdT {
    pub lerp_msec: i16,
    pub msec: u8,
    pub pad: u8,
    pub viewangles: [f32; 3],
    pub forwardmove: f32,
    pub sidemove: f32,
    pub upmove: f32,
    pub lightlevel: u8,
    pub pad2: u8,
    pub buttons: u16,
    pub impulse: u8,
    pub weaponselect: u8,
    pub impact_index: c_int,
    pub impact_position: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PMPlaneT {
    pub normal: [f32; 3],
    pub dist: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PMTraceT {
    pub allsolid: c_int,
    pub startsolid: c_int,
    pub inopen: c_int,
    pub inwater: c_int,
    pub fraction: f32,
    pub endpos: [f32; 3],
    pub plane: PMPlaneT,
    pub ent: c_int,
    pub deltavelocity: [f32; 3],
    pub hitgroup: c_int,
}

#[repr(C)]
pub struct PlayermoveT {
    pub player_index: c_int,
    pub server: c_int,
    pub multiplayer: c_int,
    pub time: f32,
    pub frametime: f32,
    pub forward: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub oldangles: [f32; 3],
    pub velocity: [f32; 3],
    pub movedir: [f32; 3],
    pub basevelocity: [f32; 3],
    pub view_ofs: [f32; 3],
    pub flDuckTime: f32,
    pub bInDuck: c_int,
    pub flTimeStepSound: c_int,
    pub iStepLeft: c_int,
    pub flFallVelocity: f32,
    pub punchangle: [f32; 3],
    pub flSwimTime: f32,
    pub flNextPrimaryAttack: f32,
    pub effects: c_int,
    pub flags: c_int,
    pub usehull: c_int,
    pub gravity: f32,
    pub friction: f32,
    pub oldbuttons: c_int,
    pub waterjumptime: f32,
    pub dead: c_int,
    pub deadflag: c_int,
    pub spectator: c_int,
    pub movetype: c_int,
    pub onground: c_int,
    pub waterlevel: c_int,
    pub watertype: c_int,
    pub oldwaterlevel: c_int,
    pub sztexturename: [c_char; 256],
    pub chtexturetype: c_char,
    pub maxspeed: f32,
    pub clientmaxspeed: f32,
    pub iuser1: c_int,
    pub iuser2: c_int,
    pub iuser3: c_int,
    pub iuser4: c_int,
    pub fuser1: f32,
    pub fuser2: f32,
    pub fuser3: f32,
    pub fuser4: f32,
    pub vuser1: [f32; 3],
    pub vuser2: [f32; 3],
    pub vuser3: [f32; 3],
    pub vuser4: [f32; 3],
    pub numphysent: c_int,
    pub physents: [PhysentT; 600],
    pub nummoveent: c_int,
    pub moveents: [PhysentT; 64],
    pub numvisent: c_int,
    pub visents: [PhysentT; 600],
    pub cmd: UserCmdT,
    pub numtouch: c_int,
    pub touchindex: [PMTraceT; 600],
    pub physinfo: [c_char; 256],
    pub movevars: *mut c_void,
    pub player_mins: [[f32; 3]; 4],
    pub player_maxs: [[f32; 3]; 4],
    pub PM_Info_ValueForKey: *mut c_void,
    pub PM_Particle: *mut c_void,
    pub PM_TestPlayerPosition: *mut c_void,
    pub Con_NPrintf: *mut c_void,
    pub Con_DPrintf: *mut c_void,
    pub Con_Printf: *mut c_void,
    pub Sys_FloatTime: *mut c_void,
    pub PM_StuckTouch: *mut c_void,
    pub PM_PointContents: *mut c_void,
    pub PM_TruePointContents: *mut c_void,
    pub PM_HullPointContents: *mut c_void,
    pub PM_PlayerTrace: *mut c_void,
    pub PM_TraceLine: *mut c_void,
    pub RandomLong: *mut c_void,
    pub RandomFloat: *mut c_void,
    pub PM_GetModelType: *mut c_void,
    pub PM_GetModelBounds: *mut c_void,
    pub PM_HullForBsp: *mut c_void,
    pub PM_TraceModel: *mut c_void,
    pub COM_FileSize: *mut c_void,
    pub COM_LoadFile: *mut c_void,
    pub COM_FreeFile: *mut c_void,
    pub memfgets: *mut c_void,
    pub runfuncs: c_int,
    pub PM_PlaySound: Option<FnPMPlaySound>,
    pub PM_TraceTexture: *mut c_void,
    pub PM_PlaybackEventFull: Option<FnPMPlaybackEventFull>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ListenerState {
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub forward: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
}

static LISTENER_STATE: Mutex<ListenerState> = Mutex::new(ListenerState {
    origin: [0.0; 3],
    angles: [0.0; 3],
    forward: [0.0, 1.0, 0.0],
    right: [1.0, 0.0, 0.0],
    up: [0.0, 0.0, 1.0],
});

pub static PLAYER_ENT_ORIGINS: Mutex<[[f32; 3]; 64]> = Mutex::new([[0.0; 3]; 64]);

pub fn get_listener_state() -> ListenerState {
    LISTENER_STATE.lock().map(|s| *s).unwrap_or_default()
}

pub fn update_listener_state(
    origin: [f32; 3],
    angles: [f32; 3],
    forward: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
) {
    if let Ok(mut state) = LISTENER_STATE.lock() {
        state.origin = origin;
        state.angles = angles;
        state.forward = forward;
        state.right = right;
        state.up = up;
    }
}

pub unsafe fn hook_player_move(ppmove: *mut c_void) {
    if ppmove.is_null() {
        return;
    }

    let pm = &mut *(ppmove as *mut PlayermoveT);

    if !pm.PM_TraceTexture.is_null() {
        crate::engine::acoustics::G_PM_TRACE_TEXTURE.store(pm.PM_TraceTexture, Ordering::Relaxed);
    }

    if let Ok(mut origins) = PLAYER_ENT_ORIGINS.lock() {
        let count = (pm.numphysent as usize)
            .min(origins.len())
            .min(pm.physents.len());
        for i in 0..count {
            let pe = &pm.physents[i];
            if pe.info >= 1 && (pe.info as usize) < origins.len() && pe.origin != [0.0, 0.0, 0.0] {
                origins[pe.info as usize] = pe.origin;
            }
        }
    }

    if pm.server != 0 {
        return;
    }

    if let Ok(mut orig) = LAST_PMMOVE_ORIGIN.lock() {
        *orig = pm.origin;
    }

    if pm.origin != [0.0, 0.0, 0.0] {
        let ear_origin = [
            pm.origin[0] + pm.view_ofs[0],
            pm.origin[1] + pm.view_ofs[1],
            pm.origin[2] + pm.view_ofs[2],
        ];
        update_listener_state(ear_origin, pm.angles, pm.forward, pm.right, pm.up);
        crate::engine::acoustics::update_room_acoustics(pm.origin);
    }
}

pub fn get_entity_world_pos(entnum: c_int) -> Option<[f32; 3]> {
    if entnum < 1 {
        return None;
    }

    if let Ok(origins) = PLAYER_ENT_ORIGINS.lock() {
        if (entnum as usize) < origins.len() {
            let pos = origins[entnum as usize];
            if pos != [0.0, 0.0, 0.0] {
                return Some(pos);
            }
        }
    }

    if (1..=64).contains(&entnum) {
        unsafe {
            if let Some(get_ent) = crate::engine::cdll_int::G_ENGFUNCS.GetEntityByIndex {
                let raw_ptr = get_ent(entnum);
                if (raw_ptr as usize) > 0x10000 {
                    let cl_ent = raw_ptr as *const crate::engine::sdk::ClEntity;
                    let ent = &*cl_ent;
                    if ent.origin != [0.0, 0.0, 0.0] {
                        return Some(ent.origin);
                    }
                    if ent.curstate.origin != [0.0, 0.0, 0.0] {
                        return Some(ent.curstate.origin);
                    }
                }
            }
        }
    }

    None
}
pub fn compute_spatial_vector(sound_origin: [f32; 3]) -> ([f32; 3], f32) {
    let listener = get_listener_state();
    let dx = sound_origin[0] - listener.origin[0];
    let dy = sound_origin[1] - listener.origin[1];
    let dz = sound_origin[2] - listener.origin[2];

    let distance = (dx * dx + dy * dy + dz * dz).sqrt();

    let local_x = dx * listener.right[0] + dy * listener.right[1] + dz * listener.right[2];
    let local_y = dx * listener.forward[0] + dy * listener.forward[1] + dz * listener.forward[2];
    let local_z = dx * listener.up[0] + dy * listener.up[1] + dz * listener.up[2];

    ([local_x, local_y, local_z], distance)
}

pub fn clean_sound_name(raw: &str) -> &str {
    let mut s = raw.trim();
    while s.starts_with('&')
        || s.starts_with('*')
        || s.starts_with('?')
        || s.starts_with('!')
        || s.starts_with('#')
        || s.starts_with('@')
        || s.starts_with(')')
        || s.starts_with('(')
        || s.starts_with('~')
        || s.starts_with('$')
        || s.starts_with('%')
    {
        s = &s[1..];
    }
    s
}

pub fn is_hrtf_pcm_bypassed() -> bool {
    HRTF_ENABLE.value_or(1.0) == 0.0 || HRTF_VOLUME.value_or(1.0) <= 0.0
}

pub fn scaled_sound_volume(volume: f32) -> f32 {
    if is_hrtf_pcm_bypassed() {
        volume
    } else {
        volume * HRTF_VOLUME.value_or(1.0)
    }
}

unsafe fn process_named_sound_ptr(
    sound_ptr: *const c_char,
    volume: f32,
    sound_origin: Option<[f32; 3]>,
) -> f32 {
    if sound_ptr.is_null() {
        return volume;
    }

    let name = CStr::from_ptr(sound_ptr).to_string_lossy();
    process_hrtf_sound(&name, volume, sound_origin)
}

fn origin_from_ptr(origin: *const f32) -> Option<[f32; 3]> {
    if origin.is_null() {
        None
    } else {
        let pos = unsafe { [*origin, *origin.add(1), *origin.add(2)] };
        if pos == [0.0, 0.0, 0.0] {
            None
        } else {
            Some(pos)
        }
    }
}

pub fn resolve_sound_world_origin(origin: *const f32, entnum: c_int) -> Option<[f32; 3]> {
    let ent_pos = get_entity_world_pos(entnum);
    let orig_pos = origin_from_ptr(origin);

    match (orig_pos, ent_pos) {
        (Some(p), Some(ep)) => {
            let p_len_sq = p[0] * p[0] + p[1] * p[1] + p[2] * p[2];
            if p_len_sq < 300.0 * 300.0 {
                if p_len_sq < 4.0 {
                    Some(ep)
                } else {
                    Some([ep[0] + p[0], ep[1] + p[1], ep[2] + p[2]])
                }
            } else {
                Some(p)
            }
        }
        (Some(p), None) => Some(p),
        (None, Some(ep)) => Some(ep),
        (None, None) => None,
    }
}

pub unsafe fn is_local_entity_or_near(entnum: c_int, orig_raw: Option<[f32; 3]>) -> bool {
    let local_idx = if let Some(get_local) = crate::engine::cdll_int::G_ENGFUNCS.GetLocalPlayer {
        let local_ptr = get_local() as *const crate::engine::sdk::ClEntity;
        if !local_ptr.is_null() {
            (*local_ptr).index
        } else {
            1
        }
    } else {
        1
    };

    if entnum == local_idx || entnum == 0 || entnum == -1 {
        return true;
    }

    if entnum > 0 && entnum != local_idx && entnum <= 32 {
        return false;
    }

    if let Ok(my_orig) = LAST_PMMOVE_ORIGIN.lock() {
        if *my_orig != [0.0, 0.0, 0.0] {
            if let Some(p) = orig_raw {
                let dist_sq = (p[0] - my_orig[0]).powi(2)
                    + (p[1] - my_orig[1]).powi(2)
                    + (p[2] - my_orig[2]).powi(2);
                if dist_sq < 22500.0 {
                    return true;
                }
            }
        }
    }

    false
}

pub fn is_casing_sound(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("shell")
        || lower.contains("sshell")
        || lower.contains("debris")
        || lower.contains("pl_shell")
}

pub fn is_explosive_sound(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("hegrenade")
        || lower.contains("explode")
        || lower.contains("explosion")
        || lower.contains("flashbang")
        || lower.contains("sg_explode")
        || lower.contains("c4_explode")
        || lower.contains("grenade")
        || lower.contains("createsmoke")
        || lower.contains("createexplosion")
}

pub fn is_reload_sound(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("reload")
        || lower.contains("clipin")
        || lower.contains("clipout")
        || lower.contains("boltpull")
        || lower.contains("boltslap")
        || lower.contains("clip")
        || lower.contains("slide")
        || lower.contains("insert")
        || lower.contains("draw")
}

pub fn sound_threat_weight(name: &str) -> f32 {
    let lower = name.to_lowercase();
    if is_explosive_sound(&lower) {
        5.0
    } else if lower.contains("weapons/")
        && (lower.contains("-1.wav")
            || lower.contains("-2.wav")
            || lower.contains("shoot")
            || lower.contains("fire"))
    {
        4.0
    } else if is_reload_sound(&lower) {
        3.5
    } else if lower.contains("knife") || lower.contains("step") {
        2.5
    } else if lower.contains("door") || lower.contains("button") || lower.contains("ric") {
        1.5
    } else if is_casing_sound(&lower) {
        0.15
    } else {
        1.0
    }
}

pub fn process_hrtf_sound(sound_name: &str, volume: f32, sound_origin: Option<[f32; 3]>) -> f32 {
    if is_hrtf_pcm_bypassed() {
        return volume;
    }

    let cleaned_name = clean_sound_name(sound_name);
    if cleaned_name.is_empty() {
        return volume;
    }

    let sound_origin = sound_origin.filter(|pos| *pos != [0.0, 0.0, 0.0]);
    let acoustics = crate::engine::acoustics::get_current_acoustics();
    let is_explosion = is_explosive_sound(cleaned_name);
    let is_reload = is_reload_sound(cleaned_name);
    let mut final_volume = volume;

    if let Some(pos) = sound_origin {
        let (_local_vec, dist) = compute_spatial_vector(pos);
        let listener = get_listener_state();
        let prop =
            unsafe { crate::engine::acoustics::evaluate_sound_propagation(pos, listener.origin) };
        let acoustics_enabled = HRTF_ACOUSTICS.enabled();

        if is_casing_sound(cleaned_name) {
            const CASING_MAX_DIST: f32 = 260.0;
            if dist > CASING_MAX_DIST {
                return 0.0;
            }
            if prop.occlusion > 0.15
                && (!prop.is_diffracted || dist > 120.0 || prop.diffraction_angle_deg > 45.0)
            {
                return 0.0;
            }
            let dist_fade = (1.0 - (dist / CASING_MAX_DIST)).clamp(0.0, 1.0).powi(2);
            let casing_vol = volume * 0.35 * dist_fade;
            return if casing_vol < 0.01 { 0.0 } else { casing_vol };
        }

        let (effective_volume_factor, effective_occlusion) = if is_explosion {
            (prop.volume_factor.max(0.75), prop.occlusion.min(0.30))
        } else {
            (prop.volume_factor.max(0.55), prop.occlusion)
        };

        let occl_factor = if effective_occlusion > 0.1 {
            (1.0 - effective_occlusion * 0.40).clamp(0.40, 1.0)
        } else {
            1.0
        };

        final_volume = volume * effective_volume_factor * occl_factor;

        if acoustics_enabled {
            let enclosure_boost = acoustics.enclosure_factor.clamp(0.0, 1.0);
            let reflection_boost = (acoustics.reflection_mix / 0.60).clamp(0.0, 1.0);
            let room_vol_scale = 1.00 + (enclosure_boost * 0.20) + (reflection_boost * 0.20);
            final_volume *= room_vol_scale;
        }

        final_volume = final_volume.clamp(0.02, 2.50);

        if is_reload {
            crate::con_println!(
                "[TRACK RELOAD 3D] '{}' pos={:?} dist={:.1} occl={:.2} vol={:.2}",
                cleaned_name,
                pos,
                dist,
                effective_occlusion,
                final_volume
            );
        }

        crate::engine::acoustics_debug::track_3d_sound(
            cleaned_name,
            pos,
            final_volume,
            effective_occlusion > 0.1,
        );
    } else if is_reload {
        crate::con_println!(
            "[DROP RELOAD 3D] Sound '{}' NOT added to 3D nodes because sound_origin is NONE!",
            cleaned_name
        );
    }

    final_volume
}

pub unsafe fn init_sound_hooks(penginefuncs: *mut ClEngineFuncs) {
    if penginefuncs.is_null() {
        return;
    }
    crate::engine::engine_sound_hook::install_engine_sound_hooks();
}

pub unsafe extern "C" fn hk_ev_play_sound(
    ent: c_int,
    origin: *const f32,
    channel: c_int,
    sample: *const c_char,
    volume: f32,
    attenuation: f32,
    f_flags: c_int,
    pitch: c_int,
) {
    let orig_raw = origin_from_ptr(origin);
    let is_local = is_local_entity_or_near(ent, orig_raw);

    let effective_volume = scaled_sound_volume(volume);
    let sound_pos = if is_local {
        None
    } else {
        resolve_sound_world_origin(origin, ent)
    };

    let final_vol = process_named_sound_ptr(sample, effective_volume, sound_pos);

    let ptr = ORIG_EV_PLAY_SOUND.load(Ordering::Relaxed);
    if !ptr.is_null() {
        let orig: PfnEVPlaySound = std::mem::transmute(ptr);
        orig(
            ent,
            origin,
            channel,
            sample,
            final_vol.max(0.01),
            attenuation,
            f_flags,
            pitch,
        );
    }
}
pub unsafe fn check_and_hook_event_api() {
    if !crate::engine::cdll_int::G_ENGFUNCS.pEventAPI.is_null() {
        let event_api = &mut *crate::engine::cdll_int::G_ENGFUNCS.pEventAPI;
        if !event_api.EV_PlaySound.is_null()
            && !std::ptr::eq(
                event_api.EV_PlaySound as *const (),
                hk_ev_play_sound as *const (),
            )
        {
            ORIG_EV_PLAY_SOUND.store(event_api.EV_PlaySound, Ordering::Relaxed);
            event_api.EV_PlaySound = hk_ev_play_sound as *mut _;
        }
    }
}

pub unsafe fn hook_studio_event(event: *const c_void, entity: *const c_void) {
    if event.is_null() {
        return;
    }

    #[repr(C)]
    struct MStudioEvent {
        pub frame: c_int,
        pub event: c_int,
        pub event_type: c_int,
        pub options: [c_char; 64],
    }

    let ev = &*(event as *const MStudioEvent);
    let options_ptr = ev.options.as_ptr();
    if *options_ptr == 0 {
        return;
    }

    let sound_name = CStr::from_ptr(options_ptr).to_string_lossy();
    let cleaned = clean_sound_name(&sound_name);
    if cleaned.is_empty() {
        return;
    }

    let origin = if !entity.is_null() {
        let ent = &*(entity as *const crate::engine::sdk::ClEntity);
        if ent.origin != [0.0, 0.0, 0.0] {
            Some(ent.origin)
        } else if ent.curstate.origin != [0.0, 0.0, 0.0] {
            Some(ent.curstate.origin)
        } else if ent.attachment[0] != [0.0, 0.0, 0.0] {
            Some(ent.attachment[0])
        } else if ent.prevstate.origin != [0.0, 0.0, 0.0] {
            Some(ent.prevstate.origin)
        } else {
            get_entity_world_pos(ent.index)
        }
    } else {
        None
    };

    if cleaned.contains("reload") || cleaned.contains("clip") || cleaned.contains("bolt") {
        crate::con_println!(
            "[DEBUG STUDIO_EVENT] ev={} sound='{}' ent_idx={} origin={:?}",
            ev.event,
            cleaned,
            if !entity.is_null() {
                (*(entity as *const crate::engine::sdk::ClEntity)).index
            } else {
                -1
            },
            origin
        );
    }

    let volume = if HRTF_ENABLE.value() != 0.0 {
        scaled_sound_volume(1.0)
    } else {
        1.0
    };

    let ent_index = if !entity.is_null() {
        (*(entity as *const crate::engine::sdk::ClEntity)).index
    } else {
        -1
    };

    let is_local = is_local_entity_or_near(ent_index, origin);
    let sound_origin = if is_local { None } else { origin };

    process_hrtf_sound(cleaned, volume, sound_origin);
}

pub unsafe fn wrap_cb_sound(cb_sound: *mut c_void) -> *mut c_void {
    if cb_sound.is_null() {
        return cb_sound;
    }
    CURRENT_CB_SOUND.store(cb_sound, Ordering::Relaxed);
    hk_temp_ent_sound as *mut _
}

pub unsafe extern "C" fn hk_temp_ent_sound(ptemp: *mut TempEntityT, damp: f32) {
    let mut effective_damp = damp.clamp(0.05, 1.0);

    if !ptemp.is_null() {
        let te = &*ptemp;

        let sound_pos = if te.entity.origin != [0.0, 0.0, 0.0] {
            Some(te.entity.origin)
        } else if te.entity.curstate.origin != [0.0, 0.0, 0.0] {
            Some(te.entity.curstate.origin)
        } else if [te.x, te.y, te.z] != [0.0, 0.0, 0.0] {
            Some([te.x, te.y, te.z])
        } else {
            None
        };

        let sound_name: Option<&'static str> = match te.hit_sound {
            BOUNCE_GLASS => Some("debris/glass"),
            BOUNCE_METAL => Some("debris/metal"),
            BOUNCE_FLESH => Some("debris/flesh"),
            BOUNCE_WOOD => Some("debris/wood"),
            BOUNCE_SHRAP => Some("weapons/ric"),
            BOUNCE_SHELL => Some("player/pl_shell"),
            BOUNCE_CONCRETE => Some("debris/concrete"),
            BOUNCE_SHOTSHELL => Some("weapons/sshell"),
            _ => None,
        };

        if let Some(name) = sound_name {
            let is_local = is_local_entity_or_near(te.entity.index, sound_pos);
            let sound_pos_hrtf = if is_local { None } else { sound_pos };
            effective_damp = process_hrtf_sound(name, effective_damp, sound_pos_hrtf);
        }
    }

    if effective_damp <= 0.005 {
        return;
    }

    let ptr = CURRENT_CB_SOUND.load(Ordering::Relaxed);
    if !ptr.is_null() {
        let orig: FnTempEntSoundCallback = std::mem::transmute(ptr);
        orig(ptemp, effective_damp);
    }
}

pub fn trigger_pain_sound(name: &str, volume: f32, _origin: [f32; 3]) {
    let cleaned = clean_sound_name(name);
    process_hrtf_sound(cleaned, volume, None);
}
