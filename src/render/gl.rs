use std::ffi::c_void;
use std::os::raw::c_char;

pub const GL_MODELVIEW: u32 = 0x1700;
pub const GL_PROJECTION: u32 = 0x1701;
pub const GL_LINES: u32 = 0x0001;
pub const GL_LINE_LOOP: u32 = 0x0002;
pub const GL_LINE_STRIP: u32 = 0x0003;
pub const GL_TRIANGLE_STRIP: u32 = 0x0005;
pub const GL_QUADS: u32 = 0x0007;
pub const GL_DEPTH_TEST: u32 = 0x0B71;
pub const GL_BLEND: u32 = 0x0BE2;
pub const GL_LINE_SMOOTH: u32 = 0x0B10;
pub const GL_TEXTURE_2D: u32 = 0x0DE1;
pub const GL_UNSIGNED_BYTE: u32 = 0x1401;
pub const GL_SRC_ALPHA: u32 = 0x0302;
pub const GL_ONE_MINUS_SRC_ALPHA: u32 = 0x0303;
pub const GL_ONE: u32 = 1;
pub const GL_RGBA: u32 = 0x1908;
pub const GL_TEXTURE_MAG_FILTER: u32 = 0x2800;
pub const GL_TEXTURE_MIN_FILTER: u32 = 0x2801;
pub const GL_TEXTURE_WRAP_S: u32 = 0x2802;
pub const GL_TEXTURE_WRAP_T: u32 = 0x2803;
pub const GL_LINEAR: u32 = 0x2601;
pub const GL_RGBA8: u32 = 0x8058;
pub const GL_CLAMP_TO_EDGE: u32 = 0x812F;

#[cfg(windows)]
pub type FnMatrixMode = unsafe extern "system" fn(u32);
#[cfg(not(windows))]
pub type FnMatrixMode = unsafe extern "C" fn(u32);

#[cfg(windows)]
pub type FnPushMatrix = unsafe extern "system" fn();
#[cfg(not(windows))]
pub type FnPushMatrix = unsafe extern "C" fn();

#[cfg(windows)]
pub type FnPopMatrix = unsafe extern "system" fn();
#[cfg(not(windows))]
pub type FnPopMatrix = unsafe extern "C" fn();

#[cfg(windows)]
pub type FnLoadMatrixf = unsafe extern "system" fn(*const f32);
#[cfg(not(windows))]
pub type FnLoadMatrixf = unsafe extern "C" fn(*const f32);

#[cfg(windows)]
pub type FnLoadIdentity = unsafe extern "system" fn();
#[cfg(not(windows))]
pub type FnLoadIdentity = unsafe extern "C" fn();

#[cfg(windows)]
pub type FnOrtho = unsafe extern "system" fn(f64, f64, f64, f64, f64, f64);
#[cfg(not(windows))]
pub type FnOrtho = unsafe extern "C" fn(f64, f64, f64, f64, f64, f64);

#[cfg(windows)]
pub type FnEnable = unsafe extern "system" fn(u32);
#[cfg(not(windows))]
pub type FnEnable = unsafe extern "C" fn(u32);

#[cfg(windows)]
pub type FnDisable = unsafe extern "system" fn(u32);
#[cfg(not(windows))]
pub type FnDisable = unsafe extern "C" fn(u32);

#[cfg(windows)]
pub type FnBlendFunc = unsafe extern "system" fn(u32, u32);
#[cfg(not(windows))]
pub type FnBlendFunc = unsafe extern "C" fn(u32, u32);

#[cfg(windows)]
pub type FnColor4f = unsafe extern "system" fn(f32, f32, f32, f32);
#[cfg(not(windows))]
pub type FnColor4f = unsafe extern "C" fn(f32, f32, f32, f32);

#[cfg(windows)]
pub type FnBegin = unsafe extern "system" fn(u32);
#[cfg(not(windows))]
pub type FnBegin = unsafe extern "C" fn(u32);

#[cfg(windows)]
pub type FnEnd = unsafe extern "system" fn();
#[cfg(not(windows))]
pub type FnEnd = unsafe extern "C" fn();

#[cfg(windows)]
pub type FnVertex2f = unsafe extern "system" fn(f32, f32);
#[cfg(not(windows))]
pub type FnVertex2f = unsafe extern "C" fn(f32, f32);

#[cfg(windows)]
pub type FnVertex3f = unsafe extern "system" fn(f32, f32, f32);
#[cfg(not(windows))]
pub type FnVertex3f = unsafe extern "C" fn(f32, f32, f32);

#[cfg(windows)]
pub type FnTexCoord2f = unsafe extern "system" fn(f32, f32);
#[cfg(not(windows))]
pub type FnTexCoord2f = unsafe extern "C" fn(f32, f32);

#[cfg(windows)]
pub type FnTexImage2D =
    unsafe extern "system" fn(u32, i32, i32, i32, i32, i32, u32, u32, *const c_void);
#[cfg(not(windows))]
pub type FnTexImage2D = unsafe extern "C" fn(u32, i32, i32, i32, i32, i32, u32, u32, *const c_void);

#[cfg(windows)]
pub type FnTexParameteri = unsafe extern "system" fn(u32, u32, i32);
#[cfg(not(windows))]
pub type FnTexParameteri = unsafe extern "C" fn(u32, u32, i32);

#[allow(non_upper_case_globals)]
pub static mut glad_glMatrixMode: Option<FnMatrixMode> = None;
#[allow(non_upper_case_globals)]
pub static mut glad_glPushMatrix: Option<FnPushMatrix> = None;
#[allow(non_upper_case_globals)]
pub static mut glad_glPopMatrix: Option<FnPopMatrix> = None;
#[allow(non_upper_case_globals)]
pub static mut glad_glLoadMatrixf: Option<FnLoadMatrixf> = None;
#[allow(non_upper_case_globals)]
pub static mut glad_glLoadIdentity: Option<FnLoadIdentity> = None;
#[allow(non_upper_case_globals)]
pub static mut glad_glOrtho: Option<FnOrtho> = None;
#[allow(non_upper_case_globals)]
pub static mut glad_glEnable: Option<FnEnable> = None;
#[allow(non_upper_case_globals)]
pub static mut glad_glDisable: Option<FnDisable> = None;
#[allow(non_upper_case_globals)]
pub static mut glad_glBlendFunc: Option<FnBlendFunc> = None;
#[allow(non_upper_case_globals)]
pub static mut glad_glColor4f: Option<FnColor4f> = None;
#[allow(non_upper_case_globals)]
pub static mut glad_glBegin: Option<FnBegin> = None;
#[allow(non_upper_case_globals)]
pub static mut glad_glEnd: Option<FnEnd> = None;
#[allow(non_upper_case_globals)]
pub static mut glad_glVertex2f: Option<FnVertex2f> = None;
#[allow(non_upper_case_globals)]
pub static mut glad_glVertex3f: Option<FnVertex3f> = None;
#[allow(non_upper_case_globals)]
pub static mut glad_glTexCoord2f: Option<FnTexCoord2f> = None;
#[allow(non_upper_case_globals)]
pub static mut glad_glTexImage2D: Option<FnTexImage2D> = None;
#[allow(non_upper_case_globals)]
pub static mut glad_glTexParameteri: Option<FnTexParameteri> = None;

macro_rules! gl_wrap {
    ($name:ident, $glad:ident, $($arg:ident : $ty:ty),*) => {
        #[inline]
        pub unsafe fn $name($($arg : $ty),*) {
            if let Some(f) = $glad {
                f($($arg),*);
            }
        }
    };
    ($name:ident, $glad:ident) => {
        #[inline]
        pub unsafe fn $name() {
            if let Some(f) = $glad {
                f();
            }
        }
    };
}

macro_rules! gl_wrap_dyn {
    ($name:ident, $dyn_ptr:ident, $($arg:ident : $ty:ty),*) => {
        #[inline]
        pub unsafe fn $name($($arg : $ty),*) {
            if let Some(f) = $dyn_ptr {
                f($($arg),*);
            }
        }
    };
}

#[cfg(windows)]
type FnGenTextures = unsafe extern "system" fn(i32, *mut u32);
#[cfg(not(windows))]
type FnGenTextures = unsafe extern "C" fn(i32, *mut u32);

#[cfg(windows)]
type FnBindTexture = unsafe extern "system" fn(u32, u32);
#[cfg(not(windows))]
type FnBindTexture = unsafe extern "C" fn(u32, u32);

#[cfg(windows)]
type FnCopyTexSubImage2D = unsafe extern "system" fn(u32, i32, i32, i32, i32, i32, i32, i32);
#[cfg(not(windows))]
type FnCopyTexSubImage2D = unsafe extern "C" fn(u32, i32, i32, i32, i32, i32, i32, i32);

#[cfg(windows)]
type FnDeleteTextures = unsafe extern "system" fn(i32, *const u32);
#[cfg(not(windows))]
type FnDeleteTextures = unsafe extern "C" fn(i32, *const u32);

#[cfg(windows)]
type FnLineWidth = unsafe extern "system" fn(f32);
#[cfg(not(windows))]
type FnLineWidth = unsafe extern "C" fn(f32);

static mut DYN_GEN_TEXTURES: Option<FnGenTextures> = None;
static mut DYN_BIND_TEXTURE: Option<FnBindTexture> = None;
static mut DYN_COPY_TEX_SUB_IMAGE_2D: Option<FnCopyTexSubImage2D> = None;
static mut DYN_DELETE_TEXTURES: Option<FnDeleteTextures> = None;
static mut DYN_LINE_WIDTH: Option<FnLineWidth> = None;

unsafe fn resolve_gl_symbol<T: Copy>(name: &[u8]) -> Option<T> {
    #[cfg(unix)]
    {
        let sym = libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr() as *const c_char);
        if sym.is_null() {
            None
        } else {
            Some(std::mem::transmute_copy(&sym))
        }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
        let h_opengl = GetModuleHandleA(b"opengl32.dll\0".as_ptr());
        if !h_opengl.is_null() {
            if let Some(sym) = GetProcAddress(h_opengl, name.as_ptr()) {
                return Some(std::mem::transmute_copy(&sym));
            }
        }
        None
    }
}

pub unsafe extern "C" fn d3d_gl_get_proc_address(_name: *const c_char) -> *mut c_void {
    std::ptr::null_mut()
}

unsafe extern "C" fn default_loader(name: *const c_char) -> *mut c_void {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
        let h_opengl = GetModuleHandleA(b"opengl32.dll\0".as_ptr());
        if !h_opengl.is_null() {
            let wgl_proc = GetProcAddress(h_opengl, b"wglGetProcAddress\0".as_ptr());
            if let Some(wgl_get_proc_address) = wgl_proc {
                let wgl_fn: unsafe extern "system" fn(*const c_char) -> *mut c_void =
                    std::mem::transmute(wgl_get_proc_address);
                let p = wgl_fn(name);
                if !p.is_null() {
                    return p;
                }
            }
            if let Some(proc) = GetProcAddress(h_opengl, name as *const u8) {
                return proc as *mut c_void;
            }
        }
        std::ptr::null_mut()
    }
    #[cfg(not(windows))]
    {
        libc::dlsym(libc::RTLD_DEFAULT, name)
    }
}

unsafe fn get_proc_address(
    loader: unsafe extern "C" fn(*const c_char) -> *mut c_void,
    name: &[u8],
) -> *mut c_void {
    let ptr = loader(name.as_ptr() as *const c_char);
    if !ptr.is_null() {
        return ptr;
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
        let h_module = GetModuleHandleA(b"opengl32.dll\0".as_ptr());
        if !h_module.is_null() {
            if let Some(proc) = GetProcAddress(h_module, name.as_ptr()) {
                return proc as *mut c_void;
            }
        }
    }
    #[cfg(not(windows))]
    {
        let p = libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr() as *const c_char);
        if !p.is_null() {
            return p;
        }
    }
    std::ptr::null_mut()
}

pub unsafe fn glad_load_gl_loader(
    loader: unsafe extern "C" fn(*const c_char) -> *mut c_void,
) -> bool {
    macro_rules! load_sym {
        ($glad:ident, $name:expr, $ty:ty) => {
            let p = get_proc_address(loader, $name);
            $glad = if !p.is_null() {
                Some(std::mem::transmute_copy::<*mut c_void, $ty>(&p))
            } else {
                None
            };
        };
    }

    load_sym!(glad_glMatrixMode, b"glMatrixMode\0", FnMatrixMode);
    load_sym!(glad_glPushMatrix, b"glPushMatrix\0", FnPushMatrix);
    load_sym!(glad_glPopMatrix, b"glPopMatrix\0", FnPopMatrix);
    load_sym!(glad_glLoadMatrixf, b"glLoadMatrixf\0", FnLoadMatrixf);
    load_sym!(glad_glLoadIdentity, b"glLoadIdentity\0", FnLoadIdentity);
    load_sym!(glad_glOrtho, b"glOrtho\0", FnOrtho);
    load_sym!(glad_glEnable, b"glEnable\0", FnEnable);
    load_sym!(glad_glDisable, b"glDisable\0", FnDisable);
    load_sym!(glad_glBlendFunc, b"glBlendFunc\0", FnBlendFunc);
    load_sym!(glad_glColor4f, b"glColor4f\0", FnColor4f);
    load_sym!(glad_glBegin, b"glBegin\0", FnBegin);
    load_sym!(glad_glEnd, b"glEnd\0", FnEnd);
    load_sym!(glad_glVertex2f, b"glVertex2f\0", FnVertex2f);
    load_sym!(glad_glVertex3f, b"glVertex3f\0", FnVertex3f);
    load_sym!(glad_glTexCoord2f, b"glTexCoord2f\0", FnTexCoord2f);
    load_sym!(glad_glTexImage2D, b"glTexImage2D\0", FnTexImage2D);
    load_sym!(glad_glTexParameteri, b"glTexParameteri\0", FnTexParameteri);

    glad_glMatrixMode.is_some()
}

pub unsafe fn load_gl(hardware_type: i32) -> bool {
    let success = match hardware_type {
        1 => glad_load_gl_loader(default_loader),
        2 => glad_load_gl_loader(d3d_gl_get_proc_address),
        _ => false,
    };

    if success {
        DYN_GEN_TEXTURES = resolve_gl_symbol(b"glGenTextures\0");
        DYN_BIND_TEXTURE = resolve_gl_symbol(b"glBindTexture\0");
        DYN_COPY_TEX_SUB_IMAGE_2D = resolve_gl_symbol(b"glCopyTexSubImage2D\0");
        DYN_DELETE_TEXTURES = resolve_gl_symbol(b"glDeleteTextures\0");
        DYN_LINE_WIDTH = resolve_gl_symbol(b"glLineWidth\0");
    }

    success
}

gl_wrap!(gl_matrix_mode,   glad_glMatrixMode,    mode: u32);
gl_wrap!(gl_push_matrix, glad_glPushMatrix);
gl_wrap!(gl_pop_matrix, glad_glPopMatrix);
gl_wrap!(gl_load_matrixf,  glad_glLoadMatrixf,   m: *const f32);
gl_wrap!(gl_load_identity, glad_glLoadIdentity);
gl_wrap!(gl_ortho,         glad_glOrtho,         left: f64, right: f64, bottom: f64, top: f64, z_near: f64, z_far: f64);
gl_wrap!(gl_enable,        glad_glEnable,        cap: u32);
gl_wrap!(gl_disable,       glad_glDisable,       cap: u32);
gl_wrap!(gl_blend_func,    glad_glBlendFunc,     sfactor: u32, dfactor: u32);
gl_wrap!(gl_color4f,       glad_glColor4f,       r: f32, g: f32, b: f32, a: f32);
gl_wrap!(gl_begin,         glad_glBegin,         mode: u32);
gl_wrap!(gl_end, glad_glEnd);
gl_wrap!(gl_vertex2f,      glad_glVertex2f,      x: f32, y: f32);
gl_wrap!(gl_vertex3f,      glad_glVertex3f,      x: f32, y: f32, z: f32);
gl_wrap!(gl_tex_coord2f,   glad_glTexCoord2f,    s: f32, t: f32);
gl_wrap!(gl_tex_parameter_i, glad_glTexParameteri, target: u32, pname: u32, param: i32);

gl_wrap_dyn!(gl_line_width,            DYN_LINE_WIDTH,            width: f32);
gl_wrap_dyn!(gl_gen_textures,          DYN_GEN_TEXTURES,          n: i32, textures: *mut u32);
gl_wrap_dyn!(gl_bind_texture,          DYN_BIND_TEXTURE,          target: u32, texture: u32);
gl_wrap_dyn!(gl_delete_textures,       DYN_DELETE_TEXTURES,       n: i32, textures: *const u32);

gl_wrap!(
    gl_tex_image_2d,
    glad_glTexImage2D,
    target: u32,
    level: i32,
    internalformat: i32,
    width: i32,
    height: i32,
    border: i32,
    format: u32,
    ty: u32,
    pixels: *const c_void
);

gl_wrap_dyn!(
    gl_copy_tex_sub_image_2d,
    DYN_COPY_TEX_SUB_IMAGE_2D,
    target: u32,
    level: i32,
    xoffset: i32,
    yoffset: i32,
    x: i32,
    y: i32,
    width: i32,
    height: i32
);
