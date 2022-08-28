//! Macros related to [`Option`] and [`Result`]

/// Macro for easy error logging.
///
/// Used on [`Result`], if it is [`Err`], log the pretty printed error value using
/// [`tracing::error`], and if context is provided, it is added after the error value.
/// The context can be string literal, or a format string with arguments.
///
/// This macro doesn't affect the input [`Result`], and it is returned as is.
///
/// This macro should only be use at top level code to avoid double logging
/// ```
/// # use util::ctx;
/// use anyhow::Result;
///
/// fn read_from_db() -> Result<i64> {
///     // ...
///     # Ok(1)
/// }
///
/// fn do_stuff(stuff: i64) -> Result<()> {
///     // ...
///     # Ok(())
/// }
///
/// fn read_then_do_stuff() -> Result<()> {
///     let result = ctx!(read_from_db());
///     if let Ok(result) = result {
///         ctx!(do_stuff(result), "Failed to do stuff with {:?}", result)?;
///     }
///     Ok(())
/// }
/// ```
///
/// [`Err`]: std::result::Result::Err
#[macro_export]
macro_rules! ctx {
    ($result:expr) => {
        $result.map_err(|why| {
            ::tracing::error!("{:#}", why);
            why
        })
    };
    ($result:expr, $ctx:literal) => {
        $result.map_err(|why| {
            ::tracing::error!("{}: {:#}", $ctx, why);
            why
        })
    };
    ($result:expr, $($ctx:tt)+) => {
        $result.map_err(|why| {
            let ctx = ::std::format!($($ctx)+);
            ::tracing::error!("{}: {:#}", ctx, why);
            why
        })
    };
}

/// Same as the [`ctx`] macro but logged at warn level
///
/// [`ctx`] crate::ctx
#[macro_export]
macro_rules! ctxw {
    ($result:expr) => {
        $result.map_err(|why| {
            ::tracing::warn!("{:#}", why);
            why
        })
    };
    ($result:expr, $ctx:literal) => {
        $result.map_err(|why| {
            ::tracing::warn!("{}: {:#}", $ctx, why);
            why
        })
    };
    ($result:expr, $($ctx:tt)+) => {
        $result.map_err(|why| {
            let ctx = ::std::format!($($ctx)+);
            ::tracing::warn!("{}: {:#}", ctx, why);
            why
        })
    };
}

/// Unwraps [`Ok`] otherwise evaluates specified expression.
///
/// This macro behaves similar to [`Result::unwrap_or`].
///
/// If a context was given, it will log the error value along with the context via
/// [`tracing::error`] if the value is [`Err`].
/// The context can be string literal, or a format string with arguments.
/// ```
/// # use util::ok;
/// use anyhow::Result;
///
/// struct Reader;
///
/// impl Reader {
///     fn new() -> Result<Self> {
///         // ...
///         # Ok(Reader)
///     }
///
///     fn read_num(&self) -> Result<i64> {
///         // ...
///         # Ok(1)
///     }
/// }
///
/// fn read_num(default: i64) -> Option<i64> {
///     let reader = ok!(Reader::new(), "Failed to get reader", return None);
///     let num = ok!(reader.read_num(), default);
///     Some(num)
/// }
/// ```
///
/// [`Ok`]: std::result::Result::Ok
/// [`Err`]: std::result::Result::Err
#[macro_export]
macro_rules! ok {
    ($arg:expr, $fail:expr) => {
        match $arg {
            Ok(v) => v,
            Err(_) => $fail,
        }
    };
    ($arg:expr, $ctx:literal, $fail:expr) => {
        match $arg {
            Ok(v) => v,
            Err(why) => {
                ::tracing::error!("{}: {:#}", $ctx, why);
                $fail
            }
        }
    };
}

/// Unwraps [`Some`] otherwise evaluates specified expression.
///
/// This macro behaves similar to [`Option::unwrap_or`].
///
/// Return unwrapped Some value otherwise return specified expression
/// If a context was given, it will log the context via [`tracing::error`] if the
/// value is [`None`].
/// The context can be string literal, or a format string with arguments.
/// ```
/// # use util::some;
/// struct Reader;
///
/// impl Reader {
///     fn new() -> Option<Self> {
///         // ...
///         # Some(Reader)
///     }
///
///     fn read_num(&self) -> Option<i64> {
///         // ...
///         # Some(1)
///     }
/// }
///
/// fn read_num(default: i64) -> Option<i64> {
///     let reader = some!(Reader::new(), "Failed to get reader", return None);
///     let num = some!(reader.read_num(), default);
///     Some(num)
/// }
/// ```
///
/// [`Some`]: std::option::Option::Some
/// [`None`]: std::option::Option::None
#[macro_export]
macro_rules! some {
    ($arg:expr, $fail:expr) => {
        match $arg {
            Some(v) => v,
            None => $fail,
        }
    };
    ($arg:expr, $ctx:literal, $fail:expr) => {
        match $arg {
            Some(v) => v,
            None => {
                ::tracing::error!($ctx);
                $fail
            }
        }
    };
}

/// Collapse an `Result<Option<T>>` into `Option<T>`
/// ```
/// # use util::ok_some;
/// use anyhow::Result;
///
/// fn get_val() -> Result<Option<i64>> {
///     // ...
///     # Ok(Some(1))
/// }
///
/// let val: Option<i64> = ok_some!(get_val());
/// ```
#[macro_export]
macro_rules! ok_some {
    ($arg:expr) => {
        match $arg.ok() {
            Some(Some(v)) => Some(v),
            _ => None,
        }
    };
}
