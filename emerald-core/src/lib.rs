//! What every Emerald crate shares: where files go ([`paths`]), interface
//! text ([`i18n`], with the `t!` macro), animated image frames ([`frames`])
//! and runs of this same executable ([`process`]).

#[macro_use]
pub mod i18n;

pub mod frames;
pub mod paths;
pub mod process;
