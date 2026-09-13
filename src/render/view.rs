use std::f32::consts::PI;
use std::sync::Mutex;

use crate::engine::cdll_int::G_ENGFUNCS;
use crate::engine::proxy::CL_FUNCS;
use crate::engine::sdk::{ClEntity, RefParams};
use crate::game::state::{USER1, USER2};
use crate::hud::fov::FOV_SCALE;
use crate::utils::cvars::*;
use crate::utils::maths::{
    angle_lerp, angle_vectors, vector_copy, vector_length_2d, vector_ma, vector_ma_2,
    vector_negate, vector_subtract, Vec3,
};

const F_PI: f32 = PI;

#[derive(Default)]
struct BobVars {
    bob_time: f32,
    last_bob_time: f32,
    last_speed: f32,
    vert_bob: f32,
    hor_bob: f32,
}

static G_BOB_VARS: Mutex<BobVars> = Mutex::new(BobVars {
    bob_time: 0.0,
    last_bob_time: 0.0,
    last_speed: 0.0,
    vert_bob: 0.0,
    hor_bob: 0.0,
});

static V_CALC_BOB_TIME: Mutex<f64> = Mutex::new(0.0);

#[inline]
fn map_val(value: f32, low1: f32, high1: f32, low2: f32, high2: f32) -> f32 {
    low2 + (value - low1) * (high2 - low2) / (high1 - low1)
}

fn v_calc_bob_csgo(pparams: &RefParams) {
    let mut bv = G_BOB_VARS.lock().unwrap();

    let mut speed = vector_length_2d(&pparams.simvel);
    let max_speed_delta = ((pparams.time - bv.last_bob_time) * 620.0).max(0.0);

    speed = speed.clamp(
        bv.last_speed - max_speed_delta,
        bv.last_speed + max_speed_delta,
    );
    speed = speed.clamp(-320.0, 320.0);
    bv.last_speed = speed;

    let lower_amt = CL_BOB_LOWER_AMT.value() * (speed * 0.001);
    let bob_offset = map_val(speed, 0.0, 320.0, 0.0, 1.0);

    bv.bob_time += (pparams.time - bv.last_bob_time) * bob_offset;
    bv.last_bob_time = pparams.time;

    let bob_cycle = (((1000.0 - 150.0) / 3.5) * 0.001) * CL_BOBCYCLE.value() * 1.25;

    let mut cycle = bv.bob_time - (bv.bob_time / bob_cycle) as i32 as f32 * bob_cycle;
    cycle /= bob_cycle;

    let cl_bobup_val = CL_BOBUP.value();

    if cycle < cl_bobup_val {
        cycle = F_PI * cycle / cl_bobup_val;
    } else {
        cycle = F_PI + F_PI * (cycle - cl_bobup_val) / (1.0 - cl_bobup_val);
    }

    let mut bob_scale = 0.00625;
    if pparams.onground == 0 {
        bob_scale = 0.00125;
    }

    bv.vert_bob = speed * (bob_scale * CL_BOBAMT_VERT.value());
    bv.vert_bob = bv.vert_bob * 0.3 + bv.vert_bob * 0.7 * cycle.sin();
    bv.vert_bob = (bv.vert_bob - lower_amt).clamp(-8.0, 4.0);

    let mut cycle_h = bv.bob_time - (bv.bob_time / bob_cycle * 2.0) as i32 as f32 * bob_cycle * 2.0;
    cycle_h /= bob_cycle * 2.0;

    if cycle_h < cl_bobup_val {
        cycle_h = F_PI * cycle_h / cl_bobup_val;
    } else {
        cycle_h = F_PI + F_PI * (cycle_h - cl_bobup_val) / (1.0 - cl_bobup_val);
    }

    bv.hor_bob = speed * (bob_scale * CL_BOBAMT_LAT.value());
    bv.hor_bob = bv.hor_bob * 0.3 + bv.hor_bob * 0.7 * cycle_h.sin();
    bv.hor_bob = bv.hor_bob.clamp(-7.0, 4.0);
}

fn v_add_bob_csgo(pparams: &RefParams, origin: &mut [f32; 3], front: &[f32; 3], side: &[f32; 3]) {
    v_calc_bob_csgo(pparams);

    let bv = G_BOB_VARS.lock().unwrap();

    vector_ma_2(front, bv.vert_bob * 0.4, origin);
    origin[2] += bv.vert_bob * 0.1;
    vector_ma_2(side, bv.hor_bob * 0.2, origin);
}

fn v_calc_bob(pparams: &RefParams) -> f32 {
    let mut bobtime = V_CALC_BOB_TIME.lock().unwrap();
    *bobtime += pparams.frametime as f64;

    let cl_bobcycle_val = CL_BOBCYCLE.value() as f64;
    let mut cycle =
        (*bobtime - (*bobtime / cl_bobcycle_val) as i32 as f64 * cl_bobcycle_val) as f32;
    cycle /= cl_bobcycle_val as f32;

    let cl_bobup_val = CL_BOBUP.value();
    if cycle < cl_bobup_val {
        cycle = F_PI * cycle / cl_bobup_val;
    } else {
        cycle = F_PI + F_PI * (cycle - cl_bobup_val) / (1.0 - cl_bobup_val);
    }

    let mut bob = vector_length_2d(&pparams.simvel) * CL_BOB.value();
    bob = bob * 0.3 + bob * 0.7 * cycle.sin();
    bob.clamp(-7.0, 4.0)
}

unsafe fn v_add_lag_hl2(params: &RefParams, origin: &mut Vec3, front: &Vec3) {
    static LAST_FRONT: Mutex<Vec3> = Mutex::new([0.0, 0.0, 0.0]);
    let mut last_front = LAST_FRONT.lock().unwrap();
    let delta_front = [
        front[0] - last_front[0],
        front[1] - last_front[1],
        front[2] - last_front[2],
    ];

    vector_ma(
        &delta_front,
        VIEWMODEL_LAG_SPEED.value() * params.frametime,
        &mut last_front,
    );
    vector_ma_2(&delta_front, -VIEWMODEL_LAG_SCALE.value(), origin);
}

#[derive(Copy, Clone, Default)]
struct LagAngles {
    valid: bool,
    value: Vec3,
}

const STEP_MILLIS: f32 = 10.0;
const ANGLE_BACKUP: usize = 128;

static LAG_ANGLES: Mutex<[LagAngles; ANGLE_BACKUP]> = Mutex::new(
    [LagAngles {
        valid: false,
        value: [0.0, 0.0, 0.0],
    }; ANGLE_BACKUP],
);
static LAST_STEP: Mutex<i32> = Mutex::new(0);

fn add_lag_angles(time: f32, angles: &Vec3) {
    let step = (time * STEP_MILLIS) as i32;
    let mut last_step = LAST_STEP.lock().unwrap();

    if step < *last_step {
        if let Ok(mut l) = LAG_ANGLES.lock() {
            *l = [LagAngles {
                valid: false,
                value: [0.0, 0.0, 0.0],
            }; ANGLE_BACKUP];
        }
    } else if step == *last_step {
        return;
    }

    *last_step = step;
    let idx = (step as usize) % ANGLE_BACKUP;
    if let Ok(mut l) = LAG_ANGLES.lock() {
        l[idx].value = *angles;
        l[idx].valid = true;
    }
}

fn get_lag_angles(time: f32, dest: &mut Vec3) -> bool {
    let step = (time * STEP_MILLIS) as i32;
    let l = LAG_ANGLES.lock().unwrap();
    let idx = (step as usize) % ANGLE_BACKUP;

    if !l[idx].valid {
        return false;
    }

    let step_time = (step as f32) * (1.0 / STEP_MILLIS);
    if step_time < time {
        let next_idx = ((step + 1) as usize) % ANGLE_BACKUP;
        if !l[next_idx].valid {
            vector_copy(&l[idx].value, dest);
        } else {
            let next_time = ((step + 1) as f32) * (1.0 / STEP_MILLIS);
            let frac = map_val(time, step_time, next_time, 0.0, 1.0);
            angle_lerp(&l[idx].value, &l[next_idx].value, frac, dest);
        }
    } else {
        vector_copy(&l[idx].value, dest);
    }

    true
}

unsafe fn v_add_lag_css(
    params: &RefParams,
    origin: &mut Vec3,
    angles: &Vec3,
    front: &Vec3,
    side: &Vec3,
    up: &Vec3,
) {
    add_lag_angles(params.time, angles);

    let mut prev_angles: Vec3 = [0.0; 3];
    if !get_lag_angles(params.time - 0.1, &mut prev_angles) {
        return;
    }

    let mut delta_angles: Vec3 = [0.0; 3];
    vector_subtract(&prev_angles, angles, &mut delta_angles);
    vector_negate(&mut delta_angles);

    let mut delta_front: Vec3 = [0.0; 3];
    angle_vectors(&delta_angles, Some(&mut delta_front), None, None);

    vector_ma_2(
        front,
        (1.0 - delta_front[0]) * VIEWMODEL_LAG_SCALE.value(),
        origin,
    );
    vector_ma_2(side, delta_front[1] * VIEWMODEL_LAG_SCALE.value(), origin);
    vector_ma_2(up, -delta_front[2] * VIEWMODEL_LAG_SCALE.value(), origin);
}

fn v_offset_viewmodel(vm: &mut ClEntity, front: &[f32; 3], side: &[f32; 3], up: &[f32; 3]) {
    let mut x = VIEWMODEL_OFFSET_X.value();
    let y = VIEWMODEL_OFFSET_Y.value();
    let z = -VIEWMODEL_OFFSET_Z.value();

    if CL_RIGHTHAND.value() == 0.0 {
        x = -x;
    }

    vector_ma_2(side, x, &mut vm.origin);
    vector_ma_2(front, y, &mut vm.origin);
    vector_ma_2(up, z, &mut vm.origin);
}

pub unsafe fn calc_custom_refdef(pparams: &mut RefParams) {
    let vm_ptr = match G_ENGFUNCS.GetViewModel {
        Some(f) => f() as *mut ClEntity,
        None => return,
    };
    if vm_ptr.is_null() {
        return;
    }

    let vm = &mut *vm_ptr;
    let mut front = [0.0f32; 3];
    let mut side = [0.0f32; 3];
    let mut up = [0.0f32; 3];

    angle_vectors(&vm.angles, Some(&mut front), Some(&mut side), Some(&mut up));

    vm.origin[0] += 1.0 / 32.0;
    vm.origin[1] += 1.0 / 32.0;
    vm.origin[2] += 1.0 / 32.0;

    v_offset_viewmodel(vm, &front, &side, &up);

    let shift = VIEWMODEL_SHIFT.value() as i32;
    if shift == 1 {
        vm.origin[2] += 1.0;
    } else if shift == 2 {
        vm.origin[2] += 1.0;
        vector_ma_2(&up, 1.0, &mut vm.origin);
    }

    let bobstyle = CL_BOBSTYLE.value() as i32;
    if bobstyle == 2 {
        v_add_bob_csgo(pparams, &mut vm.origin, &front, &side);
    } else {
        let bob = v_calc_bob(pparams);
        vector_ma_2(&front, bob * 0.4, &mut vm.origin);

        if bobstyle == 1 {
            vm.curstate.angles[0] -= bob * 0.3;
            vm.curstate.angles[1] -= bob * 0.5;
            vm.curstate.angles[2] -= bob;
        }

        if CL_BOB_CAMERA.value() != 0.0 {
            pparams.vieworg[2] += bob;
            vm.origin[2] += bob;
        }
    }

    match VIEWMODEL_LAG_STYLE.value() as i32 {
        1 => v_add_lag_hl2(pparams, &mut vm.origin, &front),
        2 => v_add_lag_css(pparams, &mut vm.origin, &vm.angles, &front, &side, &up),
        _ => {}
    }
}

unsafe fn v_get_in_eye_pos(target: i32, origin: &mut Vec3, angles: &mut Vec3) {
    if target == 0 {
        return;
    }

    let get_ent = match G_ENGFUNCS.GetEntityByIndex {
        Some(f) => f,
        None => return,
    };

    let ent = get_ent(target) as *const ClEntity;
    if ent.is_null() {
        return;
    }

    let ent_ref = &*ent;
    *origin = ent_ref.origin;
    *angles = ent_ref.angles;
    angles[0] *= -3.0;

    if ent_ref.curstate.solid == 0 {
        angles[2] = 80.0;
        origin[2] -= 8.0;
    } else if ent_ref.curstate.usehull == 1 {
        origin[2] += 12.0;
    } else {
        origin[2] += 28.0;
    }
}

pub unsafe fn calc_spectator_refdef(params: &mut RefParams, next_view: i32) {
    const OBS_IN_EYE: i32 = 4;
    const INSET_IN_EYE: i32 = 2;

    let user1 = USER1.load(std::sync::atomic::Ordering::Relaxed);
    let user2 = USER2.load(std::sync::atomic::Ordering::Relaxed);

    let get_ent = match G_ENGFUNCS.GetEntityByIndex {
        Some(f) => f,
        None => return,
    };

    let ent = get_ent(user2) as *const ClEntity;

    if (user1 == OBS_IN_EYE || SPEC_PIP.value() as i32 == INSET_IN_EYE) && !ent.is_null() {
        let ent_ref = &*ent;
        let time_diff = ent_ref.curstate.msg_time - ent_ref.prevstate.msg_time;
        if time_diff > 0.0 {
            static VELOCITY: Mutex<Vec3> = Mutex::new([0.0, 0.0, 0.0]);
            let mut distance = [0.0f32; 3];
            vector_subtract(
                &ent_ref.prevstate.origin,
                &ent_ref.curstate.origin,
                &mut distance,
            );
            let inv_t = 1.0 / time_diff;
            distance[0] *= inv_t;
            distance[1] *= inv_t;
            distance[2] *= inv_t;

            if let Ok(mut vel) = VELOCITY.lock() {
                vel[0] = vel[0] * 0.9 + distance[0] * 0.1;
                vel[1] = vel[1] * 0.9 + distance[1] * 0.1;
                vel[2] = vel[2] * 0.9 + distance[2] * 0.1;
                params.simvel = *vel;
            }
        }

        let is_spec = G_ENGFUNCS.IsSpectateOnly.map(|f| f() != 0).unwrap_or(false);
        if is_spec || SPEC_PIP.value() as i32 == INSET_IN_EYE {
            v_get_in_eye_pos(user2, &mut params.simorg, &mut params.cl_viewangles);
        } else {
            params.cl_viewangles = ent_ref.angles;
            params.cl_viewangles[0] *= -3.0;
        }
    }

    if next_view == 0 {
        if user1 == OBS_IN_EYE {
            calc_custom_refdef(params);
        }
    } else if SPEC_PIP.value() as i32 == INSET_IN_EYE {
        calc_custom_refdef(params);
    }
}

pub unsafe fn hk_calc_refdef(pparams: *mut RefParams) {
    if pparams.is_null() {
        return;
    }

    let params = &mut *pparams;

    let mut old_rollangle = 0.0;
    let mut old_rollspeed = 0.0;
    if !params.movevars.is_null() {
        old_rollangle = (*params.movevars).rollangle;
        old_rollspeed = (*params.movevars).rollspeed;
        (*params.movevars).rollangle = CL_ROLLANGLE.value();
        (*params.movevars).rollspeed = CL_ROLLSPEED.value();
    }

    let next_view = params.nextView;

    let orig_bob = CL_BOB.value();
    let bob_ptr = CL_BOB.get_ptr();
    if !bob_ptr.is_null() {
        (*bob_ptr).value = 0.0;
    }

    if let Some(f) = CL_FUNCS.pCalcRefdef {
        f(pparams);
    }

    if !bob_ptr.is_null() {
        (*bob_ptr).value = orig_bob;
    }

    if !params.movevars.is_null() {
        (*params.movevars).rollangle = old_rollangle;
        (*params.movevars).rollspeed = old_rollspeed;
    }

    if params.intermission == 0 {
        let user1 = USER1.load(std::sync::atomic::Ordering::Relaxed);
        if params.spectator != 0 || user1 != 0 {
            calc_spectator_refdef(params, next_view);
        } else if params.paused == 0 {
            calc_custom_refdef(params);
        }
    }

    crate::engine::sound_hook::update_listener_state(
        params.vieworg,
        params.viewangles,
        params.forward,
        params.right,
        params.up,
    );

    let player_origin = if params.simorg != [0.0, 0.0, 0.0] {
        params.simorg
    } else {
        [
            params.vieworg[0],
            params.vieworg[1],
            params.vieworg[2] - 28.0,
        ]
    };

    crate::engine::acoustics::update_room_acoustics(player_origin);

    crate::hud::movement_gui::update_movement_physics(params);

    crate::render::camera::update_freecam(params);
}

pub fn get_viewmodel_fov() -> f32 {
    let base_fov = get_cvar_float("viewmodel_fov", &VIEWMODEL_FOV);
    let fov_val = if base_fov <= 0.0 { 90.0 } else { base_fov };
    let scale = FOV_SCALE.lock().map(|s| *s).unwrap_or(1.0);
    let result = fov_val * scale;
    result.clamp(1.0, 179.0)
}
