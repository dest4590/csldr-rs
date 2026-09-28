use crate::utils::cvars::FOV_HORPLUS;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;

pub static FOV_SCALE: Mutex<f32> = Mutex::new(1.0);
pub static FOV_VALUE_ORIGINAL: AtomicI32 = AtomicI32::new(90);

#[derive(Debug)]
struct FovState {
    current_fov: f32,
    dest_fov: f32,
    initial_fov: f32,
    fov_time: f32,
    fov_lerp: f32,
}

static STATE: Mutex<FovState> = Mutex::new(FovState {
    current_fov: 90.0,
    dest_fov: 90.0,
    initial_fov: 90.0,
    fov_time: 0.0,
    fov_lerp: 0.0,
});

pub fn get_current_fov(screen_width: i32, screen_height: i32) -> f32 {
    let state = match STATE.lock() {
        Ok(s) => s,
        Err(_) => return 90.0,
    };
    let w = screen_width as f32;
    let h = screen_height as f32;

    if FOV_HORPLUS.value() != 0.0 && h > 0.0 && (w / h != 0.75) {
        (((state.current_fov.to_radians() / 2.0).tan() * (w / h * 0.75)).atan() * 2.0).to_degrees()
    } else {
        state.current_fov
    }
}

fn set_fov(fov: f32, state: &mut FovState) {
    if let Ok(mut scale) = FOV_SCALE.lock() {
        *scale = fov / 90.0;
    }
    state.current_fov = fov;
}

fn force_dest_fov(state: &mut FovState) {
    state.initial_fov = state.dest_fov;
    set_fov(state.dest_fov, state);
}

#[inline]
fn fov_interp(a: f32, b: f32, f: f32) -> f32 {
    let f_adj = f - 1.0;
    (b - a) * (1.0 - f_adj * f_adj).sqrt() + a
}

pub fn fov_think(client_time: f32) {
    let mut state = match STATE.lock() {
        Ok(s) => s,
        Err(_) => return,
    };

    if state.dest_fov as i32 == state.initial_fov as i32 {
        return;
    }

    if state.fov_lerp <= 0.0 || client_time < state.fov_time {
        force_dest_fov(&mut state);
        return;
    }

    let f = (client_time - state.fov_time) / state.fov_lerp;
    if f >= 1.0 {
        force_dest_fov(&mut state);
        return;
    }

    let fov = fov_interp(state.initial_fov, state.dest_fov, f);
    set_fov(fov, &mut state);
}

pub fn set_lerp_fov(fov: f32, lerp: f32, client_time: f32) {
    let mut state = match STATE.lock() {
        Ok(s) => s,
        Err(_) => return,
    };

    if state.dest_fov as i32 == fov as i32 {
        return;
    }

    state.fov_lerp = lerp;
    state.initial_fov = state.dest_fov;
    state.fov_time = client_time;
    state.dest_fov = fov;
}

pub fn hk_msg_func_set_fov(data: &[u8], client_time: f32, lerp_value: f32) -> i32 {
    let fov_raw = data.first().map_or(90, |&b| b as i32);
    let fov = if (1..=180).contains(&fov_raw) {
        fov_raw
    } else {
        90
    };

    FOV_VALUE_ORIGINAL.store(fov, Ordering::Relaxed);
    set_lerp_fov(fov as f32, lerp_value, client_time);

    fov
}
