#![allow(non_snake_case)]
#![allow(clippy::missing_safety_doc)]

pub mod engine;
pub mod game;
pub mod hud;
pub mod render;
pub mod utils;

pub use engine::*;
pub use game::*;
pub use hud::crosshair as hud_crosshair;
pub use hud::*;
pub use render::*;
pub use utils::*;

use std::ffi::c_char;

#[repr(transparent)]
pub struct ProgramVersionPtr(pub *const c_char);
unsafe impl Sync for ProgramVersionPtr {}

#[no_mangle]
pub static PROGRAM_VERSION: ProgramVersionPtr = ProgramVersionPtr(c"\ncsldr version:rust".as_ptr());
