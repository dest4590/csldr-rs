use crate::engine::export::{HUD_AddEntity, HUD_DrawNormalTriangles, HUD_DrawTransparentTriangles};
use crate::utils::platform::{plat_current_module_name, plat_dlclose, plat_dlopen, plat_dlsym};
use std::sync::Mutex;

#[repr(C)]
pub struct RenderParams {
    pub fov: f32,
    pub view_model_fov: f32,
    pub ref_params: *mut std::ffi::c_void,
}

pub type AddEntityCallback = unsafe extern "C" fn(*mut std::ffi::c_void) -> i32;

#[repr(C)]
pub struct ClientInterface {
    pub add_entity:
        unsafe extern "C" fn(i32, *mut std::ffi::c_void, *const std::os::raw::c_char) -> i32,
    pub draw_normal_triangles: unsafe extern "C" fn(),
    pub draw_transparent_triangles: unsafe extern "C" fn(),
}

#[repr(C)]
pub struct RenderInterface {
    pub modify_engfuncs: Option<unsafe extern "C" fn(*mut std::ffi::c_void)>,
    pub initialize: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *mut *mut std::ffi::c_void)>,
    pub begin_frame: Option<unsafe extern "C" fn() -> i32>,
    pub render_scene: Option<unsafe extern "C" fn(*const RenderParams)>,
    pub add_entity: Option<unsafe extern "C" fn(i32, *mut std::ffi::c_void) -> i32>,
    pub get_add_entity_callback:
        Option<unsafe extern "C" fn(AddEntityCallback) -> AddEntityCallback>,
    pub pre_draw_hud: Option<unsafe extern "C" fn()>,
    pub post_draw_hud: Option<unsafe extern "C" fn(i32, i32)>,
}

#[derive(Copy, Clone)]
struct RawHandle(*mut std::ffi::c_void);
unsafe impl Send for RawHandle {}

static RENDER_HANDLE: Mutex<RawHandle> = Mutex::new(RawHandle(std::ptr::null_mut()));

pub static RENDER_INTERFACE: Mutex<Option<RenderInterface>> = Mutex::new(None);

pub type LoaderConnect =
    unsafe extern "C" fn(*const ClientInterface, *mut RenderInterface, usize, usize) -> i32;

pub unsafe fn render_load() {
    let mut path_buf = [0 as std::os::raw::c_char; 512];
    let len = plat_current_module_name(path_buf.as_mut_ptr(), 512);
    if len == 0 {
        return;
    }

    let path_str = match std::ffi::CStr::from_ptr(path_buf.as_ptr()).to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return,
    };

    let filename_start = match path_str.rfind(['/', '\\']) {
        Some(pos) => pos + 1,
        None => 0,
    };

    if !path_str[filename_start..].starts_with("client") {
        return;
    }

    let mut new_path = path_str[..filename_start].to_string();
    new_path.push_str("render");
    new_path.push_str(&path_str[filename_start + 6..]);

    let c_path = match std::ffi::CString::new(new_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let handle = plat_dlopen(c_path.as_ptr());
    if handle.is_null() {
        return;
    }

    let connect_symbol = plat_dlsym(handle, b"LoaderConnect\0".as_ptr() as _);
    if connect_symbol.is_null() {
        plat_dlclose(handle);
        return;
    }

    let connect: LoaderConnect = std::mem::transmute(connect_symbol);
    let mut render_iface: RenderInterface = std::mem::zeroed();

    let client_iface = ClientInterface {
        add_entity: HUD_AddEntity,
        draw_normal_triangles: HUD_DrawNormalTriangles,
        draw_transparent_triangles: HUD_DrawTransparentTriangles,
    };

    if connect(
        &client_iface,
        &mut render_iface,
        std::mem::size_of::<RenderInterface>(),
        std::mem::size_of::<RenderParams>(),
    ) == 0
    {
        plat_dlclose(handle);
        return;
    }

    if let Ok(mut g_handle) = RENDER_HANDLE.lock() {
        *g_handle = RawHandle(handle);
    }
    if let Ok(mut g_iface) = RENDER_INTERFACE.lock() {
        *g_iface = Some(render_iface);
    }
}

pub unsafe fn render_unload() {
    if let Ok(mut g_handle) = RENDER_HANDLE.lock() {
        if !g_handle.0.is_null() {
            plat_dlclose(g_handle.0);
            *g_handle = RawHandle(std::ptr::null_mut());
        }
    }
    if let Ok(mut g_iface) = RENDER_INTERFACE.lock() {
        *g_iface = None;
    }
}
