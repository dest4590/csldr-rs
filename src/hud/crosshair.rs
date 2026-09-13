use crate::engine::cdll_int::{G_ENGFUNCS, SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::engine::proxy::CL_FUNCS;
use crate::game::state::USER1;
use crate::hud::fov::FOV_VALUE_ORIGINAL;
use crate::render::gl::*;
use crate::render::studiorenderer::CAN_OPENGL;
use crate::utils::cvars::*;
use crate::utils::msg::MsgRead;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;

pub static CURRENT_WEAPON_ID: AtomicI32 = AtomicI32::new(0);
pub static HIDE_HUD_FLAGS: AtomicI32 = AtomicI32::new(0);
pub static NIGHT_VISION_ON: AtomicI32 = AtomicI32::new(0);

pub static XHAIR_PLAYER_FLAGS: AtomicI32 = AtomicI32::new(0);
pub static XHAIR_PLAYER_SPEED: Mutex<f32> = Mutex::new(0.0);
pub static XHAIR_WEAPON_FLAGS: AtomicI32 = AtomicI32::new(0);
pub static XHAIR_SHOTS_FIRED: AtomicI32 = AtomicI32::new(0);

pub const HIDEHUD_WEAPONS: i32 = 1 << 0;
pub const HIDEHUD_ALL: i32 = 1 << 2;
pub const HIDEHUD_CROSSHAIR: i32 = 1 << 6;

pub const FL_ONGROUND: i32 = 0x200;
pub const FL_DUCKING: i32 = 0x4000;

pub const ACCURACY_NONE: i32 = 0;
pub const ACCURACY_JUMP: i32 = 1 << 0;
pub const ACCURACY_RUN: i32 = 1 << 1;
pub const ACCURACY_DUCK: i32 = 1 << 2;
pub const ACCURACY_INACCURATE: i32 = 1 << 3;
pub const ACCURACY_VERY_INACCURATE: i32 = 1 << 4;

pub const WEAPON_NONE: i32 = 0;
pub const WEAPON_P228: i32 = 1;
pub const WEAPON_GLOCK: i32 = 2;
pub const WEAPON_SCOUT: i32 = 3;
pub const WEAPON_HEGRENADE: i32 = 4;
pub const WEAPON_XM1014: i32 = 5;
pub const WEAPON_C4: i32 = 6;
pub const WEAPON_MAC10: i32 = 7;
pub const WEAPON_AUG: i32 = 8;
pub const WEAPON_SMOKEGRENADE: i32 = 9;
pub const WEAPON_ELITE: i32 = 10;
pub const WEAPON_FIVESEVEN: i32 = 11;
pub const WEAPON_UMP45: i32 = 12;
pub const WEAPON_SG550: i32 = 13;
pub const WEAPON_GALIL: i32 = 14;
pub const WEAPON_FAMAS: i32 = 15;
pub const WEAPON_USP: i32 = 16;
pub const WEAPON_GLOCK18: i32 = 17;
pub const WEAPON_AWP: i32 = 18;
pub const WEAPON_MP5N: i32 = 19;
pub const WEAPON_M249: i32 = 20;
pub const WEAPON_M3: i32 = 21;
pub const WEAPON_M4A1: i32 = 22;
pub const WEAPON_TMP: i32 = 23;
pub const WEAPON_G3SG1: i32 = 24;
pub const WEAPON_FLASHBANG: i32 = 25;
pub const WEAPON_DEAGLE: i32 = 26;
pub const WEAPON_SG552: i32 = 27;
pub const WEAPON_AK47: i32 = 28;
pub const WEAPON_KNIFE: i32 = 29;
pub const WEAPON_P90: i32 = 30;

struct Rect {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

static XHAIR_GAP_VAL: Mutex<f32> = Mutex::new(0.0);
static LAST_SHOTS_FIRED: AtomicI32 = AtomicI32::new(0);
static XHAIR_PREV_TIME: Mutex<f32> = Mutex::new(0.0);

pub fn hk_msg_func_cur_weapon(pbuf: &[u8]) -> i32 {
    let mut read = MsgRead::new(pbuf);
    if let (Some(state), Some(weapon_id)) = (read.read_byte(), read.read_char()) {
        if state != 0 {
            if weapon_id > WEAPON_NONE as i8 {
                CURRENT_WEAPON_ID.store(weapon_id as i32, Ordering::Relaxed);
            } else {
                CURRENT_WEAPON_ID.store(WEAPON_NONE, Ordering::Relaxed);
            }
        }
    }
    0
}

pub fn hk_msg_func_hide_weapon(pbuf: &[u8]) -> i32 {
    let mut read = MsgRead::new(pbuf);
    if let Some(flags) = read.read_byte() {
        HIDE_HUD_FLAGS.store(flags as i32, Ordering::Relaxed);
    }
    0
}

pub fn hk_msg_func_nvg_toggle(pbuf: &[u8]) -> i32 {
    let mut read = MsgRead::new(pbuf);
    if let Some(nvg) = read.read_byte() {
        NIGHT_VISION_ON.store(nvg as i32, Ordering::Relaxed);
    }
    0
}

pub fn hud_vid_init() {
    CURRENT_WEAPON_ID.store(WEAPON_NONE, Ordering::Relaxed);
    HIDE_HUD_FLAGS.store(0, Ordering::Relaxed);
    NIGHT_VISION_ON.store(0, Ordering::Relaxed);
}

fn scale_for_res(value: f32, height: i32) -> f32 {
    (value * (height as f32 / 480.0)).round()
}

unsafe fn draw_crosshair_section(x0: f32, y0: f32, x1: f32, y1: f32) {
    gl_color4f(
        XHAIR_COLOR_R.value(),
        XHAIR_COLOR_G.value(),
        XHAIR_COLOR_B.value(),
        XHAIR_ALPHA.value(),
    );
    gl_begin(GL_TRIANGLE_STRIP);
    gl_vertex2f(x0, y0);
    gl_vertex2f(x0, y1);
    gl_vertex2f(x1, y0);
    gl_vertex2f(x1, y1);
    gl_end();
}

unsafe fn draw_crosshair_padding(pad: f32, x0: f32, y0: f32, x1: f32, y1: f32) {
    gl_color4f(0.0, 0.0, 0.0, XHAIR_ALPHA.value());
    gl_begin(GL_TRIANGLE_STRIP);
    gl_vertex2f(x0, y1);
    gl_vertex2f(x1 + pad, y1 + pad);
    gl_vertex2f(x1, y1);
    gl_vertex2f(x1 + pad, y0 - pad);
    gl_vertex2f(x1, y0);
    gl_vertex2f(x0 - pad, y0 - pad);
    gl_vertex2f(x0, y0);
    gl_vertex2f(x0 - pad, y1 + pad);
    gl_vertex2f(x0, y1);
    gl_vertex2f(x1 + pad, y1 + pad);
    gl_end();
}

fn get_weapon_accuracy_flags(weapon_id: i32) -> i32 {
    let weapon_flags = XHAIR_WEAPON_FLAGS.load(Ordering::Relaxed);

    match weapon_id {
        WEAPON_P228 | WEAPON_FIVESEVEN | WEAPON_DEAGLE => {
            ACCURACY_DUCK | ACCURACY_RUN | ACCURACY_JUMP
        }
        WEAPON_MAC10 | WEAPON_UMP45 | WEAPON_MP5N | WEAPON_TMP => ACCURACY_JUMP,
        WEAPON_AUG | WEAPON_GALIL | WEAPON_M249 | WEAPON_SG552 | WEAPON_AK47 | WEAPON_P90 => {
            ACCURACY_RUN | ACCURACY_JUMP
        }
        WEAPON_FAMAS => {
            if (weapon_flags & 16) != 0 {
                ACCURACY_VERY_INACCURATE | ACCURACY_RUN | ACCURACY_JUMP
            } else {
                ACCURACY_RUN | ACCURACY_JUMP
            }
        }
        WEAPON_USP => {
            if (weapon_flags & 1) != 0 {
                ACCURACY_INACCURATE | ACCURACY_DUCK | ACCURACY_RUN | ACCURACY_JUMP
            } else {
                ACCURACY_DUCK | ACCURACY_RUN | ACCURACY_JUMP
            }
        }
        WEAPON_GLOCK18 => {
            if (weapon_flags & 2) != 0 {
                ACCURACY_VERY_INACCURATE | ACCURACY_DUCK | ACCURACY_RUN | ACCURACY_JUMP
            } else {
                ACCURACY_DUCK | ACCURACY_RUN | ACCURACY_JUMP
            }
        }
        WEAPON_M4A1 => {
            if (weapon_flags & 4) != 0 {
                ACCURACY_INACCURATE | ACCURACY_RUN | ACCURACY_JUMP
            } else {
                ACCURACY_RUN | ACCURACY_JUMP
            }
        }
        _ => ACCURACY_NONE,
    }
}

pub fn get_crosshair_gap(weapon_id: i32) -> f32 {
    let client_time = match crate::cdll_int::CLIENT_TIME.lock() {
        Ok(t) => *t,
        Err(_) => 0.0,
    };

    let (mut min_gap, delta_gap) = match weapon_id {
        WEAPON_P228 | WEAPON_HEGRENADE | WEAPON_SMOKEGRENADE | WEAPON_FIVESEVEN | WEAPON_USP
        | WEAPON_GLOCK18 | WEAPON_AWP | WEAPON_FLASHBANG | WEAPON_DEAGLE => (8.0, 3.0),

        WEAPON_SCOUT | WEAPON_SG550 | WEAPON_SG552 => (5.0, 3.0),
        WEAPON_XM1014 => (9.0, 4.0),
        WEAPON_C4 | WEAPON_UMP45 | WEAPON_M249 => (6.0, 3.0),
        WEAPON_MAC10 => (9.0, 3.0),
        WEAPON_AUG => (3.0, 3.0),
        WEAPON_MP5N => (6.0, 2.0),
        WEAPON_M3 => (8.0, 6.0),
        WEAPON_TMP | WEAPON_KNIFE | WEAPON_P90 => (7.0, 3.0),
        WEAPON_G3SG1 => (6.0, 4.0),
        WEAPON_AK47 => (4.0, 4.0),
        _ => (4.0, 3.0),
    };

    if XHAIR_GAP_USEWEAPONVALUE.value() == 0.0 {
        min_gap = 4.0;
    }

    let base_min_gap = min_gap;
    let abs_min_gap = base_min_gap * 0.5;

    let flags = get_weapon_accuracy_flags(weapon_id);
    let player_flags = XHAIR_PLAYER_FLAGS.load(Ordering::Relaxed);
    let player_speed = match XHAIR_PLAYER_SPEED.lock() {
        Ok(s) => *s,
        Err(_) => 0.0,
    };

    if XHAIR_DYNAMIC_MOVE.value() != 0.0 && flags != 0 {
        if (player_flags & FL_ONGROUND) == 0 && (flags & ACCURACY_JUMP) != 0 {
            min_gap *= 2.0;
        } else if (player_flags & FL_DUCKING) != 0 && (flags & ACCURACY_DUCK) != 0 {
            min_gap *= 0.5;
        } else {
            let run_limit = match weapon_id {
                WEAPON_AUG | WEAPON_GALIL | WEAPON_FAMAS | WEAPON_M249 | WEAPON_M4A1
                | WEAPON_SG552 | WEAPON_AK47 => 140.0,
                WEAPON_P90 => 170.0,
                _ => 0.0,
            };

            if player_speed > run_limit && (flags & ACCURACY_RUN) != 0 {
                min_gap *= 1.5;
            }
        }

        if (flags & ACCURACY_INACCURATE) != 0 {
            min_gap *= 1.4;
        }

        if (flags & ACCURACY_VERY_INACCURATE) != 0 {
            min_gap *= 1.4;
        }

        min_gap = base_min_gap + (min_gap - base_min_gap) * XHAIR_DYNAMIC_SCALE.value();
        min_gap = min_gap.max(abs_min_gap);
    }

    let mut prev_time = XHAIR_PREV_TIME.lock().unwrap();
    if *prev_time > client_time {
        *prev_time = client_time;
    }

    let delta_time = client_time - *prev_time;
    *prev_time = client_time;

    let shots_fired = XHAIR_SHOTS_FIRED.load(Ordering::Relaxed);
    let last_shots = LAST_SHOTS_FIRED.load(Ordering::Relaxed);
    let mut gap_val = XHAIR_GAP_VAL.lock().unwrap();

    if shots_fired <= last_shots {
        *gap_val -= (100.0 * delta_time) * (0.013 * *gap_val + 0.1);
    } else {
        *gap_val += delta_gap * XHAIR_DYNAMIC_SCALE.value();
        *gap_val = gap_val.min(15.0);
    }

    if shots_fired > 600 {
        XHAIR_SHOTS_FIRED.store(1, Ordering::Relaxed);
    }
    LAST_SHOTS_FIRED.store(shots_fired, Ordering::Relaxed);

    *gap_val = gap_val.max(min_gap);

    *gap_val + XHAIR_GAP.value()
}

pub unsafe fn draw_crosshair(weapon_id: i32) {
    let screen_w = SCREEN_WIDTH.load(Ordering::Relaxed);
    let screen_h = SCREEN_HEIGHT.load(Ordering::Relaxed);

    let center_x = screen_w as f32 / 2.0;
    let center_y = screen_h as f32 / 2.0;

    let gap = scale_for_res(get_crosshair_gap(weapon_id), screen_h);
    let length = scale_for_res(XHAIR_SIZE.value(), screen_h);
    let mut thickness = scale_for_res(XHAIR_THICK.value(), screen_h);
    thickness = thickness.max(1.0);

    let inner = Rect {
        left: center_x - gap - thickness / 2.0,
        right: center_x - gap - thickness / 2.0 + 2.0 * gap + thickness,
        top: center_y - gap - thickness / 2.0,
        bottom: center_y - gap - thickness / 2.0 + 2.0 * gap + thickness,
    };

    let outer = Rect {
        left: inner.left - length,
        right: inner.right + length,
        top: inner.top - length,
        bottom: inner.bottom + length,
    };

    let y0 = center_y - thickness / 2.0;
    let x0 = center_x - thickness / 2.0;
    let y1 = y0 + thickness;
    let x1 = x0 + thickness;

    gl_disable(GL_TEXTURE_2D);
    gl_enable(GL_BLEND);

    if XHAIR_ADDITIVE.value() != 0.0 {
        gl_blend_func(GL_ONE, GL_ONE);
    } else {
        gl_blend_func(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
    }

    if XHAIR_DOT.value() != 0.0 {
        draw_crosshair_section(x0, y0, x1, y1);
    }

    if XHAIR_T.value() == 0.0 {
        draw_crosshair_section(x0, outer.top, x1, inner.top);
    }

    draw_crosshair_section(x0, inner.bottom, x1, outer.bottom);
    draw_crosshair_section(outer.left, y0, inner.left, y1);
    draw_crosshair_section(inner.right, y0, outer.right, y1);

    if XHAIR_ADDITIVE.value() != 0.0 {
        gl_blend_func(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
    }

    if XHAIR_PAD.value() != 0.0 {
        let pad = XHAIR_PAD.value();

        if XHAIR_DOT.value() != 0.0 {
            draw_crosshair_padding(pad, x0, y0, x1, y1);
        }

        if XHAIR_T.value() == 0.0 {
            draw_crosshair_padding(pad, x0, outer.top, x1, inner.top);
        }

        draw_crosshair_padding(pad, x0, inner.bottom, x1, outer.bottom);
        draw_crosshair_padding(pad, outer.left, y0, inner.left, y1);
        draw_crosshair_padding(pad, inner.right, y0, outer.right, y1);
    }

    gl_color4f(1.0, 1.0, 1.0, 1.0);
    gl_blend_func(GL_SRC_ALPHA, GL_ONE);
    gl_disable(GL_BLEND);
    gl_enable(GL_TEXTURE_2D);
}

pub unsafe fn should_draw_custom_crosshair(intermission: i32, weapon_id: &mut i32) -> bool {
    if !CAN_OPENGL.load(Ordering::Relaxed) {
        return false;
    }

    if CL_CROSSHAIR_COLOR.get_ptr().is_null() || CL_CROSSHAIR_TRANSLUCENT.get_ptr().is_null() {
        return false;
    }

    if XHAIR_ENABLE.value() == 0.0 {
        return false;
    }

    if !HUD_DRAW.get_ptr().is_null() && HUD_DRAW.value() == 0.0 {
        return false;
    }

    if let Some(is_spec) = G_ENGFUNCS.IsSpectateOnly {
        if is_spec() != 0 {
            let user1 = USER1.load(Ordering::Relaxed);
            if user1 != 4 {
                return false;
            }

            let orig_fov = FOV_VALUE_ORIGINAL.load(Ordering::Relaxed);
            if orig_fov <= 44 {
                return false;
            }

            *weapon_id = 0;
            return true;
        }
    }

    if intermission != 0 {
        return false;
    }

    let user1 = USER1.load(Ordering::Relaxed);
    let hide_flags = HIDE_HUD_FLAGS.load(Ordering::Relaxed);
    if user1 != 4 && (hide_flags & (HIDEHUD_WEAPONS | HIDEHUD_ALL)) != 0 {
        return false;
    }

    if (hide_flags & HIDEHUD_CROSSHAIR) != 0 {
        return false;
    }

    let orig_fov = FOV_VALUE_ORIGINAL.load(Ordering::Relaxed);
    if orig_fov <= 54 {
        return false;
    }

    !matches!(
        *weapon_id,
        WEAPON_SCOUT | WEAPON_SG550 | WEAPON_AWP | WEAPON_G3SG1
    )
}

pub unsafe fn hk_hud_redraw(time: f32, intermission: i32) -> i32 {
    let mut weapon_id = CURRENT_WEAPON_ID.load(Ordering::Relaxed);
    let draw_custom = should_draw_custom_crosshair(intermission, &mut weapon_id);

    let res = if draw_custom {
        let color_ptr = CL_CROSSHAIR_COLOR.get_ptr();
        let trans_ptr = CL_CROSSHAIR_TRANSLUCENT.get_ptr();

        if !color_ptr.is_null() && !trans_ptr.is_null() {
            let str_ptr = (*color_ptr).string as *mut u8;
            let old_trans = (*trans_ptr).value;
            let mut old_color = [0u8; 2];
            if !str_ptr.is_null() {
                old_color[0] = *str_ptr;
                old_color[1] = *str_ptr.add(1);
                *str_ptr = b'0';
                *str_ptr.add(1) = 0;
            }

            (*trans_ptr).value = 1.0;

            let ret = if let Some(f) = CL_FUNCS.pHudRedrawFunc {
                f(time, intermission)
            } else {
                1
            };

            (*trans_ptr).value = old_trans;
            if !str_ptr.is_null() {
                *str_ptr = old_color[0];
                *str_ptr.add(1) = old_color[1];
            }

            draw_crosshair(weapon_id);
            ret
        } else {
            if let Some(f) = CL_FUNCS.pHudRedrawFunc {
                f(time, intermission)
            } else {
                1
            }
        }
    } else {
        if let Some(f) = CL_FUNCS.pHudRedrawFunc {
            f(time, intermission)
        } else {
            1
        }
    };

    crate::hud::motion_blur::draw_motion_blur();
    crate::hud::movement_gui::draw_movement_gui();
    crate::hud::acoustics_gui::draw_acoustics_gui();

    res
}
