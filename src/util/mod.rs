pub mod average;
#[macro_use]
pub mod logging;
pub mod meters;
pub mod never;
pub mod rand;

pub trait WithLifetime {
    type Type<'a>;
}
