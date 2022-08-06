use std::fmt::Display;
use std::str::FromStr;

use serenity::client::Context;
use serenity::framework::standard::Args;
use serenity::model::channel::Message;

use util::{ctx, ok};

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

pub fn consume_raw(args: &mut Args, s: &str) -> bool {
    if let Ok(arg) = args.single_quoted::<String>() {
        if arg == s {
            return true;
        }
        args.rewind();
    }
    false
}

pub fn rest(args: &mut Args) -> Vec<String> {
    args.quoted();
    let mut v = Vec::with_capacity(args.remaining());
    while let Ok(arg) = args.single_quoted::<String>() {
        v.push(arg);
    }
    v
}

#[macro_export]
macro_rules! arg {
    ($ctx:ident, $msg:ident, $args:ident, $name:literal) => {
        util::some!(crate::util::arg::single::<String>(&$ctx, &$msg, &mut $args, $name, false).await, return Ok(()))
    };
    ($ctx:ident, $msg:ident, $args:ident, ?$name:literal) => {
        crate::util::arg::optional::<String>(&mut $args)
    };
    ($ctx:ident, $msg:ident, $args:ident, $name:literal: $type:ty) => {
        util::some!(crate::util::arg::single::<$type>(&$ctx, &$msg, &mut $args, $name, false).await, return Ok(()))
    };
    ($ctx:ident, $msg:ident, $args:ident, %$name:literal: $type:ty) => {
        util::some!(crate::util::arg::single::<$type>(&$ctx, &$msg, &mut $args, $name, true).await,
            return Ok(()))
    };
    ($ctx:ident, $msg:ident, $args:ident, ?$name:literal: $type:ty) => {
        crate::util::arg::optional::<$type>(&mut $args)
    };
    ($ctx:ident, $msg:ident, $args:ident, $($name:literal),+) => {
        ($(crate::arg!($ctx, $msg, $args, $name),)+)
    };
    ($ctx:ident, $msg:ident, $args:ident, $(?$name:literal),+) => {
        ($(crate::arg!($ctx, $msg, $args, ?$name),)+)
    };
    ($ctx:ident, $msg:ident, $args:ident, $($name:literal: $type:ty),+) => {
        ($(crate::arg!($ctx, $msg, $args, $name: $type),)+)
    };
    ($ctx:ident, $msg:ident, $args:ident, $(?$name:literal: $type:ty),+) => {
        ($(crate::arg!($ctx, $msg, $args, ?$name: $type),)+)
    };
    ($ctx:ident, $msg:ident, $args:ident, $($name:literal: $type:ty),+, $(?$opt_name:literal: $opt_type:ty),+) => {
        ($(crate::arg!($ctx, $msg, $args, $name: $type),)+ $(crate::arg!($ctx, $msg, $args, ?$opt_name: $opt_type),)+)
    };
}

#[macro_export]
macro_rules! flag {
    ($ctx:ident, $msg:ident, $args:ident, $flag:literal) => {{
        let flag = crate::util::arg::consume_raw(&mut $args, $flag);
        $args.quoted();
        if $args.remaining() > 0 {
            crate::arg_check!($ctx, $msg, $args);
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
        crate::arg_check!(RAW $ctx, $msg, args);
        flags
    }}
}

#[macro_export]
macro_rules! arg_check {
    ($ctx:ident, $msg:ident, $args:ident) => {{
        let args = crate::util::arg::rest(&mut $args);
        crate::arg_check!(RAW $ctx, $msg, args);
    }};
    (RAW $ctx:ident, $msg:ident, $args:ident) => {{
        if $args.len() == 1 {
            crate::finish!($ctx, $msg, "Unrecognized argument `{}`", $args.get(0).unwrap())
        } else if $args.len() > 1 {
            crate::finish!($ctx, $msg, "Unrecognized arguments `{}`", $args.join("`, `"))
        }
    }};
}
