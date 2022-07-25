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
