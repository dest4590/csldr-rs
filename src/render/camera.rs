use crate::utils::cvars::{HRTF_FREECAM, HRTF_FREECAM_SPEED};
use crate::utils::maths::{angle_lerp, angle_vectors, Vec3};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub static FREECAM_ENABLED: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
pub struct FreecamState {
    pub origin: Vec3,
    pub angles: Vec3,
    pub initialized: bool,
}

pub static FREECAM_STATE: Mutex<FreecamState> = Mutex::new(FreecamState {
    origin: [0.0, 0.0, 0.0],
    angles: [0.0, 0.0, 0.0],
    initialized: false,
});

pub fn is_freecam_active() -> bool {
    FREECAM_ENABLED.load(Ordering::Relaxed) || HRTF_FREECAM.value() != 0.0
}

pub fn toggle_freecam() -> bool {
    let current = FREECAM_ENABLED.load(Ordering::Relaxed);
    let new_state = !current;
    FREECAM_ENABLED.store(new_state, Ordering::Relaxed);
    if !new_state {
        if let Ok(mut state) = FREECAM_STATE.lock() {
            state.initialized = false;
        }
    }
    new_state
}

pub fn update_freecam(params: &mut crate::engine::sdk::RefParams) {
    if !is_freecam_active() {
        return;
    }

    let mut state = match FREECAM_STATE.lock() {
        Ok(s) => s,
        Err(_) => return,
    };

    if !state.initialized {
        state.origin = params.vieworg;
        state.angles = params.cl_viewangles;
        state.initialized = true;
    } else {
        state.angles = params.cl_viewangles;
    }

    let mut forward = [0.0f32; 3];
    let mut right = [0.0f32; 3];
    let mut up = [0.0f32; 3];
    angle_vectors(
        &state.angles,
        Some(&mut forward),
        Some(&mut right),
        Some(&mut up),
    );

    let speed_cvar = HRTF_FREECAM_SPEED.value_or(500.0);
    let move_speed = speed_cvar * params.frametime.max(0.001);

    let (key_w, key_s, key_a, key_d, key_jump, key_duck) =
        if let Ok(mov) = crate::hud::movement_gui::MOVEMENT_STATE.lock() {
            (
                mov.key_w,
                mov.key_s,
                mov.key_a,
                mov.key_d,
                mov.key_jump,
                mov.key_duck,
            )
        } else {
            (false, false, false, false, false, false)
        };

    if key_w {
        state.origin[0] += forward[0] * move_speed;
        state.origin[1] += forward[1] * move_speed;
        state.origin[2] += forward[2] * move_speed;
    }
    if key_s {
        state.origin[0] -= forward[0] * move_speed;
        state.origin[1] -= forward[1] * move_speed;
        state.origin[2] -= forward[2] * move_speed;
    }
    if key_d {
        state.origin[0] += right[0] * move_speed;
        state.origin[1] += right[1] * move_speed;
        state.origin[2] += right[2] * move_speed;
    }
    if key_a {
        state.origin[0] -= right[0] * move_speed;
        state.origin[1] -= right[1] * move_speed;
        state.origin[2] -= right[2] * move_speed;
    }
    if key_jump {
        state.origin[2] += move_speed;
    }
    if key_duck {
        state.origin[2] -= move_speed;
    }

    params.vieworg = state.origin;
    params.viewangles = state.angles;
    params.forward = forward;
    params.right = right;
    params.up = up;
}

#[derive(Debug)]
struct CameraState {
    prev_move: Vec3,
    cur_move: Vec3,
    lerp_time: f32,
}

static STATE: Mutex<CameraState> = Mutex::new(CameraState {
    prev_move: [0.0, 0.0, 0.0],
    cur_move: [0.0, 0.0, 0.0],
    lerp_time: 0.0,
});

pub fn apply_camera_movement(
    view_angles: &mut Vec3,
    scale: f32,
    interp: f32,
    client_time: f32,
    new_movement: &Vec3,
    changed_hdr: bool,
) {
    let mut state = match STATE.lock() {
        Ok(s) => s,
        Err(_) => return,
    };

    if changed_hdr && interp > 0.0 {
        state.lerp_time = client_time;
        state.prev_move = state.cur_move;
    }

    if state.lerp_time > 0.0 {
        let frac = (client_time - state.lerp_time) / interp;
        if !(0.0..1.0).contains(&frac) {
            state.cur_move = *new_movement;
            state.lerp_time = 0.0;
        } else {
            let prev = state.prev_move;
            angle_lerp(&prev, new_movement, frac, &mut state.cur_move);
        }
    } else {
        state.cur_move = *new_movement;
    }

    for (angle, &cur) in view_angles.iter_mut().zip(&state.cur_move) {
        *angle += cur * scale;
    }
}
