#![no_std]

#[macro_use]
mod macros;

#[path = "private.rs"]
#[doc(hidden)]
pub mod __private;
