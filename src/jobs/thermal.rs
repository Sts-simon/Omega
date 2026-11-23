//! Compatibility shim: numerical implementation lives in the Bevy-independent crate.

pub use aerolab_sim_core::thermal::{
    ThermalConfig, ThermalError, ThermalResult, simulate_cancellable,
};
