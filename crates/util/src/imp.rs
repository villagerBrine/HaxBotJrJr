//! Macros for trait implementations

/// Create a simple error type that is an unit struct.
///
/// Takes a name for that struct and a string for its [`Display`].
/// ```
/// # use util::simple_error;
/// simple_error!(SimpleError, "This is an error");
///
/// let msg = format!("{}", SimpleError);
/// assert!(msg == "This is an error");
/// ```
/// Use [`make_error`] for error type with fields.
///
/// [`make_error`]: crate::make_error
/// [`Display`]: std::fmt::Display
#[macro_export]
macro_rules! simple_error {
    ($err:ident, $desc:literal) => {
        #[derive(Debug)]
        pub struct $err;
        impl ::std::fmt::Display for $err {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                ::std::write!(f, $desc)
            }
        }
        impl ::std::error::Error for $err {}
    };
}

/// Create an error type struct that takes in arguments
///
/// Takes a name for that struct, a string that is used to format the output of [`Display`], and then
/// a list of values this struct takes in terms of their type.
/// ```
/// # use util::make_error;
/// make_error!(Error, "{1} != {0}", u64, u64);
///
/// let err = Error(1, 2);
/// let msg = format!("{}", err);
/// assert!(msg == "2 != 1");
/// ```
/// Use [`simple_error`] for error type with no fields.
///
/// [`simple_error`]: crate::simple_error
/// [`Display`]: std::fmt::Display
#[macro_export]
macro_rules! make_error {
    ($err:ident, $fmt:literal, $field1:ty) => {
        #[derive(Debug)]
        pub struct $err(pub $field1);
        impl ::std::fmt::Display for $err {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                ::std::write!(f, $fmt, self.0)
            }
        }
        impl ::std::error::Error for $err {}
    };
    ($err:ident, $fmt:literal, $field1:ty, $field2:ty) => {
        #[derive(Debug)]
        pub struct $err(pub $field1, pub $field2);
        impl ::std::fmt::Display for $err {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                ::std::write!(f, $fmt, self.0, self.1)
            }
        }
        impl ::std::error::Error for $err {}
    };
}

/// Implement [`Display`] on type with the same output as [`Debug`]
/// ```
/// # use util::impl_debug_display;
/// #[derive(Debug)]
/// struct Test;
/// impl_debug_display!(Test);
///
/// assert!(format!("{}", Test) == format!("{:?}", Test));
/// ```
///
/// [`Display`]: std::fmt::Display
/// [`Debug`]: std::fmt::Debug
#[macro_export]
macro_rules! impl_debug_display {
    ($arg:ident) => {
        impl ::std::fmt::Display for $arg {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                ::std::write!(f, "{:?}", self)
            }
        }
    };
}

/// Implement `sqlx::type::Type` that encodes via [`Display`] and decodes via [`FromStr`].
///
/// [`Display`]: std::fmt::Display
/// [`FromStr`]: std::str::FromStr
#[macro_export]
macro_rules! impl_sqlx_type {
    ($this:ident) => {
        impl ::sqlx::types::Type<::sqlx::sqlite::Sqlite> for $this {
            fn type_info() -> ::sqlx::sqlite::SqliteTypeInfo {
                <&str as ::sqlx::types::Type<::sqlx::sqlite::Sqlite>>::type_info()
            }
        }

        impl<'q> ::sqlx::encode::Encode<'q, ::sqlx::sqlite::Sqlite> for $this {
            fn encode_by_ref(
                &self, args: &mut ::std::vec::Vec<::sqlx::sqlite::SqliteArgumentValue<'q>>,
            ) -> ::sqlx::encode::IsNull {
                self.to_string().encode(args)
            }
        }

        impl<'r> ::sqlx::decode::Decode<'r, ::sqlx::sqlite::Sqlite> for $this {
            fn decode(
                value: ::sqlx::sqlite::SqliteValueRef<'r>,
            ) -> ::std::result::Result<Self, ::sqlx::error::BoxDynError> {
                Ok(Self::from_str(&::std::string::String::decode(value)?)?)
            }
        }
    };
}
