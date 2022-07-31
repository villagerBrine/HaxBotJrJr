//! General utilities
pub mod discord;
pub mod imp;
pub mod string;
pub mod tri;

#[macro_export]
/// Given numbers a and b, return (a / b, a % b)
macro_rules! div_rem {
    ($a:expr, $b:expr) => {
        ($a / $b, $a % $b)
    };
}

#[macro_export]
/// Create an io::Error wrapped in Err
macro_rules! ioerr {
    ($msg:literal) => {
        Err(std::io::Error::new(std::io::ErrorKind::Other, $msg).into())
    };
    ($($msg:tt)+) => {
        Err(std::io::Error::new(std::io::ErrorKind::Other, format!($($msg)+)).into())
    };
    ($kind:ident, $msg:literal) => {
        Err(std::io::Error::new(std::io::ErrorKind::$kind, $msg).into())
    };
    ($kind:ident, $($msg:tt)+) => {
        Err(std::io::Error::new(std::io::ErrorKind::$kind, format!($($msg)+)).into())
    };
}
