//! Core types of the Ockam library.
//!
//! This crate contains the core types of the Ockam library and is intended
//! for use by other crates that provide features and add-ons to the main
//! Ockam library.
//!
//! The main Ockam crate re-exports types defined in this crate.
#![deny(unsafe_code)]
#![warn(
    missing_docs,
    trivial_casts,
    trivial_numeric_casts,
    unused_import_braces,
    unused_qualifications
)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(all(not(feature = "std"), not(feature = "alloc")))]
compile_error!(r#"The "no_std" feature currently requires the "alloc" feature"#);

#[cfg(feature = "std")]
extern crate core;

#[cfg(feature = "alloc")]
#[macro_use]
extern crate alloc;

pub use async_trait::async_trait;

pub extern crate hashbrown;

#[allow(unused_imports)]
#[macro_use]
pub extern crate hex;

#[allow(unused_imports)]
#[macro_use]
pub extern crate async_trait;
pub use async_trait::async_trait as worker;

extern crate ockam_macro;
pub use ockam_macro::AsyncTryClone;

extern crate futures_util;

pub mod compat;
mod error;
mod message;
mod processor;
mod routing;
mod uint;
mod worker;

pub use error::*;
pub use message::*;
pub use processor::*;
pub use routing::*;
pub use traits::*;
pub use uint::*;
pub use worker::*;

#[cfg(feature = "std")]
pub use std::println;

#[cfg(all(not(feature = "std"), feature = "alloc"))]
/// println macro for no_std
pub mod println_no_std {

    #[cfg(all(target_arch = "arm", feature = "itm"))]
    #[macro_export]
    /// implementation for cortex-m itm
    macro_rules! println {
        ($($arg:tt)*) => {{
            {
                // give the itm buffer time to empty
                cortex_m::asm::delay(96_000_000 / 32);

                let itm = unsafe { &mut *cortex_m::peripheral::ITM::ptr() };
                cortex_m::iprintln!(&mut itm.stim[0], $($arg)*);
            }
        }};
    }

    #[cfg(all(target_arch = "arm", feature = "semihosting"))]
    #[macro_export]
    /// implementation for cortex-m semihosting
    macro_rules! println {
        ($($arg:tt)*) => {{
            cortex_m_semihosting::hprintln!($($arg)*).unwrap();
        }};
    }

    #[cfg(not(target_arch = "arm"))]
    #[macro_export]
    /// place-holder implementation for unsupported configurations
    macro_rules! println {
        ($($arg:tt)*) => {{
            {
                use ockam_core::compat::io::Write;
                let mut buffer = [0 as u8; 1];
                let mut cursor = ockam_core::compat::io::Cursor::new(&mut buffer[..]);
                match write!(&mut cursor, $($arg)*) {
                    Ok(()) => (),
                    Err(_) => (),
                }
            }
        }};
    }
}


/// Module for custom implementation of standard traits.
pub mod traits {
    use crate::compat::boxed::Box;
    use crate::error::Result;

    /// Clone trait for async structs.
    #[async_trait]
    pub trait AsyncTryClone: Sized {
        /// Try cloning a object and return an `Err` in case of failure.
        async fn async_try_clone(&self) -> Result<Self>;
    }
    #[async_trait]
    impl<D> AsyncTryClone for D
    where
        D: Clone + Sync,
    {
        async fn async_try_clone(&self) -> Result<Self> {
            Ok(self.clone())
        }
    }
}
