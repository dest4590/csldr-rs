use crate::utils::maths::Vec3;
use crate::utils::msg::MsgRead;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub static RECALC_SHELL: AtomicBool = AtomicBool::new(false);
pub static SHELL_ROTATION: Mutex<f32> = Mutex::new(0.0);
pub static SHELL_ORIGIN: Mutex<Vec3> = Mutex::new([0.0, 0.0, 0.0]);
pub static SHELL_VELOCITY: Mutex<Vec3> = Mutex::new([0.0, 0.0, 0.0]);

pub fn hk_msg_func_brass(pbuf: &[u8], is_czero: bool, should_mirror: bool) {
    if !should_mirror {
        return;
    }

    let mut read = MsgRead::new(pbuf);
    let mut orig = [0.0f32; 3];
    let mut vel = [0.0f32; 3];

    let rot = if is_czero {
        orig[0] = read.read_coord().unwrap_or(0.0);
        orig[1] = read.read_coord().unwrap_or(0.0);
        orig[2] = read.read_coord().unwrap_or(0.0);
        vel[0] = read.read_coord().unwrap_or(0.0);
        vel[1] = read.read_coord().unwrap_or(0.0);
        vel[2] = read.read_coord().unwrap_or(0.0);
        read.read_angle().unwrap_or(0.0)
    } else {
        read.read_byte();
        orig[0] = read.read_coord().unwrap_or(0.0);
        orig[1] = read.read_coord().unwrap_or(0.0);
        orig[2] = read.read_coord().unwrap_or(0.0);
        read.read_coord();
        read.read_coord();
        read.read_coord();
        vel[0] = read.read_coord().unwrap_or(0.0);
        vel[1] = read.read_coord().unwrap_or(0.0);
        vel[2] = read.read_coord().unwrap_or(0.0);
        read.read_angle().unwrap_or(0.0)
    };

    if let Ok(mut g_orig) = SHELL_ORIGIN.lock() {
        *g_orig = orig;
    }
    if let Ok(mut g_vel) = SHELL_VELOCITY.lock() {
        *g_vel = vel;
    }
    if let Ok(mut g_rot) = SHELL_ROTATION.lock() {
        *g_rot = rot;
    }

    RECALC_SHELL.store(true, Ordering::Relaxed);
}

pub fn hk_temp_model_recalc(righthand: bool) -> (Vec3, Vec3) {
    RECALC_SHELL.store(false, Ordering::Relaxed);

    let rot_deg = SHELL_ROTATION.lock().map_or(0.0, |r| *r);
    let rot = rot_deg.to_radians();
    let (sine, cosine) = rot.sin_cos();

    let mut orig = SHELL_ORIGIN.lock().map_or([0.0, 0.0, 0.0], |o| *o);
    let mut vel = SHELL_VELOCITY.lock().map_or([0.0, 0.0, 0.0], |v| *v);

    let (x, y) = if !righthand {
        vel[0] -= sine * 120.0;
        vel[1] += cosine * 120.0;
        (-9.0 * sine, 9.0 * cosine)
    } else {
        (9.0 * sine, -9.0 * cosine)
    };

    orig[0] += x;
    orig[1] += y;

    (orig, vel)
}
