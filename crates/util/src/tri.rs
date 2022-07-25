/// Macros related to Option and Result

#[macro_export]
/// Same as the try macro but with optional context.
macro_rules! tri {
    ($val:expr) => {$val?};
    ($val:expr, $ctx:literal) => {$val.context($ctx)?};
    ($val:expr, $($ctx:tt)+) => {$val.context(format!($($ctx)+))?};
}

#[macro_export]
/// Attach a context to an Result that is logged at error level alongside the Err value
/// This macro should only be use at top level code to avoid double logging
macro_rules! ctx {
    ($result:expr) => {
        $result.map_err(|why| {
            tracing::error!("{:#}", why);
            why
        })
    };
    ($result:expr, $ctx:literal) => {
        $result.map_err(|why| {
            tracing::error!("{}: {:#}", $ctx, why);
            why
        })
    };
    ($result:expr, $($ctx:tt)+) => {
        $result.map_err(|why| {
            let ctx = format!($($ctx)+);
            tracing::error!("{}: {:#}", ctx, why);
            why
        })
    };
}

#[macro_export]
/// Return unwrapped Ok value otherwise return specified expression
/// If a context was given, it is the same as ok!(ctx!(..), ..)
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
                tracing::error!("{}: {:#}", $ctx, why);
                $fail
            }
        }
    };
}

#[macro_export]
/// Return unwrapped Some value otherwise return specified expression
/// If a context was given, it is the same as some!(ctx!(..), ..)
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
                tracing::error!($ctx);
                $fail
            }
        }
    };
}

#[macro_export]
/// Collapse an Option<Option<T>> into Option<T>
macro_rules! some2 {
    ($arg:expr) => {
        match $arg.ok() {
            Some(Some(v)) => Some(v),
            _ => None,
        }
    };
}
