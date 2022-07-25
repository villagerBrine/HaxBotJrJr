//! Macros for trait implementations

#[macro_export]
/// Implement type as parsing error
macro_rules! parse_error {
    ($err:ident, $name:literal) => {
        #[derive(Debug)]
        pub struct $err(pub String);
        impl std::fmt::Display for $err {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "Failed to parse '{}' as {}", self.0, $name)
            }
        }
        impl std::error::Error for $err {}
    };
}

#[macro_export]
/// Implement Error on type with a literal description
macro_rules! simple_error {
    ($err:ident, $desc:literal) => {
        #[derive(Debug)]
        pub struct $err;
        impl std::fmt::Display for $err {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, $desc)
            }
        }
        impl std::error::Error for $err {}
    };
}

#[macro_export]
/// Implement Error on type with formatted description
macro_rules! make_error {
    ($err:ident, $fmt:literal, $field1:ty) => {
        #[derive(Debug)]
        pub struct $err(pub $field1);
        impl std::fmt::Display for $err {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, $fmt, self.0)
            }
        }
        impl std::error::Error for $err {}
    };
    ($err:ident, $fmt:literal, $field1:ty, $field2:ty) => {
        #[derive(Debug)]
        pub struct $err(pub $field1, pub $field2);
        impl std::fmt::Display for $err {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, $fmt, self.0, self.1)
            }
        }
        impl std::error::Error for $err {}
    };
}

#[macro_export]
/// Implement Display on type with the same output as Debug
macro_rules! impl_debug_display {
    ($arg:ident) => {
        impl std::fmt::Display for $arg {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{:?}", self)
            }
        }
    };
}

#[macro_export]
/// Implement sqlx type
macro_rules! impl_sqlx_type {
    ($this:ident) => {
        impl sqlx::types::Type<sqlx::sqlite::Sqlite> for $this {
            fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
                <&str as sqlx::types::Type<sqlx::sqlite::Sqlite>>::type_info()
            }
        }

        impl<'q> sqlx::encode::Encode<'q, sqlx::sqlite::Sqlite> for $this {
            fn encode_by_ref(
                &self, args: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>,
            ) -> sqlx::encode::IsNull {
                self.to_string().encode(args)
            }
        }

        impl<'r> sqlx::decode::Decode<'r, sqlx::sqlite::Sqlite> for $this {
            fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
                Ok(Self::from_str(&String::decode(value)?)?)
            }
        }
    };
}
