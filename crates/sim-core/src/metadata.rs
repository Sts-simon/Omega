use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Provenance attached to every persisted scientific result.
///
/// `input_fingerprint` is a deterministic FNV-1a bookkeeping hash. It is not a
/// cryptographic signature and must not be used to authenticate data.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationMetadata {
    pub metadata_schema_version: u32,
    pub application_version: String,
    pub model_id: String,
    pub model_version: String,
    pub solver_id: String,
    pub solver_version: String,
    pub unit_system: String,
    pub input_fingerprint: String,
    pub created_unix_ms: u128,
}

impl SimulationMetadata {
    pub fn new<T: Serialize>(
        model_id: impl Into<String>,
        model_version: impl Into<String>,
        solver_id: impl Into<String>,
        solver_version: impl Into<String>,
        unit_system: impl Into<String>,
        input: &T,
    ) -> Result<Self, String> {
        let serialized_input = serde_json::to_vec(input).map_err(|error| error.to_string())?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?;

        Ok(Self {
            metadata_schema_version: 1,
            application_version: env!("CARGO_PKG_VERSION").to_string(),
            model_id: model_id.into(),
            model_version: model_version.into(),
            solver_id: solver_id.into(),
            solver_version: solver_version.into(),
            unit_system: unit_system.into(),
            input_fingerprint: fnv1a64(&serialized_input),
            created_unix_ms: now.as_millis(),
        })
    }
}

fn fnv1a64(bytes: &[u8]) -> String {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let hash = bytes
        .iter()
        .fold(OFFSET_BASIS, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(PRIME));
    format!("fnv1a64:{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct Inputs {
        width: usize,
        factor: f64,
    }

    fn metadata(input: &Inputs) -> SimulationMetadata {
        SimulationMetadata::new(
            "material.heat_conduction.2d",
            "2.0.0",
            "explicit_finite_volume_forward_euler_precomputed",
            "1.1.0",
            "SI",
            input,
        )
        .unwrap()
    }

    #[test]
    fn identical_inputs_have_identical_fingerprints() {
        let first = metadata(&Inputs { width: 64, factor: 0.2 });
        let second = metadata(&Inputs { width: 64, factor: 0.2 });
        assert_eq!(first.input_fingerprint, second.input_fingerprint);
    }

    #[test]
    fn changed_inputs_change_the_fingerprint() {
        let first = metadata(&Inputs { width: 64, factor: 0.2 });
        let second = metadata(&Inputs {
            width: 64,
            factor: 0.21,
        });
        assert_ne!(first.input_fingerprint, second.input_fingerprint);
    }
}
