pub fn con_print_str(s: &str) {
    let clean = if s.contains('\0') {
        s.replace('\0', "")
    } else {
        s.to_string()
    };
    if let (Ok(fmt), Ok(c_msg)) = (std::ffi::CString::new("%s"), std::ffi::CString::new(clean)) {
        unsafe {
            if let Some(con_printf) = crate::engine::cdll_int::G_ENGFUNCS.Con_Printf {
                con_printf(fmt.as_ptr(), c_msg.as_ptr());
            }
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

#[cfg(test)]
mod tests {
    #[test]
    fn test_console_macros() {
        con_print!("Test con_print without newline");
        con_println!("Test con_println with args: {}", 123);
    }
}
