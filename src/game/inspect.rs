use crate::engine::sdk::{ClEntity, MStudioSeqDesc, StudioHdr};
use crate::game::state::{CURRENT_WEAPON_IN_RELOAD, CURRENT_WEAPON_IN_SPECIAL_RELOAD, USER1};
use crate::hud::crosshair::*;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;

const INSPECT_ANIMS: &[i32] = &[
    0,  // none
    7,  // p228
    13, // glock
    5,  // scout
    4,  // hegrenade
    7,  // xm1014
    4,  // c4
    6,  // mac10
    6,  // aug
    4,  // smokegrenade
    16, // elite
    6,  // fiveseven
    6,  // ump45
    5,  // sg550
    6,  // galil
    6,  // famas
    16, // usp
    13, // glock18
    6,  // awp
    6,  // mp5n
    5,  // m249
    7,  // m3
    14, // m4a1
    6,  // tmp
    5,  // g3sg1
    4,  // flashbang
    6,  // deagle
    6,  // sg552
    6,  // ak47
    8,  // knife
    6,  // p90
];

pub const USP_UNSIL_IDLE: i32 = 8;
pub const USP_ATTACH_SILENCER: i32 = 14;
pub const USP_DETACH_SILENCER: i32 = 15;

pub const M4A1_UNSIL_IDLE: i32 = 7;
pub const M4A1_ATTACH_SILENCER: i32 = 12;
pub const M4A1_DETACH_SILENCER: i32 = 13;

pub const ELITE_IDLE_LEFTEMPTY: i32 = 1;
pub const GLOCK18_IDLE2: i32 = 1;
pub const GLOCK18_IDLE3: i32 = 2;

static INSPECT_END_TIME: Mutex<f32> = Mutex::new(0.0);
static OLD_VIEW_ANIM_TIME: Mutex<f32> = Mutex::new(0.0);
static OLD_VIEW_SEQUENCE: AtomicI32 = AtomicI32::new(0);

pub fn lookup_inspect(cur_sequence: i32, weapon_id: i32) -> i32 {
    let anim = match INSPECT_ANIMS.get(weapon_id as usize) {
        Some(&a) => a,
        None => return -1,
    };
    if weapon_id == WEAPON_USP {
        if cur_sequence >= USP_UNSIL_IDLE && cur_sequence != anim {
            return anim + 1;
        }
    } else if weapon_id == WEAPON_M4A1 && cur_sequence >= M4A1_UNSIL_IDLE && cur_sequence != anim {
        return anim + 1;
    }

    anim
}

fn set_inspect_time(studiohdr: *const StudioHdr, seq: i32, client_time: f32) {
    unsafe {
        if studiohdr.is_null() || seq < 0 {
            return;
        }
        let seqindex = (*studiohdr).seqindex as usize;
        let base_ptr = (studiohdr as *const u8).add(seqindex);
        let seq_desc_ptr = (base_ptr as *const MStudioSeqDesc).add(seq as usize);
        let numframes = (*seq_desc_ptr).numframes as f32;
        let fps = (*seq_desc_ptr).fps;
        if fps > 0.0 {
            let duration = numframes / fps;
            if let Ok(mut end_time) = INSPECT_END_TIME.lock() {
                *end_time = client_time + duration;
            }
        }
    }
}

pub fn is_idle_sequence(seq: i32) -> bool {
    if seq == 0 {
        return true;
    }

    let weapon_id = CURRENT_WEAPON_ID.load(Ordering::Relaxed);
    match weapon_id {
        WEAPON_ELITE => seq == ELITE_IDLE_LEFTEMPTY,
        WEAPON_USP => seq == USP_UNSIL_IDLE,
        WEAPON_GLOCK18 => seq == GLOCK18_IDLE2 || seq == GLOCK18_IDLE3,
        WEAPON_M4A1 => seq == M4A1_UNSIL_IDLE,
        _ => false,
    }
}

pub unsafe fn inspect_think(client_time: f32) {
    let inspect_end_time = match INSPECT_END_TIME.lock() {
        Ok(t) => *t,
        Err(_) => return,
    };

    if inspect_end_time > client_time {
        let get_vm_fn = match crate::cdll_int::G_ENGFUNCS.GetViewModel {
            Some(f) => f,
            None => return,
        };

        let vm = match (get_vm_fn() as *mut ClEntity).as_mut() {
            Some(v) => v,
            None => return,
        };
        let old_anim_time = match OLD_VIEW_ANIM_TIME.lock() {
            Ok(t) => *t,
            Err(_) => return,
        };
        let old_seq = OLD_VIEW_SEQUENCE.load(Ordering::Relaxed);

        if vm.curstate.animtime != old_anim_time && is_idle_sequence(vm.curstate.sequence) {
            vm.curstate.sequence = old_seq;
            vm.curstate.animtime = old_anim_time;
        }

        if let Ok(mut t) = OLD_VIEW_ANIM_TIME.lock() {
            *t = vm.curstate.animtime;
        }
        OLD_VIEW_SEQUENCE.store(vm.curstate.sequence, Ordering::Relaxed);
    }
}

pub unsafe fn inspect_f() {
    let user1 = USER1.load(Ordering::Relaxed);
    if user1 != 0 {
        return;
    }

    let in_reload = CURRENT_WEAPON_IN_RELOAD.load(Ordering::Relaxed);
    let in_special_reload = CURRENT_WEAPON_IN_SPECIAL_RELOAD.load(Ordering::Relaxed);
    if in_reload != 0 || in_special_reload != 0 {
        return;
    }

    let get_vm_fn = match crate::cdll_int::G_ENGFUNCS.GetViewModel {
        Some(f) => f,
        None => return,
    };

    let vm = match (get_vm_fn() as *mut ClEntity).as_ref() {
        Some(v) => v,
        None => return,
    };

    let cache_data = match vm.model.as_ref().map(|m| m.cache.data as *const StudioHdr) {
        Some(c) if !c.is_null() => c,
        _ => return,
    };

    let weapon_id = CURRENT_WEAPON_ID.load(Ordering::Relaxed);

    match (weapon_id, vm.curstate.sequence) {
        (WEAPON_USP, USP_ATTACH_SILENCER | USP_DETACH_SILENCER) => return,
        (WEAPON_M4A1, M4A1_ATTACH_SILENCER | M4A1_DETACH_SILENCER) => return,
        _ => {}
    }

    let sequence = lookup_inspect(vm.curstate.sequence, weapon_id);
    let numseq = (*cache_data).numseq;

    if sequence < 0 || sequence >= numseq {
        return;
    }

    let client_time = crate::engine::cdll_int::CLIENT_TIME
        .lock()
        .map_or(0.0, |t| *t);

    set_inspect_time(cache_data, sequence, client_time);

    if let Some(event_api) = crate::cdll_int::G_ENGFUNCS.pEventAPI.as_ref() {
        if let Some(anim_fn) = event_api.EV_WeaponAnimation {
            anim_fn(sequence, vm.curstate.body);
        }
    }
}
