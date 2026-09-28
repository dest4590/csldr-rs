use std::ffi::{c_char, CString};
use std::sync::atomic::Ordering;
use std::sync::Mutex;

use crate::engine::cdll_int::{G_ENGFUNCS, SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::engine::sdk::RefParams;
use crate::render::draw2d::{
    draw_rect_2d as draw_rect, draw_text_2d as draw_text, get_text_width as get_text_width_f32,
};
use crate::render::gl::*;
use crate::render::studiorenderer::CAN_OPENGL;
use crate::utils::cvars::*;
use crate::utils::maths::normalize_angle;

pub const IN_ATTACK: u16 = 1 << 0;
pub const IN_JUMP: u16 = 1 << 1;
pub const IN_DUCK: u16 = 1 << 2;
pub const IN_FORWARD: u16 = 1 << 3;
pub const IN_BACK: u16 = 1 << 4;
pub const IN_USE: u16 = 1 << 5;
pub const IN_CANCEL: u16 = 1 << 6;
pub const IN_LEFT: u16 = 1 << 7;
pub const IN_RIGHT: u16 = 1 << 8;
pub const IN_MOVELEFT: u16 = 1 << 9;
pub const IN_MOVERIGHT: u16 = 1 << 10;
pub const IN_ATTACK2: u16 = 1 << 11;

#[repr(C)]
pub struct UserCmd {
    pub lerp_msec: i16,
    pub msec: u8,
    pub _pad1: u8,
    pub viewangles: [f32; 3],
    pub forwardmove: f32,
    pub sidemove: f32,
    pub upmove: f32,
    pub lightlevel: u8,
    pub _pad2: u8,
    pub buttons: u16,
    pub impulse: u8,
    pub weaponselect: u8,
    pub _pad3: [u8; 2],
    pub impact_index: i32,
    pub impact_position: [f32; 3],
}

#[derive(Clone, Copy, Debug)]
pub struct LongjumpStats {
    pub distance: f32,
    pub prestrafe: f32,
    pub max_speed: f32,
    pub strafes: u32,
    pub sync_pct: f32,
    pub height_delta: f32,
    pub air_time: f32,
    pub timestamp: f32,
}

#[derive(Debug, Default)]
pub struct MovementState {
    pub vel_2d: f32,
    pub vel_3d: f32,
    pub vel_z: f32,
    pub speed_diff: f32,
    pub is_onground: bool,

    pub key_w: bool,
    pub key_a: bool,
    pub key_s: bool,
    pub key_d: bool,
    pub key_duck: bool,
    pub key_jump: bool,
    pub last_cmd_forward: f32,
    pub last_cmd_side: f32,

    pub in_air: bool,
    pub jump_origin: [f32; 3],
    pub takeoff_speed: f32,
    pub takeoff_time: f32,
    pub max_air_speed: f32,
    pub strafe_count: u32,
    pub current_strafe_dir: i32,
    pub air_frames: u32,
    pub synced_frames: u32,
    pub last_yaw: f32,

    pub last_lj: Option<LongjumpStats>,

    pub speed_history: Vec<f32>,
    pub last_buttons: u16,
}

pub static MOVEMENT_STATE: Mutex<MovementState> = Mutex::new(MovementState {
    vel_2d: 0.0,
    vel_3d: 0.0,
    vel_z: 0.0,
    speed_diff: 0.0,
    is_onground: true,
    key_w: false,
    key_a: false,
    key_s: false,
    key_d: false,
    key_duck: false,
    key_jump: false,
    last_cmd_forward: 0.0,
    last_cmd_side: 0.0,
    in_air: false,
    jump_origin: [0.0; 3],
    takeoff_speed: 0.0,
    takeoff_time: 0.0,
    max_air_speed: 0.0,
    strafe_count: 0,
    current_strafe_dir: 0,
    air_frames: 0,
    synced_frames: 0,
    last_yaw: 0.0,
    last_lj: None,
    speed_history: Vec::new(),
    last_buttons: 0,
});

fn catmull_rom(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

pub unsafe fn update_user_cmd(cmd_ptr: *const std::ffi::c_void) {
    if let Some(cmd) = (cmd_ptr as *const UserCmd).as_ref() {
        if let Ok(mut state) = MOVEMENT_STATE.lock() {
            state.key_w = cmd.forwardmove > 0.0 || (cmd.buttons & IN_FORWARD) != 0;
            state.key_s = cmd.forwardmove < 0.0 || (cmd.buttons & IN_BACK) != 0;
            state.key_a = cmd.sidemove < 0.0
                || (cmd.buttons & IN_MOVELEFT) != 0
                || (cmd.buttons & IN_LEFT) != 0;
            state.key_d = cmd.sidemove > 0.0
                || (cmd.buttons & IN_MOVERIGHT) != 0
                || (cmd.buttons & IN_RIGHT) != 0;
            state.key_duck = (cmd.buttons & IN_DUCK) != 0 || cmd.upmove < 0.0;
            state.key_jump = (cmd.buttons & IN_JUMP) != 0 || cmd.upmove > 0.0;
            state.last_cmd_forward = cmd.forwardmove;
            state.last_cmd_side = cmd.sidemove;
            state.last_buttons = cmd.buttons;
        }
    }
}

pub fn get_last_buttons() -> u16 {
    MOVEMENT_STATE.lock().map_or(0, |state| state.last_buttons)
}

pub unsafe fn update_movement_physics(params: &RefParams) {
    let mut state = match MOVEMENT_STATE.lock() {
        Ok(s) => s,
        Err(_) => return,
    };

    let [vx, vy, vz] = params.simvel;

    let current_speed_2d = (vx * vx + vy * vy).sqrt();
    let current_speed_3d = (vx * vx + vy * vy + vz * vz).sqrt();

    state.speed_diff = current_speed_2d - state.vel_2d;
    state.vel_2d = current_speed_2d;
    state.vel_3d = current_speed_3d;
    state.vel_z = vz;

    state.speed_history.push(current_speed_2d);
    if state.speed_history.len() > 90 {
        let overflow = state.speed_history.len() - 90;
        state.speed_history.drain(..overflow);
    }

    let curr_onground = params.onground != 0;

    if params.paused != 0 || params.intermission != 0 {
        state.is_onground = curr_onground;
        return;
    }

    if state.is_onground && !curr_onground {
        state.in_air = true;
        state.takeoff_speed = current_speed_2d;
        state.takeoff_time = params.time;
        state.jump_origin = params.simorg;
        state.max_air_speed = current_speed_2d;
        state.strafe_count = 0;
        state.current_strafe_dir = 0;
        state.air_frames = 0;
        state.synced_frames = 0;
        state.last_yaw = params.cl_viewangles[1];
    } else if !curr_onground && state.in_air {
        state.max_air_speed = state.max_air_speed.max(current_speed_2d);

        let yaw = params.cl_viewangles[1];
        let yaw_diff = normalize_angle(yaw - state.last_yaw);

        let turning_left = yaw_diff > 0.0001;
        let turning_right = yaw_diff < -0.0001;
        let holding_left = state.key_a || state.last_cmd_side < 0.0;
        let holding_right = state.key_d || state.last_cmd_side > 0.0;

        if turning_left || turning_right {
            state.air_frames += 1;
            if (turning_left && holding_left) || (turning_right && holding_right) {
                state.synced_frames += 1;
            }
        }

        let active_dir = if holding_left || turning_left {
            -1
        } else if holding_right || turning_right {
            1
        } else {
            0
        };

        if active_dir != 0 && active_dir != state.current_strafe_dir {
            state.strafe_count += 1;
            state.current_strafe_dir = active_dir;
        }

        state.last_yaw = yaw;
    } else if !state.is_onground && curr_onground && state.in_air {
        state.in_air = false;
        let air_time = params.time - state.takeoff_time;

        let dx = params.simorg[0] - state.jump_origin[0];
        let dy = params.simorg[1] - state.jump_origin[1];
        let dz = params.simorg[2] - state.jump_origin[2];
        let dist_2d = (dx * dx + dy * dy).sqrt();

        let lj_dist = dist_2d + 32.0;
        let sync_pct = if state.air_frames > 0 {
            (state.synced_frames as f32 / state.air_frames as f32) * 100.0
        } else {
            0.0
        };

        if air_time >= 0.2 && lj_dist > 230.0 && dz.abs() <= 64.0 {
            let stats = LongjumpStats {
                distance: lj_dist,
                prestrafe: state.takeoff_speed,
                max_speed: state.max_air_speed,
                strafes: state.strafe_count,
                sync_pct,
                height_delta: dz,
                air_time,
                timestamp: params.time,
            };

            if stats.distance >= 240.0 && MOVEMENT_GUI_ENABLE.value() != 0.0 {
                if let Ok(sound_path) = CString::new("buttons/bell1.wav") {
                    if !G_ENGFUNCS.pfnPlaySoundByName.is_null() {
                        let pfn: unsafe extern "C" fn(*const c_char, f32) =
                            std::mem::transmute(G_ENGFUNCS.pfnPlaySoundByName);
                        pfn(sound_path.as_ptr(), 1.0);
                    }
                }
            }

            crate::con_println!(
                "[CSLDR LJ] Dist: {:.2} u | Prestrafe: {:.2} u/s | MaxSpeed: {:.2} u/s | Sync: {:.1}%",
                stats.distance, stats.prestrafe, stats.max_speed, stats.sync_pct
            );

            state.last_lj = Some(stats);
        }
    }

    state.is_onground = curr_onground;
}

#[inline]
unsafe fn get_text_width(text: &str) -> i32 {
    get_text_width_f32(text) as i32
}

pub unsafe fn draw_movement_gui() {
    if MOVEMENT_GUI_ENABLE.value() == 0.0 || !CAN_OPENGL.load(Ordering::Relaxed) {
        return;
    }

    let show_speed = MOVEMENT_GUI_SPEED.value() != 0.0;
    let show_lj = MOVEMENT_GUI_LJ.value() != 0.0;
    let show_graph = MOVEMENT_GUI_GRAPH.value() != 0.0;

    if !show_speed && !show_lj && !show_graph {
        return;
    }

    let screen_w = SCREEN_WIDTH.load(Ordering::Relaxed);
    let screen_h = SCREEN_HEIGHT.load(Ordering::Relaxed);

    let pos_x = MOVEMENT_GUI_POS_X.value().clamp(0.05, 0.95);
    let pos_y = MOVEMENT_GUI_POS_Y.value().clamp(0.05, 0.95);

    let center_x = (screen_w as f32 * pos_x) as i32;
    let start_y = (screen_h as f32 * pos_y) as i32;

    let state = match MOVEMENT_STATE.lock() {
        Ok(s) => s,
        Err(_) => return,
    };

    let client_time = crate::engine::cdll_int::CLIENT_TIME
        .lock()
        .map_or(0.0, |t| *t);

    struct RenderLine {
        text: String,
        r: f32,
        g: f32,
        b: f32,
        width: i32,
    }

    let mut lines: Vec<RenderLine> = Vec::with_capacity(2);

    if show_speed {
        let speed_str = format!("{:.2} u/s", state.vel_2d);
        let width = get_text_width(&speed_str);
        let (r, g, b) = if state.speed_diff > 0.1 {
            (0.2, 1.0, 0.4)
        } else if state.speed_diff < -0.1 {
            (1.0, 0.3, 0.2)
        } else {
            (
                MOVEMENT_GUI_COLOR_R.value(),
                MOVEMENT_GUI_COLOR_G.value(),
                MOVEMENT_GUI_COLOR_B.value(),
            )
        };
        lines.push(RenderLine {
            text: speed_str,
            r,
            g,
            b,
            width,
        });
    }

    if show_lj {
        if state.in_air {
            let sync = if state.air_frames > 0 {
                (state.synced_frames as f32 / state.air_frames as f32) * 100.0
            } else {
                0.0
            };
            let live_str = format!(
                "Air: {:.1} | Pre: {:.1} | Sync: {:.1}%",
                state.vel_2d, state.takeoff_speed, sync
            );
            let width = get_text_width(&live_str);
            lines.push(RenderLine {
                text: live_str,
                r: 1.0,
                g: 0.8,
                b: 0.2,
                width,
            });
        } else if let Some(ref lj) = state.last_lj {
            if lj.distance > 230.0 && client_time - lj.timestamp < 10.0 {
                let lj_str = format!(
                    "LJ: {:.2} u | Pre: {:.2} | Sync: {:.1}%",
                    lj.distance, lj.prestrafe, lj.sync_pct
                );
                let width = get_text_width(&lj_str);
                let (r, g, b) = if lj.distance >= 250.0 {
                    (1.0, 0.85, 0.1)
                } else if lj.distance >= 240.0 {
                    (0.2, 0.9, 1.0)
                } else {
                    (0.4, 1.0, 0.4)
                };
                lines.push(RenderLine {
                    text: lj_str,
                    r,
                    g,
                    b,
                    width,
                });
            }
        }
    }

    let graph_w = MOVEMENT_GUI_GRAPH_W.value().clamp(100.0, 400.0);
    let graph_h = MOVEMENT_GUI_GRAPH_H.value().clamp(20.0, 120.0);

    let max_text_w = lines.iter().map(|l| l.width).max().unwrap_or(0);
    let needed_w = if show_graph {
        max_text_w.max(graph_w as i32)
    } else {
        max_text_w
    };

    if needed_w == 0 && !show_graph {
        return;
    }

    let line_height = 18;
    let padding_x = 12.0;
    let padding_y = 6.0;

    let panel_w = needed_w as f32 + padding_x * 2.0;
    let text_total_h = (lines.len() as i32 * line_height) as f32;
    let graph_total_h = if show_graph { graph_h + 8.0 } else { 0.0 };
    let panel_h = text_total_h + graph_total_h + padding_y * 2.0;

    if MOVEMENT_GUI_BG.value() != 0.0 {
        let bg_x = center_x as f32 - panel_w / 2.0;
        let bg_y = start_y as f32 - padding_y;
        draw_rect(bg_x, bg_y, panel_w, panel_h, 0.0, 0.0, 0.0, 0.6);
    }

    let mut current_y = start_y;
    for line in &lines {
        let x = center_x - line.width / 2;
        draw_text(x, current_y, &line.text, line.r, line.g, line.b);
        current_y += line_height;
    }

    if show_graph && state.speed_history.len() >= 2 {
        let gx = center_x as f32 - graph_w / 2.0;
        let gy = current_y as f32 + 4.0;

        draw_rect(gx, gy, graph_w, graph_h, 0.0, 0.0, 0.0, 0.2);

        gl_disable(GL_TEXTURE_2D);
        gl_disable(GL_DEPTH_TEST);
        gl_enable(GL_BLEND);
        gl_blend_func(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
        gl_color4f(1.0, 1.0, 1.0, 0.08);
        gl_begin(GL_LINE_LOOP);
        gl_vertex2f(gx, gy);
        gl_vertex2f(gx + graph_w, gy);
        gl_vertex2f(gx + graph_w, gy + graph_h);
        gl_vertex2f(gx, gy + graph_h);
        gl_end();

        let samples = &state.speed_history;
        let n = samples.len();

        let mut min_val = f32::MAX;
        let mut max_val = f32::MIN;
        for &s in samples {
            if s < min_val {
                min_val = s;
            }
            if s > max_val {
                max_val = s;
            }
        }

        min_val = min_val.min(200.0);
        max_val = max_val.max(270.0);
        let val_range = (max_val - min_val).max(10.0);

        if (min_val..=max_val).contains(&250.0) {
            let ref_y =
                (gy + graph_h - ((250.0 - min_val) / val_range) * graph_h).clamp(gy, gy + graph_h);
            draw_rect(gx, ref_y, graph_w, 1.0, 0.3, 0.6, 1.0, 0.4);
        }

        let sub_steps = 4;
        let num_curve_points = (n - 1) * sub_steps + 1;
        let mut curve_points: Vec<(f32, f32, f32)> = Vec::with_capacity(num_curve_points);

        for i in 0..(n - 1) {
            let p0 = if i > 0 { samples[i - 1] } else { samples[i] };
            let p1 = samples[i];
            let p2 = samples[i + 1];
            let p3 = if i + 2 < n { samples[i + 2] } else { p2 };

            for s in 0..sub_steps {
                let t = s as f32 / sub_steps as f32;
                let val = catmull_rom(p0, p1, p2, p3, t);

                let frac_x = (i as f32 + t) / ((n - 1) as f32);
                let x = gx + frac_x * graph_w;
                let raw_y = gy + graph_h - ((val - min_val) / val_range) * graph_h;
                let y = raw_y.clamp(gy, gy + graph_h);

                let delta = if i > 0 {
                    samples[i] - samples[i - 1]
                } else {
                    0.0
                };

                curve_points.push((x, y, delta));
            }
        }

        let last_val = samples[n - 1];
        let last_x = gx + graph_w;
        let raw_last_y = gy + graph_h - ((last_val - min_val) / val_range) * graph_h;
        let last_y = raw_last_y.clamp(gy, gy + graph_h);
        let last_delta = samples[n - 1] - samples[n - 2];
        curve_points.push((last_x, last_y, last_delta));

        gl_disable(GL_TEXTURE_2D);
        gl_disable(GL_DEPTH_TEST);
        gl_enable(GL_BLEND);
        gl_blend_func(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
        gl_enable(GL_LINE_SMOOTH);

        gl_begin(GL_TRIANGLE_STRIP);
        for &(px, py, delta) in &curve_points {
            let (r, g, b) = if delta > 0.1 {
                (0.2, 1.0, 0.4)
            } else if delta < -0.1 {
                (1.0, 0.3, 0.2)
            } else {
                (0.2, 0.9, 1.0)
            };
            gl_color4f(r, g, b, 0.25);
            gl_vertex2f(px, py);
            gl_color4f(r, g, b, 0.0);
            gl_vertex2f(px, gy + graph_h);
        }
        gl_end();

        gl_begin(GL_LINE_STRIP);
        for &(px, py, delta) in &curve_points {
            let (r, g, b) = if delta > 0.1 {
                (0.2, 1.0, 0.4)
            } else if delta < -0.1 {
                (1.0, 0.3, 0.2)
            } else {
                (0.2, 0.9, 1.0)
            };
            gl_color4f(r, g, b, 0.95);
            gl_vertex2f(px, py);
        }
        gl_end();

        gl_disable(GL_LINE_SMOOTH);
        gl_disable(GL_BLEND);
        gl_enable(GL_TEXTURE_2D);
    }
}
