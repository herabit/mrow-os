#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

#[macro_use]
mod macros;

#[path = "private.rs"]
#[doc(hidden)]
pub mod __private;

#[cfg(feature = "mbr")]
pub mod mbr;
