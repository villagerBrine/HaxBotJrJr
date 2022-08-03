use std::fmt::Display;
use std::str::FromStr;

use serenity::client::Context;
use serenity::framework::standard::{Args, Delimiter};
use serenity::model::channel::Message;

use util::{ctx, some};

pub async fn single<O>(ctx: &Context, msg: &Message, args: &mut Args, name: &str, use_err: bool) -> Option<O>
where
    O: FromStr,
    <O as FromStr>::Err: Display,
{
    let arg = some!(args.quoted().current(), {
        let _ = ctx!(msg.reply(ctx, format!("{} not provided", name)).await);
        return None;
    });
    match O::from_str(arg) {
        Ok(val) => {
            args.advance();
            return Some(val);
        }
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
    let arg = some!(args.quoted().current(), return None);
    if let Ok(val) = O::from_str(arg) {
        args.advance();
        return Some(val);
    }
    None
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

pub fn rest(args: &Args) -> Args {
    Args::new(args.rest(), &[Delimiter::Single(' ')])
}

pub fn flag(args: &Args, name: &str) -> bool {
    args.raw().any(|arg| arg == name)
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
    ($args:ident, $name:literal) => {{
        crate::util::arg::flag(&$args, $name)
    }};
    ($args:ident, $($name:literal),+) => {{
        ($(crate::util::arg::flag(&$args, $name),)+)
    }};
    (%$args:ident, $name:literal) => {{
        let args = crate::util::arg::rest(&mut $args);
        crate::util::arg::flag(&args, $name)
    }};
    (%$args:ident, $($name:literal),+) => {{
        let args = crate::util::arg::rest(&mut $args);
        ($(crate::util::arg::flag(&args, $name),)+)
    }};
}
