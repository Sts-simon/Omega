use omega_rules::flight::{FlightConfig, simulate};
use omega_rules::program::{Block, Program};
use omega_rules::vehicle::{ComputerClass, Part, PartSpec, Stage, Vehicle};
use std::hint::black_box;
use std::time::Instant;

fn benchmark_vehicle() -> Vehicle {
    Vehicle {
        name: "Benchmark vehicle".to_string(),
        stages: vec![Stage {
            parts: vec![
                Part {
                    id: "motor".to_string(),
                    spec: PartSpec::SolidMotor {
                        length_m: 2.0,
                        diameter_m: 0.4,
                        burn_time_s: 20.0,
                    },
                },
                Part {
                    id: "computer".to_string(),
                    spec: PartSpec::Computer {
                        class: ComputerClass::Basic,
                    },
                },
            ],
        }],
    }
}

fn main() {
    const WARMUP_RUNS: u64 = 50;
    const MEASURED_RUNS: u64 = 1_000;

    let vehicle = benchmark_vehicle();
    let program = Program::single(vec![Block::IgniteStage { stage: 0 }]);

    for seed in 0..WARMUP_RUNS {
        let config = FlightConfig {
            seed,
            ..FlightConfig::default()
        };
        black_box(simulate(&vehicle, &program, config).expect("valid benchmark input"));
    }

    let start = Instant::now();
    let mut total_samples = 0usize;
    for seed in WARMUP_RUNS..WARMUP_RUNS + MEASURED_RUNS {
        let config = FlightConfig { seed, ..FlightConfig::default() };
        let report = black_box(simulate(&vehicle, &program, config).expect("valid benchmark input"));
        total_samples += report.samples.len();
    }
    let elapsed = start.elapsed();
    let nanos_per_run = elapsed.as_nanos() / u128::from(MEASURED_RUNS);

    println!("Omega rules trial microbenchmark");
    println!("warm-up runs: {WARMUP_RUNS}");
    println!("measured runs: {MEASURED_RUNS}");
    println!("elapsed: {:.3} s", elapsed.as_secs_f64());
    println!("average: {nanos_per_run} ns/run");
    println!(
        "average samples/run: {:.1}",
        total_samples as f64 / MEASURED_RUNS as f64
    );
}
