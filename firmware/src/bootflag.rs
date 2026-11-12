//! State that outlives a reset, in the watchdog's scratch registers.
//!
//! RP235x keeps `WATCHDOG.SCRATCH0..7` across a watchdog or soft reset (only
//! power-on and the RUN pin clear them), so they carry a boot flag and the
//! last supervisor breadcrumb from one boot to the next, bootloader in between.
//! Neither the bootloader nor embassy-boot-rp/embassy-rp touch them. The boot
//! ROM reads 4..7 as a watchdog-boot vector only while SCRATCH4 holds its
//! magic (0xb007c0d3), which nothing here writes. Something outside the
//! firmware can still write them: after a probe-rs reset SCRATCH6 has read
//! 0x2 (every recorded probe boot), which `decode_crumb` rejects.

use embassy_rp::pac::WATCHDOG;

/// SCRATCH5 value that enables `bench` at boot. Anything else (0 after
/// power-on) leaves it disabled, as without `enabled-if`.
#[cfg(feature = "enabled-if")]
const BENCH_ON: u32 = 0xBE9C_0001;

/// `bench`'s `enabled_if:` predicate: is the boot flag set?
#[cfg(feature = "enabled-if")]
pub fn bench_enabled() -> bool {
    let enabled = WATCHDOG.scratch5().read() == BENCH_ON;
    #[cfg(feature = "thermal-demo")]
    {
        safety_core::reset_record::allow_bench_autostart(
            enabled,
            crate::watchdog::previous_reset_cause(),
        )
    }
    #[cfg(not(feature = "thermal-demo"))]
    { enabled }
}

/// Set or clear the boot flag; takes effect at the next boot.
#[cfg(feature = "enabled-if")]
pub fn set_bench(on: bool) {
    WATCHDOG
        .scratch5()
        .write(|w| *w = if on { BENCH_ON } else { 0 });
}

/// Keep the latest `crumb_word()` where the next boot can read it.
#[cfg(feature = "breadcrumb")]
pub fn stash_crumb(w: u32) {
    WATCHDOG.scratch6().write(|v| *v = w);
}

/// The crumb the previous boot stashed (0: none). Clears the stash, so call
/// it once, at boot.
#[cfg(feature = "breadcrumb")]
pub fn take_prev_crumb() -> u32 {
    let w = WATCHDOG.scratch6().read();
    WATCHDOG.scratch6().write(|v| *v = 0);
    w
}


// SAFETY-DEMO: SCRATCH3 is reserved solely for a best-effort reset cause.
// SCRATCH5 remains the BENCH boot flag; SCRATCH6 remains the supervisor crumb.
// No use is made of SCRATCH4, whose boot-ROM vector magic must stay untouched.
/// Record the reason before withholding the existing hardware watchdog feed.
/// The RP2350 retains these scratch registers over a watchdog/soft reset, not
/// over power loss. ROM/probe interactions can overwrite them; treat as hints.
#[cfg(feature = "thermal-demo")]
pub fn stash_safety_reset_reason(reason: safety_core::escalation::EscalationReason) {
    WATCHDOG.scratch3().write(|w| *w = safety_core::reset_record::encode_reset_reason(reason));
}

/// Read once at boot, then clear to avoid displaying stale reset diagnostics.
#[cfg(feature = "thermal-demo")]
pub fn take_safety_reset_reason() -> Option<safety_core::escalation::EscalationReason> {
    let word = WATCHDOG.scratch3().read();
    WATCHDOG.scratch3().write(|w| *w = 0);
    safety_core::reset_record::decode_reset_reason(word)
}
