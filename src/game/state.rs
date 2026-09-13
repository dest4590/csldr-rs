use crate::utils::maths::vector_length;
use crate::utils::msg::MsgRead;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;

pub static CURRENT_WEAPON_STATE: AtomicI32 = AtomicI32::new(0);
pub static CURRENT_WEAPON_IN_RELOAD: AtomicI32 = AtomicI32::new(0);
pub static CURRENT_WEAPON_IN_SPECIAL_RELOAD: AtomicI32 = AtomicI32::new(0);

pub fn update_post_run_cmd(
    flags: i32,
    velocity: &[f32; 3],
    weapon_state: i32,
    in_reload: i32,
    in_special_reload: i32,
) {
    crate::hud::crosshair::XHAIR_PLAYER_FLAGS.store(flags, Ordering::Relaxed);
    if let Ok(mut speed) = crate::hud::crosshair::XHAIR_PLAYER_SPEED.lock() {
        *speed = vector_length(velocity);
    }
    crate::hud::crosshair::XHAIR_WEAPON_FLAGS.store(weapon_state, Ordering::Relaxed);
    CURRENT_WEAPON_STATE.store(weapon_state, Ordering::Relaxed);
    CURRENT_WEAPON_IN_RELOAD.store(in_reload, Ordering::Relaxed);
    CURRENT_WEAPON_IN_SPECIAL_RELOAD.store(in_special_reload, Ordering::Relaxed);
}

pub const TEAM_UNASSIGNED: i32 = 0;
pub const TEAM_TERRORIST: i32 = 1;
pub const TEAM_CT: i32 = 2;
pub const TEAM_SPECTATOR: i32 = 3;

#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerInfo {
    pub team: i32,
}

pub static PLAYER_INFO: Mutex<[PlayerInfo; 65]> = Mutex::new([PlayerInfo { team: 0 }; 65]);
pub static LOCAL_TEAM: AtomicI32 = AtomicI32::new(TEAM_UNASSIGNED);
pub static USER1: AtomicI32 = AtomicI32::new(0);
pub static USER2: AtomicI32 = AtomicI32::new(0);

pub fn hk_msg_func_team_info(pbuf: &[u8], local_player_index: i32) -> i32 {
    let mut read = MsgRead::new(pbuf);
    if let Some(index) = read.read_byte() {
        let team_name = read.read_string();
        let idx = index as usize;

        if idx < 65 {
            let team = match team_name.as_str() {
                "CT" => TEAM_CT,
                "TERRORIST" => TEAM_TERRORIST,
                "SPECTATOR" => TEAM_SPECTATOR,
                _ => TEAM_UNASSIGNED,
            };

            if let Ok(mut infos) = PLAYER_INFO.lock() {
                infos[idx].team = team;
            }

            if index as i32 == local_player_index {
                LOCAL_TEAM.store(team, Ordering::Relaxed);
            }
        }
    }
    0
}

pub fn hk_process_player_state(
    dst_number: i32,
    local_index: i32,
    src_iuser1: i32,
    src_iuser2: i32,
) {
    if local_index == dst_number {
        USER1.store(src_iuser1, Ordering::Relaxed);
        USER2.store(src_iuser2, Ordering::Relaxed);
    }
}
