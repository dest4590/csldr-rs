use std::os::raw::c_int;

use crate::engine::cdll_int::{G_ENGFUNCS, SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::render::gl::*;

#[macro_export]
macro_rules! with_gl_2d {
    ($body:block) => {{
        let sw = crate::engine::cdll_int::SCREEN_WIDTH.load(std::sync::atomic::Ordering::Relaxed) as f64;
        let sh = crate::engine::cdll_int::SCREEN_HEIGHT.load(std::sync::atomic::Ordering::Relaxed) as f64;

        gl_matrix_mode(GL_PROJECTION);
        gl_push_matrix();
        gl_load_identity();
        gl_ortho(0.0, sw, sh, 0.0, -1.0, 1.0);

        gl_matrix_mode(GL_MODELVIEW);
        gl_push_matrix();
        gl_load_identity();

        gl_disable(GL_TEXTURE_2D);
        gl_disable(GL_DEPTH_TEST);
        gl_enable(GL_BLEND);
        gl_blend_func(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

        $body

        gl_disable(GL_BLEND);
        gl_enable(GL_TEXTURE_2D);
        gl_pop_matrix();
        gl_matrix_mode(GL_PROJECTION);
        gl_pop_matrix();
        gl_matrix_mode(GL_MODELVIEW);
    }};
}

#[macro_export]
macro_rules! draw_primitive_2d {
    ($mode:expr, $color:expr, $( ($x:expr, $y:expr) ),+ $(,)?) => {{
        let (r, g, b, a) = $color;
        gl_color4f(r, g, b, a);
        gl_begin($mode);
        $(
            gl_vertex2f($x as f32, $y as f32);
        )+
        gl_end();
    }};
}

#[repr(C)]
pub struct TriangleAPI {
    pub version: c_int,
    pub RenderMode: Option<unsafe extern "C" fn(c_int)>,
    pub Begin: Option<unsafe extern "C" fn(c_int)>,
    pub End: Option<unsafe extern "C" fn()>,
    pub Color4f: Option<unsafe extern "C" fn(f32, f32, f32, f32)>,
    pub Color4ub: Option<unsafe extern "C" fn(u8, u8, u8, u8)>,
    pub TexCoord2f: Option<unsafe extern "C" fn(f32, f32)>,
    pub Vertex3fv: Option<unsafe extern "C" fn(*const f32)>,
    pub Vertex3f: Option<unsafe extern "C" fn(f32, f32, f32)>,
    pub Brightness: Option<unsafe extern "C" fn(f32)>,
    pub CullFace: Option<unsafe extern "C" fn(c_int)>,
    pub SpriteTexture: Option<unsafe extern "C" fn(*mut std::ffi::c_void, c_int) -> c_int>,
    pub WorldToScreen: Option<unsafe extern "C" fn(*const f32, *mut f32) -> c_int>,
    pub Fog: Option<unsafe extern "C" fn(*const f32, f32, f32, c_int)>,
    pub ScreenToWorld: Option<unsafe extern "C" fn(*const f32, *mut f32)>,
    pub GetMatrix: Option<unsafe extern "C" fn(c_int, *mut f32)>,
    pub BoxInPVS: Option<unsafe extern "C" fn(*const f32, *const f32) -> c_int>,
    pub LightAtPoint: Option<unsafe extern "C" fn(*const f32, *mut f32)>,
    pub Color4fRendermode: Option<unsafe extern "C" fn(f32, f32, f32, f32, c_int)>,
    pub FogParams: Option<unsafe extern "C" fn(f32, c_int)>,
}

pub unsafe fn world_to_screen(world_pos: [f32; 3]) -> Option<[f32; 2]> {
    if G_ENGFUNCS.pTriAPI.is_null() {
        return None;
    }
    let tri = &*(G_ENGFUNCS.pTriAPI as *const TriangleAPI);
    let w2s = tri.WorldToScreen?;

    let mut screen_pos = [0.0f32; 2];
    let res = w2s(world_pos.as_ptr(), screen_pos.as_mut_ptr());
    if res != 0 {
        return None;
    }

    let sw = SCREEN_WIDTH.load(std::sync::atomic::Ordering::Relaxed) as f32;
    let sh = SCREEN_HEIGHT.load(std::sync::atomic::Ordering::Relaxed) as f32;

    let x = (screen_pos[0] + 1.0) * 0.5 * sw;
    let y = (1.0 - screen_pos[1]) * 0.5 * sh;

    if x >= 0.0 && x <= sw && y >= 0.0 && y <= sh {
        Some([x, y])
    } else {
        None
    }
}

pub unsafe fn draw_rect_2d(x: f32, y: f32, w: f32, h: f32, r: f32, g: f32, b: f32, a: f32) {
    with_gl_2d!({
        draw_primitive_2d!(
            GL_TRIANGLE_STRIP,
            (r, g, b, a),
            (x, y),
            (x, y + h),
            (x + w, y),
            (x + w, y + h)
        );
    });
}

pub unsafe fn draw_rect_outline_2d(x: f32, y: f32, w: f32, h: f32, r: f32, g: f32, b: f32, a: f32) {
    with_gl_2d!({
        draw_primitive_2d!(
            GL_LINE_LOOP,
            (r, g, b, a),
            (x, y),
            (x + w, y),
            (x + w, y + h),
            (x, y + h)
        );
    });
}

static FONT_5X7: [[u8; 5]; 95] = [
    [0x00, 0x00, 0x00, 0x00, 0x00], // 32 ' '
    [0x00, 0x00, 0x5F, 0x00, 0x00], // 33 '!'
    [0x00, 0x07, 0x00, 0x07, 0x00], // 34 '"'
    [0x14, 0x7F, 0x14, 0x7F, 0x14], // 35 '#'
    [0x24, 0x2A, 0x7F, 0x2A, 0x12], // 36 '$'
    [0x23, 0x13, 0x08, 0x64, 0x62], // 37 '%'
    [0x36, 0x49, 0x55, 0x22, 0x50], // 38 '&'
    [0x00, 0x05, 0x03, 0x00, 0x00], // 39 '\''
    [0x00, 0x1C, 0x22, 0x41, 0x00], // 40 '('
    [0x00, 0x41, 0x22, 0x1C, 0x00], // 41 ')'
    [0x14, 0x08, 0x3E, 0x08, 0x14], // 42 '*'
    [0x08, 0x08, 0x3E, 0x08, 0x08], // 43 '+'
    [0x00, 0x50, 0x30, 0x00, 0x00], // 44 ','
    [0x08, 0x08, 0x08, 0x08, 0x08], // 45 '-'
    [0x00, 0x60, 0x60, 0x00, 0x00], // 46 '.'
    [0x20, 0x10, 0x08, 0x04, 0x02], // 47 '/'
    [0x3E, 0x51, 0x49, 0x45, 0x3E], // 48 '0'
    [0x00, 0x42, 0x7F, 0x40, 0x00], // 49 '1'
    [0x42, 0x61, 0x51, 0x49, 0x46], // 50 '2'
    [0x21, 0x41, 0x45, 0x4B, 0x31], // 51 '3'
    [0x18, 0x14, 0x12, 0x7F, 0x10], // 52 '4'
    [0x27, 0x45, 0x45, 0x45, 0x39], // 53 '5'
    [0x3C, 0x4A, 0x49, 0x49, 0x30], // 54 '6'
    [0x01, 0x71, 0x09, 0x05, 0x03], // 55 '7'
    [0x36, 0x49, 0x49, 0x49, 0x36], // 56 '8'
    [0x06, 0x49, 0x49, 0x29, 0x1E], // 57 '9'
    [0x00, 0x36, 0x36, 0x00, 0x00], // 58 ':'
    [0x00, 0x56, 0x36, 0x00, 0x00], // 59 ';'
    [0x08, 0x14, 0x22, 0x41, 0x00], // 60 '<'
    [0x14, 0x14, 0x14, 0x14, 0x14], // 61 '='
    [0x00, 0x41, 0x22, 0x14, 0x08], // 62 '>'
    [0x02, 0x01, 0x51, 0x09, 0x06], // 63 '?'
    [0x32, 0x49, 0x79, 0x41, 0x3E], // 64 '@'
    [0x7E, 0x11, 0x11, 0x11, 0x7E], // 65 'A'
    [0x7F, 0x49, 0x49, 0x49, 0x36], // 66 'B'
    [0x3E, 0x41, 0x41, 0x41, 0x22], // 67 'C'
    [0x7F, 0x41, 0x41, 0x22, 0x1C], // 68 'D'
    [0x7F, 0x49, 0x49, 0x49, 0x41], // 69 'E'
    [0x7F, 0x09, 0x09, 0x09, 0x01], // 70 'F'
    [0x3E, 0x41, 0x49, 0x49, 0x7A], // 71 'G'
    [0x7F, 0x08, 0x08, 0x08, 0x7F], // 72 'H'
    [0x00, 0x41, 0x7F, 0x41, 0x00], // 73 'I'
    [0x20, 0x40, 0x41, 0x3F, 0x01], // 74 'J'
    [0x7F, 0x08, 0x14, 0x22, 0x41], // 75 'K'
    [0x7F, 0x40, 0x40, 0x40, 0x40], // 76 'L'
    [0x7F, 0x02, 0x0C, 0x02, 0x7F], // 77 'M'
    [0x7F, 0x04, 0x08, 0x10, 0x7F], // 78 'N'
    [0x3E, 0x41, 0x41, 0x41, 0x3E], // 79 'O'
    [0x7F, 0x09, 0x09, 0x09, 0x06], // 80 'P'
    [0x3E, 0x41, 0x51, 0x21, 0x5E], // 81 'Q'
    [0x7F, 0x09, 0x19, 0x29, 0x46], // 82 'R'
    [0x46, 0x49, 0x49, 0x49, 0x31], // 83 'S'
    [0x01, 0x01, 0x7F, 0x01, 0x01], // 84 'T'
    [0x3F, 0x40, 0x40, 0x40, 0x3F], // 85 'U'
    [0x1F, 0x20, 0x40, 0x20, 0x1F], // 86 'V'
    [0x3F, 0x40, 0x38, 0x40, 0x3F], // 87 'W'
    [0x63, 0x14, 0x08, 0x14, 0x63], // 88 'X'
    [0x07, 0x08, 0x70, 0x08, 0x07], // 89 'Y'
    [0x61, 0x51, 0x49, 0x45, 0x43], // 90 'Z'
    [0x00, 0x7F, 0x41, 0x41, 0x00], // 91 '['
    [0x02, 0x04, 0x08, 0x10, 0x20], // 92 '\'
    [0x00, 0x41, 0x41, 0x7F, 0x00], // 93 ']'
    [0x04, 0x02, 0x01, 0x02, 0x04], // 94 '^'
    [0x40, 0x40, 0x40, 0x40, 0x40], // 95 '_'
    [0x00, 0x01, 0x02, 0x04, 0x00], // 96 '`'
    [0x20, 0x54, 0x54, 0x54, 0x78], // 97 'a'
    [0x7F, 0x48, 0x44, 0x44, 0x38], // 98 'b'
    [0x38, 0x44, 0x44, 0x44, 0x20], // 99 'c'
    [0x38, 0x44, 0x44, 0x48, 0x7F], // 100 'd'
    [0x38, 0x54, 0x54, 0x54, 0x18], // 101 'e'
    [0x08, 0x7E, 0x09, 0x01, 0x02], // 102 'f'
    [0x0C, 0x52, 0x52, 0x52, 0x3E], // 103 'g'
    [0x7F, 0x08, 0x04, 0x04, 0x78], // 104 'h'
    [0x00, 0x44, 0x7D, 0x40, 0x00], // 105 'i'
    [0x20, 0x40, 0x44, 0x3D, 0x00], // 106 'j'
    [0x7F, 0x10, 0x28, 0x44, 0x00], // 107 'k'
    [0x00, 0x41, 0x7F, 0x40, 0x00], // 108 'l'
    [0x7C, 0x04, 0x18, 0x04, 0x78], // 109 'm'
    [0x7C, 0x08, 0x04, 0x04, 0x78], // 110 'n'
    [0x38, 0x44, 0x44, 0x44, 0x38], // 111 'o'
    [0x7C, 0x14, 0x14, 0x14, 0x08], // 112 'p'
    [0x08, 0x14, 0x14, 0x18, 0x7C], // 113 'q'
    [0x7C, 0x08, 0x04, 0x04, 0x08], // 114 'r'
    [0x48, 0x54, 0x54, 0x54, 0x20], // 115 's'
    [0x04, 0x3E, 0x44, 0x40, 0x20], // 116 't'
    [0x3C, 0x40, 0x40, 0x20, 0x7C], // 117 'u'
    [0x1C, 0x20, 0x40, 0x20, 0x1C], // 118 'v'
    [0x3C, 0x40, 0x30, 0x40, 0x3C], // 119 'w'
    [0x44, 0x28, 0x10, 0x28, 0x44], // 120 'x'
    [0x0C, 0x50, 0x50, 0x50, 0x3C], // 121 'y'
    [0x44, 0x64, 0x54, 0x4C, 0x44], // 122 'z'
    [0x08, 0x1C, 0x36, 0x41, 0x00], // 123 '{'
    [0x00, 0x00, 0x7F, 0x00, 0x00], // 124 '|'
    [0x00, 0x41, 0x36, 0x1C, 0x08], // 125 '}'
    [0x08, 0x04, 0x08, 0x10, 0x08], // 126 '~'
];

pub unsafe fn get_text_width(text: &str) -> f32 {
    text.len() as f32 * 6.0
}

pub unsafe fn draw_custom_text_2d(
    x: f32,
    y: f32,
    text: &str,
    r: f32,
    g: f32,
    b: f32,
    a: f32,
    scale: f32,
) {
    let char_w = (6.0 * scale).floor();
    let start_x = x.floor();
    let start_y = y.floor();

    with_gl_2d!({
        gl_color4f(0.0, 0.0, 0.0, a * 0.85);
        gl_begin(GL_QUADS);
        for (idx, ch) in text.chars().enumerate() {
            let cx = start_x + (idx as f32) * char_w;
            let byte_idx = if (32..=126).contains(&(ch as u8)) {
                (ch as usize) - 32
            } else {
                0
            };
            let cols = FONT_5X7[byte_idx];
            for (col_idx, &col) in cols.iter().enumerate() {
                let px = (cx + (col_idx as f32) * scale + 1.0).floor();
                let pw = (px + scale).floor();
                for bit in 0..7 {
                    if (col & (1 << bit)) != 0 {
                        let py = (start_y + (bit as f32) * scale + 1.0).floor();
                        let ph = (py + scale).floor();
                        gl_vertex2f(px, py);
                        gl_vertex2f(pw, py);
                        gl_vertex2f(pw, ph);
                        gl_vertex2f(px, ph);
                    }
                }
            }
        }
        gl_end();

        gl_color4f(r, g, b, a);
        gl_begin(GL_QUADS);
        for (idx, ch) in text.chars().enumerate() {
            let cx = start_x + (idx as f32) * char_w;
            let byte_idx = if (32..=126).contains(&(ch as u8)) {
                (ch as usize) - 32
            } else {
                0
            };
            let cols = FONT_5X7[byte_idx];
            for (col_idx, &col) in cols.iter().enumerate() {
                let px = (cx + (col_idx as f32) * scale).floor();
                let pw = (px + scale).floor();
                for bit in 0..7 {
                    if (col & (1 << bit)) != 0 {
                        let py = (start_y + (bit as f32) * scale).floor();
                        let ph = (py + scale).floor();
                        gl_vertex2f(px, py);
                        gl_vertex2f(pw, py);
                        gl_vertex2f(pw, ph);
                        gl_vertex2f(px, ph);
                    }
                }
            }
        }
        gl_end();
    });
}

pub unsafe fn draw_text_2d(x: i32, y: i32, text: &str, r: f32, g: f32, b: f32) {
    draw_custom_text_2d(x as f32, y as f32, text, r, g, b, 0.95, 1.0);
}
