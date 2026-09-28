use crate::acoustics_gui::draw_acoustic_rays_3d;
use crate::engine::cdll_int::{hk_initialize, hk_update_client_data, G_ENGFUNCS};
use crate::engine::proxy::{CL_FUNCS, P_CREATE_INTERFACE};
use crate::engine::sdk::{ClEngineFuncs, EntityState, LocalState, RefParams};
use crate::game::state::{hk_process_player_state, update_post_run_cmd};
use crate::hud::crosshair::{hk_hud_redraw, hud_vid_init};
use crate::render::view::hk_calc_refdef;
use std::os::raw::{c_char, c_double, c_float, c_int, c_uchar, c_uint, c_void};

macro_rules! delegate {
    (void $name:ident($($arg:ident : $ty:ty),*) => $field:ident) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name($($arg: $ty),*) {
            if let Some(f) = CL_FUNCS.$field { f($($arg),*); }
        }
    };
    (ret $name:ident($($arg:ident : $ty:ty),*) -> $ret:ty => $field:ident, $default:expr) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name($($arg: $ty),*) -> $ret {
            if let Some(f) = CL_FUNCS.$field { f($($arg),*) } else { $default }
        }
    };
}

delegate!(void CAM_Think() => pCamThink);
delegate!(void CL_CameraOffset(ofs: *mut c_float) => pCL_GetCameraOffsets);
delegate!(void HUD_CreateEntities() => pCreateEntities);
delegate!(void HUD_DirectorMessage(isize: c_int, pbuf: *mut c_void) => pDirectorMessage);
delegate!(void HUD_DrawNormalTriangles() => pDrawNormalTriangles);
delegate!(void HUD_Reset() => pHudResetFunc);
delegate!(void HUD_Shutdown() => pShutdown);
delegate!(void HUD_TxferLocalOverrides(state: *mut c_void, client: *const c_void) => pTxferLocalOverrides);
delegate!(void HUD_TxferPredictionData(ps: *mut c_void, pps: *const c_void, pcd: *mut c_void, ppcd: *const c_void, wd: *mut c_void, pwd: *const c_void) => pTxferPredictionData);
delegate!(void HUD_VoiceStatus(entindex: c_int, btalking: c_int) => pVoiceStatus);
delegate!(void IN_Accumulate() => pIN_Accumulate);
delegate!(void IN_ActivateMouse() => pIN_ActivateMouse);
delegate!(void IN_ClearStates() => pIN_ClearStates);
delegate!(void IN_DeactivateMouse() => pIN_DeactivateMouse);
delegate!(void IN_MouseEvent(mstate: c_int) => pIN_MouseEvent);
delegate!(void Demo_ReadBuffer(size: c_int, buffer: *mut c_uchar) => pReadDemoBuffer);
delegate!(void HUD_ChatInputPosition(x: *mut c_int, y: *mut c_int) => pChatInputPosition);

delegate!(ret HUD_AddEntity(type_idx: c_int, ent: *mut c_void, modelname: *const c_char) -> c_int => pAddEntity, 1);
delegate!(ret HUD_ConnectionlessPacket(net_from: *const c_void, args: *const c_char, response_buffer: *mut c_char, response_buffer_size: *mut c_int) -> c_int => pConnectionlessPacket, 0);
delegate!(ret HUD_GetHullBounds(hullnumber: c_int, mins: *mut c_float, maxs: *mut c_float) -> c_int => pGetHullBounds, 1);
delegate!(ret HUD_GetPlayerTeam(iplayer: c_int) -> c_int => pGetPlayerTeam, 0);
delegate!(ret HUD_Key_Event(down: c_int, keynum: c_int, pszbinding: *const c_char) -> c_int => pKeyEvent, 1);
delegate!(ret HUD_PlayerMoveTexture(name: *mut c_char) -> c_char => pClientTextureType, 0);
delegate!(ret CL_IsThirdPerson() -> c_int => pCL_IsThirdPerson, 0);

delegate!(ret HUD_GetUserEntity(index: c_int) -> *mut c_void => pGetUserEntity, std::ptr::null_mut());
delegate!(ret ClientFactory() -> *mut c_void => pClientFactory, std::ptr::null_mut());
delegate!(ret KB_Find(name: *const c_char) -> *mut c_void => pFindKey, std::ptr::null_mut());

#[no_mangle]
pub unsafe extern "C" fn CL_CreateMove(frametime: c_float, cmd: *mut c_void, active: c_int) {
    if let Some(f) = CL_FUNCS.pCL_CreateMove {
        f(frametime, cmd, active);
    }
    crate::hud::movement_gui::update_user_cmd(cmd);

    if crate::render::camera::is_freecam_active() {
        if let Some(user_cmd) = (cmd as *mut crate::hud::movement_gui::UserCmd).as_mut() {
            user_cmd.forwardmove = 0.0;
            user_cmd.sidemove = 0.0;
            user_cmd.upmove = 0.0;
            user_cmd.buttons &= !(crate::hud::movement_gui::IN_JUMP
                | crate::hud::movement_gui::IN_DUCK
                | crate::hud::movement_gui::IN_FORWARD
                | crate::hud::movement_gui::IN_BACK
                | crate::hud::movement_gui::IN_MOVELEFT
                | crate::hud::movement_gui::IN_MOVERIGHT
                | crate::hud::movement_gui::IN_ATTACK
                | crate::hud::movement_gui::IN_ATTACK2);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn CreateInterface(
    pname: *const c_char,
    preturncode: *mut c_int,
) -> *mut c_void {
    if let Some(f) = P_CREATE_INTERFACE {
        f(pname, preturncode)
    } else {
        std::ptr::null_mut()
    }
}

#[no_mangle]
pub unsafe extern "C" fn HUD_DrawTransparentTriangles() {
    draw_acoustic_rays_3d();
    if let Some(f) = CL_FUNCS.pDrawTransparentTriangles {
        f();
    }
}

#[no_mangle]
pub unsafe extern "C" fn HUD_Frame(time: c_double) {
    crate::cdll_int::hk_hud_frame(time);
}

#[no_mangle]
pub unsafe extern "C" fn HUD_GetStudioModelInterface(
    version: c_int,
    ppinterface: *mut *mut c_void,
    pstudio: *mut c_void,
) -> c_int {
    crate::studiorenderer::hk_get_studio_model_interface(version, ppinterface as _, pstudio as _)
}

#[no_mangle]
pub unsafe extern "C" fn HUD_Init() {
    crate::cdll_int::hk_hud_init();
}

#[no_mangle]
pub unsafe extern "C" fn HUD_PostRunCmd(
    from: *mut c_void,
    to: *mut c_void,
    cmd: *mut c_void,
    runfuncs: c_int,
    time: c_double,
    random_seed: c_uint,
) {
    if !from.is_null() {
        let from_state = from as *const LocalState;
        let client = &(*from_state).client;
        let weapon_id = client.m_iId as usize;
        let current_weapon = if weapon_id < 64 {
            &(*from_state).weapondata[weapon_id]
        } else {
            &(*from_state).weapondata[0]
        };

        update_post_run_cmd(
            client.flags,
            &client.velocity,
            current_weapon.m_iWeaponState,
            current_weapon.m_fInReload,
            current_weapon.m_fInSpecialReload,
        );
    }

    if let Some(f) = CL_FUNCS.pPostRunCmd {
        f(from, to, cmd, runfuncs, time, random_seed);
    }
}

#[no_mangle]
pub unsafe extern "C" fn HUD_ProcessPlayerState(dst: *mut c_void, src: *const c_void) {
    if !dst.is_null() && !src.is_null() {
        let dst_ent = dst as *const EntityState;
        let src_ent = src as *const EntityState;
        let local_player_index = if let Some(get_local) = G_ENGFUNCS.GetLocalPlayer {
            let local_ptr = get_local() as *const crate::engine::sdk::ClEntity;
            if !local_ptr.is_null() {
                (*local_ptr).index
            } else {
                -1
            }
        } else {
            -1
        };

        hk_process_player_state(
            (*dst_ent).number,
            local_player_index,
            (*src_ent).iuser1,
            (*src_ent).iuser2,
        );
    }

    if let Some(f) = CL_FUNCS.pProcessPlayerState {
        f(dst, src);
    }
}

#[no_mangle]
pub unsafe extern "C" fn HUD_Redraw(time: c_float, intermission: c_int) -> c_int {
    hk_hud_redraw(time, intermission)
}

#[no_mangle]
pub unsafe extern "C" fn HUD_StudioEvent(event: *const c_void, entity: *const c_void) {
    crate::hooks::sound::hook_studio_event(event, entity);
    if let Some(f) = CL_FUNCS.pStudioEvent {
        f(event, entity);
    }
}

#[no_mangle]
pub unsafe extern "C" fn HUD_TempEntUpdate(
    frametime: c_double,
    client_time: c_double,
    cl_gravity: c_double,
    ppfree: *mut *mut c_void,
    ppactive: *mut *mut c_void,
    cb_add_vis: *mut c_void,
    cb_sound: *mut c_void,
) {
    let sound_cb = crate::hooks::sound::wrap_cb_sound(cb_sound);
    if let Some(f) = CL_FUNCS.pTempEntUpdate {
        f(
            frametime,
            client_time,
            cl_gravity,
            ppfree,
            ppactive,
            cb_add_vis,
            sound_cb,
        );
    }
}

#[no_mangle]
pub unsafe extern "C" fn HUD_UpdateClientData(pcldata: *mut c_void, fltime: c_float) -> c_int {
    hk_update_client_data(pcldata, fltime)
}

#[no_mangle]
pub unsafe extern "C" fn HUD_VidInit() -> c_int {
    hud_vid_init();
    if let Some(f) = CL_FUNCS.pHudVidInitFunc {
        f()
    } else {
        1
    }
}

#[no_mangle]
pub unsafe extern "C" fn Initialize(penginefuncs: *mut ClEngineFuncs, iversion: c_int) -> c_int {
    hk_initialize(penginefuncs, iversion)
}

#[no_mangle]
pub unsafe extern "C" fn V_CalcRefdef(pparams: *mut c_void) {
    hk_calc_refdef(pparams as *mut RefParams);
}

#[no_mangle]
pub unsafe extern "C" fn HUD_PlayerMove(ppmove: *mut c_void, server: c_int) {
    crate::hooks::sound::hook_player_move(ppmove);
    if let Some(f) = CL_FUNCS.pClientMove {
        f(ppmove, server);
    }
}

#[no_mangle]
pub unsafe extern "C" fn HUD_PlayerMoveInit(ppmove: *mut c_void) {
    crate::hooks::sound::hook_player_move(ppmove);
    if let Some(f) = CL_FUNCS.pClientMoveInit {
        f(ppmove);
    }
}
