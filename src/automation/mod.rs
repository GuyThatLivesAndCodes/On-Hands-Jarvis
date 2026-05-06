// Autonomous-control toolset. Each submodule maps to one of the
// "Autonomous System Control" capabilities described in the README and is
// gated by an `Autonomy` safeguard flag in `crate::config`.

pub mod apps;
pub mod input;
pub mod system;
