//! Utility functions for commands
pub mod arg;
pub mod db;
pub mod discord;
pub mod macros;

/// Wraps `T`, the `Terminate` variant signals the calling command that it should terminate.
pub enum Terminator<T> {
    Proceed(T),
    Terminate,
}

#[macro_export]
/// Unwraps `Proceed`, and terminates the command if it is `Terminate`
macro_rules! t {
    ($t_result:expr) => {
        match $t_result {
            $crate::util::Terminator::Proceed(v) => v,
            $crate::util::Terminator::Terminate => return Ok(()),
        }
    };
    (? $t_result:expr) => {
        match $t_result {
            $crate::util::Terminator::Proceed(v) => v,
            $crate::util::Terminator::Terminate => return $crate::util::Terminator::Terminate,
        }
    };
}

#[macro_export]
/// Same as `ctx!(...)?` but returns `Terminator::Terminate` instead of `Err`
macro_rules! ttry {
    ($result:expr) => {
        match util::ctx!($result) {
            Ok(v) => v,
            Err(_) => return $crate::util::Terminator::Terminate,
        }
    };
    ($result:expr, $ctx:literal) => {
        match util::ctx!($result, $ctx) {
            Ok(v) => v,
            Err(_) => return $crate::util::Terminator::Terminate,
        }
    };
    ($result:expr, $($ctx:tt)+) => {
        match util::ctx!($result, $($ctx)+) {
            Ok(v) => v,
            Err(_) => return $crate::util::Terminator::Terminate,
        }
    };
}

#[macro_export]
/// Same as `finish` but returns `Terminator::Terminate`
macro_rules! tfinish {
    ($ctx:ident, $sender:expr, $content:expr) => {{
        let _ = $sender.reply(&$ctx, $content).await.map_err(|why| {
            tracing::error!("Failed to reply to message: {:#}", why);
            why
        });
        return $crate::util::Terminator::Terminate;
    }};
    ($ctx:ident, $sender:expr, $($content:tt)+) => {{
        let _ = $sender.reply(&$ctx, format!($($content)+)).await.map_err(|why| {
            tracing::error!("Failed to reply to message: {:#}", why);
            why
        });
        return $crate::util::Terminator::Terminate;
    }};
}

#[macro_export]
/// Same as `ctx` but for `Terminator<Result>`
macro_rules! tctx {
    ($t_result:expr) => {
        util::ctx!($crate::util::t!($t_result))
    };
    ($t_result:expr, $ctx:literal) => {
        util::ctx!($crate::util::t!($t_result), $ctx)
    };
    ($t_result:expr, $($ctx:tt)+) => {
        util::ctx!($crate::util::t!($t_result), $($ctx)+)
    };
}

#[macro_export]
/// Same as `ok` but for `Terminator<Result>`
macro_rules! tok {
    ($t_result:expr, $fail:expr) => {
        util::ok!($crate::util::t!($t_result), $fail)
    };
    ($t_result:expr, $ctx:literal, $fail:expr) => {
        util::ok!($crate::util::t!($t_result), $ctx, $fail)
    };
}
