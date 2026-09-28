use crate::engine::sdk::ClDllFunc;
use crate::utils::platform::{
    plat_current_module_name, plat_dlclose, plat_dlopen, plat_dlsym, plat_error,
};
use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

#[derive(Copy, Clone)]
struct RawPtr(*mut c_void);
unsafe impl Send for RawPtr {}

static CLIENT_ORIG: Mutex<RawPtr> = Mutex::new(RawPtr(std::ptr::null_mut()));
pub static mut CL_FUNCS: ClDllFunc = unsafe { std::mem::zeroed() };
pub static mut P_CREATE_INTERFACE: Option<
    unsafe extern "C" fn(*const c_char, *mut c_int) -> *mut c_void,
> = None;
static INITIALIZED: AtomicBool = AtomicBool::new(false);

pub unsafe fn proxy_init() {
    if INITIALIZED.swap(true, Ordering::Relaxed) {
        return;
    }

    let mut path_buf = [0 as std::os::raw::c_char; 512];
    let len = plat_current_module_name(path_buf.as_mut_ptr(), 512);
    if len == 0 {
        plat_error("Could not get current module name\n");
    }

    let path_cstr = std::ffi::CStr::from_ptr(path_buf.as_ptr());
    let mut path_str = path_cstr.to_string_lossy().into_owned();

    const ORIG_SUFFIX: &str = "_orig";
    if let Some(dot_idx) = path_str.rfind('.') {
        path_str.insert_str(dot_idx, ORIG_SUFFIX);
    } else {
        path_str.push_str(ORIG_SUFFIX);
    }

    let c_path = CString::new(path_str).unwrap();
    let handle = plat_dlopen(c_path.as_ptr());
    if handle.is_null() {
        return;
    }

    if let Ok(mut orig) = CLIENT_ORIG.lock() {
        orig.0 = handle;
    }

    macro_rules! load_sym {
        ($field:expr, $name:expr) => {
            let sym_c = CString::new($name).unwrap();
            let ptr = plat_dlsym(handle, sym_c.as_ptr());
            if ptr.is_null() {
                plat_error(&format!("Could not find {}\n", $name));
            }
            #[allow(clippy::missing_transmute_annotations)]
            {
                $field = std::mem::transmute(ptr);
            }
        };
    }

    load_sym!(P_CREATE_INTERFACE, "CreateInterface");

    load_sym!(CL_FUNCS.pInitFunc, "Initialize");
    load_sym!(CL_FUNCS.pHudInitFunc, "HUD_Init");
    load_sym!(CL_FUNCS.pHudVidInitFunc, "HUD_VidInit");
    load_sym!(CL_FUNCS.pHudRedrawFunc, "HUD_Redraw");
    load_sym!(CL_FUNCS.pHudUpdateClientDataFunc, "HUD_UpdateClientData");
    load_sym!(CL_FUNCS.pHudResetFunc, "HUD_Reset");
    load_sym!(CL_FUNCS.pClientMove, "HUD_PlayerMove");
    load_sym!(CL_FUNCS.pClientMoveInit, "HUD_PlayerMoveInit");
    load_sym!(CL_FUNCS.pClientTextureType, "HUD_PlayerMoveTexture");
    load_sym!(CL_FUNCS.pIN_ActivateMouse, "IN_ActivateMouse");
    load_sym!(CL_FUNCS.pIN_DeactivateMouse, "IN_DeactivateMouse");
    load_sym!(CL_FUNCS.pIN_MouseEvent, "IN_MouseEvent");
    load_sym!(CL_FUNCS.pIN_ClearStates, "IN_ClearStates");
    load_sym!(CL_FUNCS.pIN_Accumulate, "IN_Accumulate");
    load_sym!(CL_FUNCS.pCL_CreateMove, "CL_CreateMove");
    load_sym!(CL_FUNCS.pCL_IsThirdPerson, "CL_IsThirdPerson");
    load_sym!(CL_FUNCS.pCL_GetCameraOffsets, "CL_CameraOffset");
    load_sym!(CL_FUNCS.pFindKey, "KB_Find");
    load_sym!(CL_FUNCS.pCamThink, "CAM_Think");
    load_sym!(CL_FUNCS.pCalcRefdef, "V_CalcRefdef");
    load_sym!(CL_FUNCS.pAddEntity, "HUD_AddEntity");
    load_sym!(CL_FUNCS.pCreateEntities, "HUD_CreateEntities");
    load_sym!(CL_FUNCS.pDrawNormalTriangles, "HUD_DrawNormalTriangles");
    load_sym!(
        CL_FUNCS.pDrawTransparentTriangles,
        "HUD_DrawTransparentTriangles"
    );
    load_sym!(CL_FUNCS.pStudioEvent, "HUD_StudioEvent");
    load_sym!(CL_FUNCS.pPostRunCmd, "HUD_PostRunCmd");
    load_sym!(CL_FUNCS.pShutdown, "HUD_Shutdown");
    load_sym!(CL_FUNCS.pTxferLocalOverrides, "HUD_TxferLocalOverrides");
    load_sym!(CL_FUNCS.pProcessPlayerState, "HUD_ProcessPlayerState");
    load_sym!(CL_FUNCS.pTxferPredictionData, "HUD_TxferPredictionData");
    load_sym!(CL_FUNCS.pReadDemoBuffer, "Demo_ReadBuffer");
    load_sym!(CL_FUNCS.pConnectionlessPacket, "HUD_ConnectionlessPacket");
    load_sym!(CL_FUNCS.pGetHullBounds, "HUD_GetHullBounds");
    load_sym!(CL_FUNCS.pHudFrame, "HUD_Frame");
    load_sym!(CL_FUNCS.pKeyEvent, "HUD_Key_Event");
    load_sym!(CL_FUNCS.pTempEntUpdate, "HUD_TempEntUpdate");
    load_sym!(CL_FUNCS.pGetUserEntity, "HUD_GetUserEntity");
    load_sym!(CL_FUNCS.pVoiceStatus, "HUD_VoiceStatus");
    load_sym!(CL_FUNCS.pDirectorMessage, "HUD_DirectorMessage");
    load_sym!(CL_FUNCS.pStudioInterface, "HUD_GetStudioModelInterface");
    load_sym!(CL_FUNCS.pChatInputPosition, "HUD_ChatInputPosition");
    load_sym!(CL_FUNCS.pGetPlayerTeam, "HUD_GetPlayerTeam");
    load_sym!(CL_FUNCS.pClientFactory, "ClientFactory");

    crate::hooks::alsa_sdl::install_audio_hooks();
    crate::render::render::render_load();
}

pub unsafe fn proxy_quit() {
    crate::render::render::render_unload();
    if let Ok(mut orig) = CLIENT_ORIG.lock() {
        if !orig.0.is_null() {
            plat_dlclose(orig.0);
            orig.0 = std::ptr::null_mut();
        }
    }
}
