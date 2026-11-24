//! Abstract flight program: a few parallel threads of simple blocks (wait, ignite, separate,
//! hold a speed or a pressure level, lock the target). The installed hardware decides which
//! blocks exist and the computer decides how many threads and blocks fit.
//!
//! `LockTarget` is only a request for an abstract success roll in the simulation; there is no
//! guidance or tracking logic anywhere in this crate.

use crate::assembly::Vehicle;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "block", rename_all = "snake_case")]
pub enum Block {
    /// Pause this thread for some seconds.
    Wait {
        seconds: f64,
    },
    IgniteStage {
        stage: usize,
    },
    SeparateStage {
        stage: usize,
    },
    /// Throttle for liquid and air-breathing engines (solid motors cannot throttle).
    SetThrottle {
        fraction: f64,
    },
    /// Needs a pitot tube.
    HoldSpeed {
        speed_ms: f64,
    },
    /// Needs a barometer: hold the altitude where the air pressure equals `hpa`.
    HoldPressure {
        hpa: f64,
    },
    /// Needs a radar. Must be the last block of its thread (final phase).
    LockTarget,
}

impl Block {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Wait { .. } => "wait",
            Self::IgniteStage { .. } => "ignite_stage",
            Self::SeparateStage { .. } => "separate_stage",
            Self::SetThrottle { .. } => "set_throttle",
            Self::HoldSpeed { .. } => "hold_speed",
            Self::HoldPressure { .. } => "hold_pressure",
            Self::LockTarget => "lock_target",
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Wait { seconds } => format!("Wait {seconds:.1} s"),
            Self::IgniteStage { stage } => format!("Ignite stage {}", stage + 1),
            Self::SeparateStage { stage } => format!("Separate stage {}", stage + 1),
            Self::SetThrottle { fraction } => format!("Throttle {:.0}%", fraction * 100.0),
            Self::HoldSpeed { speed_ms } => format!("Hold speed {speed_ms:.0} m/s"),
            Self::HoldPressure { hpa } => format!("Hold {hpa:.0} hPa"),
            Self::LockTarget => "Lock target (final phase)".to_string(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub threads: Vec<Vec<Block>>,
}

impl Program {
    pub fn block_count(&self) -> usize {
        self.threads.iter().map(Vec::len).sum()
    }

    /// One thread from a block list, handy for tests and presets.
    pub fn single(blocks: Vec<Block>) -> Self {
        Self { threads: vec![blocks] }
    }
}

/// Everything wrong with the program for this vehicle (empty list = valid).
pub fn validate_program(program: &Program, vehicle: &Vehicle) -> Vec<String> {
    let mut issues = Vec::new();
    let Some(computer) = vehicle.computer() else {
        return vec!["the vehicle has no flight computer".to_string()];
    };
    if program.threads.len() > computer.threads() {
        issues.push(format!(
            "{} parallel threads but the computer runs only {}",
            program.threads.len(),
            computer.threads()
        ));
    }
    if program.block_count() > computer.ram_blocks() {
        issues.push(format!(
            "{} blocks but the computer memory holds only {}",
            program.block_count(),
            computer.ram_blocks()
        ));
    }
    let available = vehicle.available_blocks();
    let mut locks = 0;
    for (t, thread) in program.threads.iter().enumerate() {
        for (b, block) in thread.iter().enumerate() {
            let place = format!("thread {} block {}", t + 1, b + 1);
            if !available.contains(block.kind()) {
                issues.push(format!(
                    "{place}: '{}' needs hardware this vehicle does not have",
                    block.label()
                ));
            }
            match *block {
                Block::Wait { seconds } if !(0.0..=3_600.0).contains(&seconds) || !seconds.is_finite() => {
                    issues.push(format!("{place}: wait must be between 0 and 3600 s"));
                }
                Block::IgniteStage { stage } | Block::SeparateStage { stage } if stage >= vehicle.stage_count() => {
                    issues.push(format!("{place}: stage {} does not exist", stage + 1));
                }
                Block::SetThrottle { fraction } if !(0.0..=1.0).contains(&fraction) || !fraction.is_finite() => {
                    issues.push(format!("{place}: throttle must be between 0 and 1"));
                }
                Block::HoldSpeed { speed_ms } if !(10.0..=1_500.0).contains(&speed_ms) || !speed_ms.is_finite() => {
                    issues.push(format!("{place}: speed must be between 10 and 1500 m/s"));
                }
                Block::HoldPressure { hpa } if !(50.0..=1_013.25).contains(&hpa) || !hpa.is_finite() => {
                    issues.push(format!("{place}: pressure must be between 50 and 1013 hPa"));
                }
                Block::LockTarget => {
                    locks += 1;
                    if b + 1 != thread.len() {
                        issues.push(format!(
                            "{place}: 'Lock target' must be the last block of its thread (final phase)"
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    if locks > 1 {
        issues.push("only one 'Lock target' block is allowed".to_string());
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assembly::tests::sounding_rocket;
    use crate::vehicle::{ComputerClass, PartSpec, RadarClass, SensorKind};

    fn with_radar() -> Vehicle {
        let mut v = sounding_rocket();
        v.add_to_top(
            "radar",
            PartSpec::Sensor {
                kind: SensorKind::Radar(RadarClass::Basic),
            },
        )
        .unwrap();
        v
    }

    #[test]
    fn valid_sequence_passes() {
        let program = Program::single(vec![
            Block::IgniteStage { stage: 0 },
            Block::Wait { seconds: 20.0 },
            Block::SeparateStage { stage: 0 },
            Block::IgniteStage { stage: 1 },
            Block::HoldSpeed { speed_ms: 300.0 },
        ]);
        assert!(validate_program(&program, &sounding_rocket()).is_empty());
    }

    #[test]
    fn a_basic_computer_limits_threads_and_memory() {
        let vehicle = sounding_rocket(); // basic computer: 1 thread, 8 blocks
        let program = Program {
            threads: vec![vec![Block::Wait { seconds: 1.0 }], vec![Block::Wait { seconds: 1.0 }]],
        };
        assert!(
            validate_program(&program, &vehicle)
                .join(" ")
                .contains("parallel threads")
        );
        let long = Program::single(vec![Block::Wait { seconds: 1.0 }; 9]);
        assert!(validate_program(&long, &vehicle).join(" ").contains("memory"));

        let mut better = sounding_rocket();
        better.replace_part(
            "pc",
            PartSpec::Computer {
                class: ComputerClass::Standard,
            },
        );
        assert!(validate_program(&program, &better).is_empty());
        assert!(
            validate_program(&long, &better).is_empty(),
            "16 blocks fit a standard computer"
        );
    }

    #[test]
    fn blocks_require_the_matching_sensor() {
        let vehicle = sounding_rocket(); // pitot only
        let program = Program::single(vec![Block::HoldPressure { hpa: 900.0 }, Block::LockTarget]);
        let issues = validate_program(&program, &vehicle).join(" | ");
        assert!(
            issues.contains("Hold 900 hPa") && issues.contains("Lock target"),
            "{issues}"
        );
        let ok = Program::single(vec![Block::HoldSpeed { speed_ms: 250.0 }, Block::LockTarget]);
        assert!(validate_program(&ok, &with_radar()).is_empty());
    }

    #[test]
    fn lock_target_must_close_its_thread_and_be_unique() {
        let vehicle = with_radar();
        let early = Program::single(vec![Block::LockTarget, Block::Wait { seconds: 1.0 }]);
        assert!(validate_program(&early, &vehicle).join(" ").contains("last block"));
        let twice = Program {
            threads: vec![vec![Block::LockTarget]],
        };
        assert!(validate_program(&twice, &vehicle).is_empty());
    }

    #[test]
    fn invalid_parameters_and_missing_stages_are_reported() {
        let vehicle = sounding_rocket();
        let program = Program::single(vec![
            Block::IgniteStage { stage: 5 },
            Block::SetThrottle { fraction: 1.5 },
            Block::Wait { seconds: -1.0 },
            Block::HoldSpeed { speed_ms: 5.0 },
        ]);
        let issues = validate_program(&program, &vehicle);
        assert_eq!(issues.len(), 4, "{issues:?}");
        let mut single = sounding_rocket();
        single.remove_subtree("sustainer");
        assert!(!validate_program(&Program::single(vec![Block::SeparateStage { stage: 0 }]), &single).is_empty());
    }

    #[test]
    fn no_computer_means_no_program() {
        let mut vehicle = sounding_rocket();
        vehicle.remove_subtree("pc");
        assert!(validate_program(&Program::default(), &vehicle)[0].contains("no flight computer"));
    }

    #[test]
    fn programs_round_trip_through_json() {
        let program = Program::single(vec![
            Block::HoldPressure { hpa: 1013.0 },
            Block::Wait { seconds: 2.5 },
            Block::LockTarget,
        ]);
        let json = serde_json::to_string(&program).unwrap();
        assert_eq!(serde_json::from_str::<Program>(&json).unwrap(), program);
    }
}
