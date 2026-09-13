use crate::sdk::{CVar, ClEngineFuncs};
use std::ffi::CString;
use std::sync::atomic::{AtomicPtr, Ordering};

static mut ENGFUNCS: *const ClEngineFuncs = std::ptr::null();

pub unsafe fn set_engfuncs(ptr: *const ClEngineFuncs) {
    ENGFUNCS = ptr;
}

fn engfuncs() -> &'static ClEngineFuncs {
    unsafe { &*ENGFUNCS }
}

pub struct CVarRef {
    ptr: AtomicPtr<CVar>,
}

impl CVarRef {
    pub const fn new() -> Self {
        Self {
            ptr: AtomicPtr::new(std::ptr::null_mut()),
        }
    }

    pub fn set(&self, ptr: *mut CVar) {
        self.ptr.store(ptr, Ordering::Relaxed);
    }

    pub fn get_ptr(&self) -> *mut CVar {
        self.ptr.load(Ordering::Relaxed)
    }

    pub fn value(&self) -> f32 {
        let p = self.get_ptr();
        if !p.is_null() {
            unsafe { (*p).value }
        } else {
            0.0
        }
    }

    pub fn value_or(&self, default: f32) -> f32 {
        let p = self.get_ptr();
        if !p.is_null() {
            unsafe { (*p).value }
        } else {
            default
        }
    }

    pub fn enabled(&self) -> bool {
        self.value() != 0.0
    }
}

impl Default for CVarRef {
    fn default() -> Self {
        Self::new()
    }
}

pub fn get_cvar_float(name: &str, fallback_ref: &CVarRef) -> f32 {
    unsafe {
        let ef = engfuncs();
        if let Some(get_float) = ef.pfnGetCvarFloat {
            if let Ok(c_name) = CString::new(name) {
                let val = get_float(c_name.as_ptr());
                if val != 0.0 {
                    return val;
                }
            }
        }
    }
    fallback_ref.value()
}

unsafe fn register_cvar(name: &str, default_val: &str, target: &CVarRef) {
    let ef = engfuncs();
    if let Some(reg_fn) = ef.pfnRegisterVariable {
        let c_name = Box::leak(CString::new(name).unwrap().into_boxed_c_str());
        let c_val = Box::leak(CString::new(default_val).unwrap().into_boxed_c_str());
        let ptr = reg_fn(c_name.as_ptr(), c_val.as_ptr(), 1);
        target.set(ptr);
    }
}

unsafe fn get_engine_cvar(name: &str, target: &CVarRef) {
    let ef = engfuncs();
    if let Some(get_ptr_fn) = ef.pfnGetCvarPointer {
        let c_name = CString::new(name).unwrap();
        let ptr = get_ptr_fn(c_name.as_ptr());
        target.set(ptr);
    }
}

macro_rules! cvar_table {
    ($(
        $kind:ident($($arg:tt),*)
    );* $(;)?) => {
        $(
            cvar_table!(@static $kind($($arg),*));
        )*

        pub unsafe fn init_cvars() {
            $(
                cvar_table!(@init $kind($($arg),*));
            )*
        }
    };


    (@static register($_name:expr, $_default:expr, $target:ident)) => {
        pub static $target: CVarRef = CVarRef::new();
    };

    (@static get($_name:expr, $target:ident)) => {
        pub static $target: CVarRef = CVarRef::new();
    };


    (@init register($name:expr, $default:expr, $target:ident)) => {
        register_cvar($name, $default, &$target);
    };

    (@init get($name:expr, $target:ident)) => {
        get_engine_cvar($name, &$target);
    };
}

cvar_table! {
    register("viewmodel_fov",            "90",    VIEWMODEL_FOV);
    register("viewmodel_shift",          "0",     VIEWMODEL_SHIFT);
    register("viewmodel_offset_x",       "0",     VIEWMODEL_OFFSET_X);
    register("viewmodel_offset_y",       "0",     VIEWMODEL_OFFSET_Y);
    register("viewmodel_offset_z",       "0",     VIEWMODEL_OFFSET_Z);
    register("viewmodel_hands",          "",      VIEWMODEL_HANDS);
    register("cl_bobstyle",              "0",     CL_BOBSTYLE);
    register("cl_bobamt_vert",           "0.13",  CL_BOBAMT_VERT);
    register("cl_bobamt_lat",            "0.32",  CL_BOBAMT_LAT);
    register("cl_bob_lower_amt",         "8",     CL_BOB_LOWER_AMT);
    register("cl_bob_camera",            "1",     CL_BOB_CAMERA);
    register("cl_rollangle",             "0",     CL_ROLLANGLE);
    register("cl_rollspeed",             "200",   CL_ROLLSPEED);
    register("viewmodel_lag_style",      "0",     VIEWMODEL_LAG_STYLE);
    register("viewmodel_lag_scale",      "1.0",   VIEWMODEL_LAG_SCALE);
    register("viewmodel_lag_speed",      "8.0",   VIEWMODEL_LAG_SPEED);
    register("fov_horplus",              "1",     FOV_HORPLUS);
    register("fov_lerp",                 "0",     FOV_LERP);
    register("camera_movement_scale",    "1",     CAMERA_MOVEMENT_SCALE);
    register("camera_movement_interp",   "0",     CAMERA_MOVEMENT_INTERP);
    register("xhair_enable",             "0",     XHAIR_ENABLE);
    register("xhair_gap",                "0",     XHAIR_GAP);
    register("xhair_size",               "4",     XHAIR_SIZE);
    register("xhair_thick",              "0",     XHAIR_THICK);
    register("xhair_pad",                "0",     XHAIR_PAD);
    register("xhair_dot",                "0",     XHAIR_DOT);
    register("xhair_t",                  "0",     XHAIR_T);
    register("xhair_dynamic_scale",      "0",     XHAIR_DYNAMIC_SCALE);
    register("xhair_gap_useweaponvalue", "0",     XHAIR_GAP_USEWEAPONVALUE);
    register("xhair_dynamic_move",       "1",     XHAIR_DYNAMIC_MOVE);
    register("xhair_color_r",            "0",     XHAIR_COLOR_R);
    register("xhair_color_g",            "1",     XHAIR_COLOR_G);
    register("xhair_color_b",            "0",     XHAIR_COLOR_B);
    register("xhair_alpha",              "1",     XHAIR_ALPHA);
    register("xhair_additive",           "0",     XHAIR_ADDITIVE);
    register("motion_blur_enable",       "0",     MOTION_BLUR_ENABLE);
    register("motion_blur_intensity",    "1.0",   MOTION_BLUR_INTENSITY);
    register("motion_blur_falling",      "1",     MOTION_BLUR_FALLING);
    register("hrtf_enable",              "1",     HRTF_ENABLE);
    register("hrtf_volume",              "1.0",   HRTF_VOLUME);
    register("hrtf_debug",               "0",     HRTF_DEBUG);
    register("hrtf_debug_rays",          "0",     HRTF_DEBUG_RAYS);
    register("hrtf_acoustics",           "1",     HRTF_ACOUSTICS);
    register("hrtf_reverb_scale",        "1.0",   HRTF_REVERB_SCALE);
    register("hrtf_occlusion",           "1",     HRTF_OCCLUSION);
    register("hrtf_freecam",             "0",     HRTF_FREECAM);
    register("hrtf_freecam_speed",       "500",   HRTF_FREECAM_SPEED);
    register("movement_gui_enable",      "1",     MOVEMENT_GUI_ENABLE);
    register("movement_gui_keys",        "0",     MOVEMENT_GUI_KEYS);
    register("movement_gui_speed",       "1",     MOVEMENT_GUI_SPEED);
    register("movement_gui_lj",          "1",     MOVEMENT_GUI_LJ);
    register("movement_gui_graph",       "1",     MOVEMENT_GUI_GRAPH);
    register("movement_gui_graph_w",     "220",   MOVEMENT_GUI_GRAPH_W);
    register("movement_gui_graph_h",     "45",    MOVEMENT_GUI_GRAPH_H);
    register("movement_gui_pos_x",       "0.5",   MOVEMENT_GUI_POS_X);
    register("movement_gui_pos_y",       "0.75",  MOVEMENT_GUI_POS_Y);
    register("movement_gui_color_r",     "0.0",   MOVEMENT_GUI_COLOR_R);
    register("movement_gui_color_g",     "1.0",   MOVEMENT_GUI_COLOR_G);
    register("movement_gui_color_b",     "0.8",   MOVEMENT_GUI_COLOR_B);
    register("movement_gui_bg",          "1",     MOVEMENT_GUI_BG);

    get("cl_bobcycle",                CL_BOBCYCLE);
    get("cl_bobup",                   CL_BOBUP);
    get("cl_bob",                     CL_BOB);
    get("cl_crosshair_color",         CL_CROSSHAIR_COLOR);
    get("cl_crosshair_translucent",   CL_CROSSHAIR_TRANSLUCENT);
    get("hud_draw",                   HUD_DRAW);
    get("spec_pip",                   SPEC_PIP);
    get("cl_lw",                      CL_LW);
    get("cl_righthand",               CL_RIGHTHAND);
}
