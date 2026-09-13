use std::ffi::CString;

pub fn con_print_str(s: &str) {
    unsafe {
        if let Some(con_printf) = crate::engine::cdll_int::G_ENGFUNCS.Con_Printf {
            let clean = if s.contains('\0') {
                s.replace('\0', "")
            } else {
                s.to_string()
            };
            if let Ok(c_msg) = CString::new(clean) {
                con_printf(c"%s".as_ptr(), c_msg.as_ptr());
            }
        } else {
            eprint!("{s}");
        }
    }
}

#[macro_export]
macro_rules! con_print {
    ($($arg:tt)*) => {
        $crate::utils::console::con_print_str(&format!($($arg)*))
    };
}

#[macro_export]
macro_rules! con_println {
    () => {
        $crate::utils::console::con_print_str("\n")
    };
    ($($arg:tt)*) => {
        $crate::utils::console::con_print_str(&format!("{}\n", format_args!($($arg)*)))
    };
}

#[macro_export]
macro_rules! console_print {
    ($($arg:tt)*) => {
        $crate::con_print!($($arg)*)
    };
}

#[macro_export]
macro_rules! console_println {
    ($($arg:tt)*) => {
        $crate::con_println!($($arg)*)
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_console_macros() {
        con_print!("Test con_print without newline");
        con_println!("Test con_println with args: {}", 123);
        console_print!("Test console_print alias");
        console_println!("Test console_println alias: {}", "abc");
    }
}
