use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::process;
use std::sync::atomic::AtomicI32;

pub static IS_WARZONE: AtomicI32 = AtomicI32::new(0);

pub unsafe fn plat_dlopen(filename: *const c_char) -> *mut std::ffi::c_void {
    #[cfg(unix)]
    {
        libc::dlopen(filename, libc::RTLD_NOW)
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::LibraryLoader::LoadLibraryA;
        LoadLibraryA(filename as _) as *mut _
    }
}

pub unsafe fn plat_checked_dlopen(filename: *const c_char) -> *mut std::ffi::c_void {
    let handle = plat_dlopen(filename);
    if handle.is_null() {
        let name_str = CStr::from_ptr(filename).to_string_lossy();
        plat_error(&format!("Could not load '{name_str}'"));
    }
    handle
}

pub unsafe fn plat_dlsym(
    handle: *mut std::ffi::c_void,
    name: *const c_char,
) -> *mut std::ffi::c_void {
    #[cfg(unix)]
    {
        libc::dlsym(handle, name)
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::LibraryLoader::GetProcAddress;
        GetProcAddress(handle as _, name as _) as *mut _
    }
}

pub unsafe fn plat_dlclose(handle: *mut std::ffi::c_void) {
    #[cfg(unix)]
    {
        libc::dlclose(handle);
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::LibraryLoader::FreeLibrary;
        FreeLibrary(handle as _);
    }
}

pub unsafe fn plat_current_module_name(buf: *mut c_char, size: usize) -> usize {
    if size == 0 {
        return 0;
    }
    #[cfg(unix)]
    {
        let mut info: libc::Dl_info = std::mem::zeroed();
        if libc::dladdr(plat_current_module_name as *const _, &mut info) == 0
            || info.dli_fname.is_null()
        {
            return 0;
        }
        let len = libc::strlen(info.dli_fname);
        if len >= size {
            return 0;
        }
        libc::memcpy(buf as _, info.dli_fname as _, len + 1);
        len
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::LibraryLoader::{
            GetModuleFileNameA, GetModuleHandleExA, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        };
        let mut h_module = 0isize;
        GetModuleHandleExA(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            plat_current_module_name as *const _,
            &mut h_module,
        );
        let len = GetModuleFileNameA(h_module, buf as _, size as u32) as usize;
        if len == 0 || len >= size {
            0
        } else {
            len
        }
    }
}

pub fn plat_error(msg: &str) -> ! {
    eprintln!("{msg}");
    #[cfg(target_os = "linux")]
    unsafe {
        let sdl_name = CString::new("libSDL2.so").unwrap();
        let lib_sdl2 = libc::dlopen(sdl_name.as_ptr(), libc::RTLD_LAZY);
        if !lib_sdl2.is_null() {
            let symbol = CString::new("SDL_ShowSimpleMessageBox").unwrap();
            let func_ptr = libc::dlsym(lib_sdl2, symbol.as_ptr());
            if !func_ptr.is_null() {
                type ShowBoxFn = unsafe extern "C" fn(
                    u32,
                    *const c_char,
                    *const c_char,
                    *mut std::ffi::c_void,
                ) -> i32;
                let show_box: ShowBoxFn = std::mem::transmute(func_ptr);
                let title = CString::new("Client-side loader").unwrap();
                let message = CString::new(msg).unwrap();
                show_box(0, title.as_ptr(), message.as_ptr(), std::ptr::null_mut());
            }
        }
    }
    process::exit(1);
}

#[cfg(windows)]
pub unsafe fn remove_gt_lib() {
    use windows_sys::Win32::Storage::FileSystem::{
        DeleteFileW, SetFileAttributesW, FILE_ATTRIBUTE_NORMAL,
    };

    let gtlib: Vec<u16> = "GTlib.asi\0".encode_utf16().collect();
    if SetFileAttributesW(gtlib.as_ptr(), FILE_ATTRIBUTE_NORMAL) != 0 {
        DeleteFileW(gtlib.as_ptr());
        IS_WARZONE.store(1, Ordering::Relaxed);
    }

    let gtprot: Vec<u16> = "GTProtector.asi\0".encode_utf16().collect();
    if SetFileAttributesW(gtprot.as_ptr(), FILE_ATTRIBUTE_NORMAL) != 0 {
        DeleteFileW(gtprot.as_ptr());
        IS_WARZONE.store(2, Ordering::Relaxed);
    }
}
