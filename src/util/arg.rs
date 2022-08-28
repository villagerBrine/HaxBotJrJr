//! Functions for parsing command arguments.
//!
//! These functions only consumes the arguments in order, meaning they will never skip an argument.
//! The `flag` macro consumes all remaining arguments, so it should be used last during argument
//! parsing.
//!
//! The `arg_check` macro is used to send error message and terminate command if the arguments
//! aren't consumed entirely, meaning some arguments are invalid and wasn't being parsed.
//! This is useful as functions `optional`, `any` and `many` simply stops when it encounters
//! an invalid argument, so user have no way of knowing if an argument was actually parsed or not.
//!
//! Note that the `flag` macro invokes `arg_check`, so you don't need to use `arg_check` when `flag`
//! is also used.
use std::fmt::Display;
use std::str::FromStr;

use serenity::client::Context;
use serenity::framework::standard::Args;
use serenity::model::channel::Message;

use util::{ctx, ok};

use crate::util::Terminator;

/// Similar to `Args::single_quoted`, but also sends error message when it failed to parse or there
/// is no argument to parse.
/// `name` describe the argument and is used in the error message.
/// If `use_err` is true, then `FromStr::Err` is included in the error message.
pub async fn single<O>(ctx: &Context, msg: &Message, args: &mut Args, name: &str, use_err: bool) -> Option<O>
where
    O: FromStr,
    <O as FromStr>::Err: Display,
{
    let arg = ok!(args.single_quoted::<String>(), {
        let _ = ctx!(msg.reply(ctx, format!("{} not provided", name)).await);
        return None;
    });
    match O::from_str(&arg) {
        Ok(val) => return Some(val),
        Err(why) => {
            let _ = if use_err {
                ctx!(msg.reply(ctx, format!("'{}' isn't a valid {}: {}", arg, name, why)).await)
            } else {
                ctx!(msg.reply(ctx, format!("'{}' isn't a valid {}", arg, name)).await)
            };
        }
    }
    None
}

/// Similar to `single`, but it only consumes the argument if it can be parsed.
pub fn optional<O>(args: &mut Args) -> Option<O>
where
    O: FromStr,
{
    let arg = ok!(args.single_quoted::<String>(), return None);
    let val = O::from_str(&arg).ok();
    if val.is_none() {
        args.rewind();
    }
    val
}

/// Consume and parse arguments until it encounters an argument that can't be parsed or end of the
/// argument list.
pub fn any<O>(args: &mut Args) -> Vec<O>
where
    O: FromStr,
{
    let mut vals = Vec::new();
    while let Some(val) = optional::<O>(args) {
        vals.push(val)
    }
    vals
}

/// Similar to `any`, but sends an error message if the list is empty and return None
pub async fn many<O>(ctx: &Context, msg: &Message, args: &mut Args, name: &str) -> Option<Vec<O>>
where
    O: FromStr,
{
    let val = any::<O>(args);
    if val.is_empty() {
        let _ = ctx!(msg.reply(ctx, format!("{} not provided", name)).await);
        return None;
    }
    Some(val)
}

/// Consume an argument that is the same as given string and return true, false is returned if the
/// argument didn't match.
pub fn consume_raw(args: &mut Args, s: &str) -> bool {
    if let Ok(arg) = args.single_quoted::<String>() {
        if arg == s {
            return true;
        }
        args.rewind();
    }
    false
}

/// Consume all remaining arguments
pub fn rest(args: &mut Args) -> Vec<String> {
    args.quoted();
    let mut v = Vec::with_capacity(args.remaining());
    while let Ok(arg) = args.single_quoted::<String>() {
        v.push(arg);
    }
    v
}

/// Consumes all remaining arguments, if it is not empty, then send an error message and terminate
/// the command.
/// This is to alert the user of unparsed / invalid arguments
pub async fn arg_check(ctx: &Context, msg: &Message, args: &mut Args) -> Terminator<()> {
    let args = rest(args);
    arg_check_list(ctx, msg, args).await
}

/// Same as `arg_check` buts accepts `Vec<String>` instead of `Args`
async fn arg_check_list(ctx: &Context, msg: &Message, arg_list: Vec<String>) -> Terminator<()> {
    if arg_list.len() == 1 {
        let _ = ctx!(
            msg.reply(ctx, format!("Unrecognized argument `{}`", arg_list.get(0).unwrap()))
                .await
        );
        Terminator::Terminate
    } else if arg_list.len() > 1 {
        let _ = ctx!(
            msg.reply(ctx, format!("Unrecognized arguments `{}`", arg_list.join("`, `")))
                .await
        );
        Terminator::Terminate
    } else {
        Terminator::Proceed(())
    }
}

#[macro_export]
/// Parse arguments.
///
/// The first 3 parameters are of types `Context`, `Message` and `Args`, and the rest are arg
/// expressions separated by commas.
///
/// Arg expressions can be following:
/// - `"name"` Get single argument as String
/// - `?"name"` Get optional argument as Option<String>
/// - `"name": T` Get single argument as T
/// - `?"name": T` Get optional argument as Option<T>
///
/// Examples:
/// ```
/// # use haxbotjr::arg;
/// use anyhow::Result;
/// use serenity::client::Context;
/// use serenity::model::channel::Message;
/// use serenity::framework::standard::{Args, Delimiter};
///
/// use util::ioerr;
///
/// #[derive(Eq, PartialEq)]
/// enum UserAction {
///     Remove,
///     Add,
///     ChangePassword,
/// }
///
/// impl std::str::FromStr for UserAction {
///     type Err = std::io::Error;
///
///     fn from_str(s: &str) -> Result<Self, Self::Err> {
///         Ok(match s {
///             "remove" => Self::Remove,
///             "Add" => Self::Add,
///             "change_password" => Self::ChangePassword,
///             _ => return ioerr!("Failed to parse '{}' as UserAction", s),
///         })
///     }
/// }
///
/// async fn parse_arguments(ctx: &Context, msg: &Message) -> Result<()> {
///     let mut args = Args::new("test123 remove", &[Delimiter::Single(' ')]);
///
///     let username = arg!(ctx, msg, args, "username");
///     assert!(username == "test123");
///
///     let (action, password) = arg!(ctx, msg, args, "action": UserAction, ?"password");
///     assert!(action == UserAction::Remove);
///     assert!(password.is_none());
///
///     Ok(())
/// }
/// ```
/// Required arguments always come before the optional arguments in the expression list.
macro_rules! arg {
    ($ctx:ident, $msg:ident, $args:ident, $name:literal) => {
        util::some!($crate::util::arg::single::<String>(&$ctx, &$msg, &mut $args, $name, false).await, return Ok(()))
    };
    ($ctx:ident, $msg:ident, $args:ident, ?$name:literal) => {
        $crate::util::arg::optional::<String>(&mut $args)
    };
    ($ctx:ident, $msg:ident, $args:ident, $name:literal: $type:ty) => {
        util::some!($crate::util::arg::single::<$type>(&$ctx, &$msg, &mut $args, $name, false).await, return Ok(()))
    };
    ($ctx:ident, $msg:ident, $args:ident, ?$name:literal: $type:ty) => {
        $crate::util::arg::optional::<$type>(&mut $args)
    };
    ($ctx:ident, $msg:ident, $args:ident, $($name:literal),+) => {
        ($($crate::arg!($ctx, $msg, $args, $name),)+)
    };
    ($ctx:ident, $msg:ident, $args:ident, $(?$name:literal),+) => {
        ($($crate::arg!($ctx, $msg, $args, ?$name),)+)
    };
    ($ctx:ident, $msg:ident, $args:ident, $($name:literal: $type:ty),+) => {
        ($($crate::arg!($ctx, $msg, $args, $name: $type),)+)
    };
    ($ctx:ident, $msg:ident, $args:ident, $(?$name:literal: $type:ty),+) => {
        ($($crate::arg!($ctx, $msg, $args, ?$name: $type),)+)
    };
    ($ctx:ident, $msg:ident, $args:ident, $($name:literal: $type:ty),+, $(?$opt_name:literal: $opt_type:ty),+) => {
        ($($crate::arg!($ctx, $msg, $args, $name: $type),)+
         $($crate::arg!($ctx, $msg, $args, ?$opt_name: $opt_type),)+)
    };
    ($ctx:ident, $msg:ident, $args:ident, $($name:literal: $type:ty),+, $(?$opt_name:literal),+) => {
        ($($crate::arg!($ctx, $msg, $args, $name: $type),)+
         $($crate::arg!($ctx, $msg, $args, ?$opt_name),)+)
    };
    ($ctx:ident, $msg:ident, $args:ident, $($name:literal),+, $(?$opt_name:literal: $opt_type:ty),+) => {
        ($($crate::arg!($ctx, $msg, $args, $name),)+
         $($crate::arg!($ctx, $msg, $args, ?$opt_name: $opt_type),)+)
    };
    ($ctx:ident, $msg:ident, $args:ident, $($name:literal),+, $(?$opt_name:literal),+) => {
        ($($crate::arg!($ctx, $msg, $args, $name),)+
         $($crate::arg!($ctx, $msg, $args, ?$opt_name),)+)
    };
}

#[macro_export]
/// Consumes all remaining arguments and checks if they contains flags.
///
/// The first 3 parameters are of types `Context`, `Message` and `Args`, and the rest are flag
/// names separated by commas.
///
/// Note that this macro calls `arg_check`, so you don't need to do it yourself if you are also
/// using `flag`.
macro_rules! flag {
    ($ctx:ident, $msg:ident, $args:ident, $flag:literal) => {{
        let flag = crate::util::arg::consume_raw(&mut $args, $flag);
        $args.quoted();
        if $args.remaining() > 0 {
            crate::t!(crate::util::arg::arg_check($ctx, $msg, &mut $args).await);
        }
        flag
    }};
    ($ctx:ident, $msg:ident, $args:ident, $($flag:literal),+) => {{
        let mut args = crate::util::arg::rest(&mut $args);
        let flags = ($(match args.iter().position(|arg| arg == $flag) {
            Some(i) => {
                args.remove(i);
                true
            }
            None => false
        },)+);
        crate::t!(crate::util::arg::arg_check_list($ctx, $msg, args).await);
        flags
    }}
}
