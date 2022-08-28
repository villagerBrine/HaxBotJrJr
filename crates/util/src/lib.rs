//! General utilities
#[warn(missing_docs, missing_debug_implementations)]
pub mod discord;
pub mod imp;
pub mod string;
pub mod tri;

/// Given numbers a and b, return (a / b, a % b)
/// ```
/// # use util::div_rem;
/// assert!(div_rem!(21, 10) == (2, 1));
/// ```
#[macro_export]
macro_rules! div_rem {
    ($a:expr, $b:expr) => {
        ($a / $b, $a % $b)
    };
}

/// Create an [`io::Error`] wrapped in [`Result::Err`].
///
/// Takes the name of the [`io::ErrorKind`] variant, and then the message.
/// The error message can be a literal, or a format string with arguments.
/// If the [`io::ErrorKind`] variant name is omitted, then [`io::ErrorKind::Other`] is used.
/// ```
/// # use util::ioerr;
/// use anyhow::Result;
///
/// fn read_db_for_password(username: &str) -> Result<Option<String>> {
///     // ...
///     # Ok(Some(String::new()))
/// }
///
/// fn get_password(username: &str) -> Result<String> {
///     match read_db_for_password(username) {
///         Ok(pw) => match pw {
///             Some(pw) => Ok(pw),
///             None => ioerr!(NotFound, "Password not found")
///         },
///         // `ErrorKind::Other` by default
///         Err(why) => ioerr!("Database error: {:#}", why)
///     }
/// }
/// ```
///
/// [`io::Error`]: std::io::Error
/// [`io::ErrorKind`]: std::io::ErrorKind
/// [`io::ErrorKind::Other`]: std::io::ErrorKind::Other
#[macro_export]
macro_rules! ioerr {
    ($msg:literal) => {
        Err(::std::io::Error::new(::std::io::ErrorKind::Other, $msg).into())
    };
    ($kind:ident, $msg:literal) => {
        Err(::std::io::Error::new(::std::io::ErrorKind::$kind, $msg).into())
    };
    ($kind:ident, $($msg:tt)+) => {
        Err(::std::io::Error::new(::std::io::ErrorKind::$kind, format!($($msg)+)).into())
    };
    ($($msg:tt)+) => {
        Err(::std::io::Error::new(::std::io::ErrorKind::Other, format!($($msg)+)).into())
    };
}
