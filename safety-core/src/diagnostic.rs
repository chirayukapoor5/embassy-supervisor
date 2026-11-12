//! Bounded, deterministic ALU self-check executed by the workload worker.
//!
//! This probe is separate from application iterations. It runs in SAFE so the
//! monitor can demand nontrivial computation before reopening the workload
//! gate. This is NOT independent hardware, memory ECC, or a safety-certified
//! processor self-test. Execution time must be measured on silicon.
const SEED: u32 = 0x1A2B_3C4D;
const GOLDEN: u32 = 0xF930_3A58;
const ROUNDS: usize = 96;

/// A short integer-work test with an independently frozen golden vector.
/// The raw seed may be changed by a test to inject a synthetic mismatch.
pub fn diagnostic_probe_check(mut seed: u32) -> bool {
    for _ in 0..ROUNDS {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
    }
    seed == GOLDEN
}

/// Execute bounded work at runtime, including in the worker's SAFE state.
/// black_box prevents the host compiler from precomputing the known vector.
#[inline(never)]
pub fn run_diagnostic_probe() -> bool {
    diagnostic_probe_check(core::hint::black_box(SEED))
}
