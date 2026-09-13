use std::sync::atomic::Ordering;
use std::sync::Mutex;

use crate::engine::cdll_int::{CLIENT_TIME, G_ENGFUNCS, SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::render::gl::*;
use crate::render::studiorenderer::CAN_OPENGL;
use crate::utils::cvars::*;
use crate::utils::maths::normalize_angle;

struct BlurState {
    texture_id: u32,
    tex_w: i32,
    tex_h: i32,
    prev_pitch: f32,
    prev_yaw: f32,
    prev_time: f32,
    smoothed_magnitude: f32,
    has_valid_texture: bool,
}

static BLUR_STATE: Mutex<BlurState> = Mutex::new(BlurState {
    texture_id: 0,
    tex_w: 0,
    tex_h: 0,
    prev_pitch: 0.0,
    prev_yaw: 0.0,
    prev_time: 0.0,
    smoothed_magnitude: 0.0,
    has_valid_texture: false,
});

pub unsafe fn draw_motion_blur() {
    if MOTION_BLUR_ENABLE.value() == 0.0 || !CAN_OPENGL.load(Ordering::Relaxed) {
        return;
    }

    let sw = SCREEN_WIDTH.load(Ordering::Relaxed);
    let sh = SCREEN_HEIGHT.load(Ordering::Relaxed);
    if sw <= 0 || sh <= 0 {
        return;
    }

    let client_time = match CLIENT_TIME.lock() {
        Ok(t) => *t,
        Err(_) => return,
    };

    let mut view_angles = [0.0f32; 3];
    if !G_ENGFUNCS.GetViewAngles.is_null() {
        let get_viewangles: unsafe extern "C" fn(*mut f32) =
            std::mem::transmute(G_ENGFUNCS.GetViewAngles);
        get_viewangles(view_angles.as_mut_ptr());
    }

    let [pitch, yaw, _] = view_angles;

    let mut state = match BLUR_STATE.lock() {
        Ok(s) => s,
        Err(_) => return,
    };

    if state.texture_id == 0 || state.tex_w != sw || state.tex_h != sh {
        if state.texture_id != 0 {
            gl_delete_textures(1, &state.texture_id);
            state.texture_id = 0;
        }

        let mut tex = 0u32;
        gl_gen_textures(1, &mut tex);
        gl_bind_texture(GL_TEXTURE_2D, tex);
        gl_tex_parameter_i(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR as i32);
        gl_tex_parameter_i(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as i32);
        gl_tex_parameter_i(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE as i32);
        gl_tex_parameter_i(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE as i32);
        gl_tex_image_2d(
            GL_TEXTURE_2D,
            0,
            GL_RGBA8 as i32,
            sw,
            sh,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            std::ptr::null(),
        );

        state.texture_id = tex;
        state.tex_w = sw;
        state.tex_h = sh;
        state.has_valid_texture = false;
        state.prev_pitch = pitch;
        state.prev_yaw = yaw;
        state.prev_time = client_time;
        state.smoothed_magnitude = 0.0;
        return;
    }

    let dt = (client_time - state.prev_time).max(0.001);
    if state.prev_time == 0.0 || dt > 0.2 {
        state.prev_pitch = pitch;
        state.prev_yaw = yaw;
        state.prev_time = client_time;
        return;
    }

    let delta_yaw = normalize_angle(yaw - state.prev_yaw);
    let delta_pitch = pitch - state.prev_pitch;

    state.prev_pitch = pitch;
    state.prev_yaw = yaw;
    state.prev_time = client_time;

    let intensity = MOTION_BLUR_INTENSITY.value();
    let speed = ((delta_yaw * delta_yaw + delta_pitch * delta_pitch).sqrt() / dt) * intensity;

    state.smoothed_magnitude = state.smoothed_magnitude * 0.65 + speed * 0.35;

    if state.smoothed_magnitude < 25.0 {
        state.smoothed_magnitude = 0.0;
    }

    let factor = ((state.smoothed_magnitude / 700.0) * 0.38).clamp(0.0, 0.38);

    if factor > 0.03 && state.has_valid_texture {
        gl_disable(GL_DEPTH_TEST);
        gl_enable(GL_BLEND);
        gl_blend_func(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
        gl_enable(GL_TEXTURE_2D);
        gl_bind_texture(GL_TEXTURE_2D, state.texture_id);

        gl_color4f(1.0, 1.0, 1.0, factor);

        gl_matrix_mode(GL_PROJECTION);
        gl_push_matrix();
        gl_load_identity();
        gl_ortho(0.0, sw as f64, sh as f64, 0.0, -1.0, 1.0);

        gl_matrix_mode(GL_MODELVIEW);
        gl_push_matrix();
        gl_load_identity();

        gl_begin(GL_QUADS);
        gl_tex_coord2f(0.0, 0.0);
        gl_vertex2f(0.0, sh as f32);
        gl_tex_coord2f(1.0, 0.0);
        gl_vertex2f(sw as f32, sh as f32);
        gl_tex_coord2f(1.0, 1.0);
        gl_vertex2f(sw as f32, 0.0);
        gl_tex_coord2f(0.0, 1.0);
        gl_vertex2f(0.0, 0.0);
        gl_end();

        gl_pop_matrix();
        gl_matrix_mode(GL_PROJECTION);
        gl_pop_matrix();
        gl_matrix_mode(GL_MODELVIEW);

        gl_disable(GL_BLEND);
        gl_disable(GL_DEPTH_TEST);
    }

    gl_bind_texture(GL_TEXTURE_2D, state.texture_id);
    gl_copy_tex_sub_image_2d(GL_TEXTURE_2D, 0, 0, 0, 0, 0, sw, sh);
    state.has_valid_texture = true;
}
