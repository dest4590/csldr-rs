use crate::engine::sdk::{EngineStudioApi, RStudioInterface};
use crate::render::gl::*;
use crate::utils::maths::radians;
use std::sync::atomic::{AtomicBool, Ordering};

pub static CAN_OPENGL: AtomicBool = AtomicBool::new(false);
pub static RENDER_ACTIVE: AtomicBool = AtomicBool::new(false);

pub static mut I_ENGINE_STUDIO: EngineStudioApi = unsafe { std::mem::zeroed() };
pub static mut STUDIO: RStudioInterface = unsafe { std::mem::zeroed() };

const Z_NEAR: f32 = 1.0;
const Z_FAR: f32 = 4096.0;

unsafe fn set_projection_matrix() {
    if !CAN_OPENGL.load(Ordering::Relaxed) || RENDER_ACTIVE.load(Ordering::Relaxed) {
        return;
    }

    let sw = crate::cdll_int::SCREEN_WIDTH.load(Ordering::Relaxed) as f32;
    let sh = crate::cdll_int::SCREEN_HEIGHT.load(Ordering::Relaxed) as f32;
    if sh <= 0.0 {
        return;
    }
    let aspect = sw / sh;

    let vm_fov = crate::view::get_viewmodel_fov();
    let fov = 0.75 * (radians(vm_fov) * 0.5).tan();
    if fov == 0.0 {
        return;
    }

    let f = 1.0 / fov;
    let fn_val = 1.0 / (Z_NEAR - Z_FAR);

    let mut matrix = [[0.0f32; 4]; 4];
    matrix[0][0] = f / aspect;
    matrix[1][1] = f;
    matrix[2][2] = (Z_NEAR + Z_FAR) * fn_val;
    matrix[2][3] = -1.0;
    matrix[3][2] = 2.0 * Z_NEAR * Z_FAR * fn_val;

    gl_matrix_mode(GL_PROJECTION);
    gl_push_matrix();
    gl_load_matrixf(matrix.as_ptr() as *const f32);
    gl_matrix_mode(GL_MODELVIEW);
}

unsafe fn restore_projection_matrix() {
    if !CAN_OPENGL.load(Ordering::Relaxed) || RENDER_ACTIVE.load(Ordering::Relaxed) {
        return;
    }

    gl_matrix_mode(GL_PROJECTION);
    gl_pop_matrix();
    gl_matrix_mode(GL_MODELVIEW);
}

pub unsafe extern "C" fn my_studio_draw_model(flags: i32) -> i32 {
    let draw_model = |f| STUDIO.StudioDrawModel.map_or(0, |draw| draw(f));

    let (cur_ent, view_ent) = match (
        I_ENGINE_STUDIO.GetCurrentEntity,
        I_ENGINE_STUDIO.GetViewEntity,
    ) {
        (Some(get_cur), Some(get_view)) => (get_cur(), get_view()),
        _ => return draw_model(flags),
    };

    if cur_ent.is_null() || cur_ent != view_ent {
        return draw_model(flags);
    }

    set_projection_matrix();

    let client_time = crate::cdll_int::CLIENT_TIME.lock().map_or(0.0, |t| *t);
    crate::inspect::inspect_think(client_time);

    let res = draw_model(flags);

    restore_projection_matrix();

    res
}

pub unsafe fn hk_get_studio_model_interface(
    version: i32,
    ppinterface: *mut *mut RStudioInterface,
    pstudio: *mut EngineStudioApi,
) -> i32 {
    if !pstudio.is_null() {
        if let Some(is_hw) = (*pstudio).IsHardware {
            let hw = is_hw();
            let can_gl = load_gl(hw);
            CAN_OPENGL.store(can_gl, Ordering::Relaxed);
        }
        std::ptr::copy_nonoverlapping(pstudio, &raw mut I_ENGINE_STUDIO, 1);
    }

    let result = if let Some(f) = crate::engine::proxy::CL_FUNCS.pStudioInterface {
        f(version, ppinterface as _, pstudio as _)
    } else {
        1
    };

    if !ppinterface.is_null() && !(*ppinterface).is_null() {
        std::ptr::copy_nonoverlapping(*ppinterface, &raw mut STUDIO, 1);
        (**ppinterface).StudioDrawModel = Some(my_studio_draw_model);
    }

    result
}
