//! Best-effort reset-cause format for the existing RP2350 SCRATCH3 register.
//!
//! The pure encoding/decoding is host + QEMU testable. Scratch-register
//! retention itself is NOT proven without physical silicon. A 16-bit version
//! marker and an 8-bit complement catch many accidental corruptions; they do
//! not provide cryptographic authenticity or guaranteed integrity.
use crate::escalation::EscalationReason;

const PREFIX: u32 = 0xD15A_0000;

pub const fn encode_reset_reason(reason: EscalationReason) -> u32 {
    let code = reason.code();
    PREFIX | ((code as u32) << 8) | ((!code) as u32)
}

/// Rejects erased, unknown, malformed, stale-version or complement-mismatched
/// contents. The caller should read-once and clear SCRATCH3 after boot.
pub const fn decode_reset_reason(word: u32) -> Option<EscalationReason> {
    if word & 0xFFFF_0000 != PREFIX {
        return None;
    }
    let code = ((word >> 8) & 0xFF) as u8;
    let complement = (word & 0xFF) as u8;
    if complement != !code {
        return None;
    }
    EscalationReason::from_code(code)
}

/// Decouple first-boot behavior from the RP2350 register API.
/// An earlier escalation blocks benchmark auto-start even if a boot flag is
/// unexpectedly left enabled. Manual control can still start the demo later.
pub const fn allow_bench_autostart(
    boot_flag_enabled: bool,
    previous_reset: Option<EscalationReason>,
) -> bool {
    boot_flag_enabled && previous_reset.is_none()
}
