#[macro_export]
macro_rules! call_engine {
    ($func:expr, $sig:ty $(, $arg:expr)*) => {
        if !$func.is_null() {
            let f: $sig = std::mem::transmute($func);
            f($($arg),*);
        }
    };
}

#[macro_export]
macro_rules! mutex_lock_or {
    ($mutex:expr, $default:expr $(, |$v:ident| $body:expr)?) => {
        match $mutex.lock() {
            Ok($($v)?) => $($body)?
            Err(_) => $default,
        }
    };
}
