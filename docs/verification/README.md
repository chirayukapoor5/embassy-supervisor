# Requirements traceability and hardware-free runtime validation

This directory contains engineering-style verification artifacts for the original
safety-core extension of embassy-supervisor. It is **not** an ISO 26262 or
Automotive SPICE certification claim.

## Source-controlled traceability

The [requirements.json](requirements.json) manifest contains 27 individual
requirements (SR-001 through SR-027). Each one maps to an implementation
source file and one fault-injection scenario (FI-001 through FI-027).

All scenarios execute the actual `safety-core` Rust state machines using
**synthetic inputs and controlled timestamps**. They do not emulate the RP2350
watchdog or report measured microcontroller temperature.

Reproduce the structured report in Windows cmd from repository root:

```bat
mkdir reports
cargo run --locked -p safety-core --example fault_matrix --target x86_64-pc-windows-msvc > reports\raw-fault-campaign.jsonl
python tools\verify_fault_report.py
```

The verifier requires an exact manifest-to-scenario mapping, unique IDs,
matching expected and observed outputs, an accurate summary and an explicit
no-hardware-executed flag. It fails CI if any evidence is missing or inconsistent.

Artifacts produced by the verification workflow:
- `reports/raw-fault-campaign.jsonl` — the complete per-scenario runtime log
- `reports/fault-injection-results.json` — structured verified evidence
- `reports/fault-injection-report.md` — reviewer-readable traceability table

Download the **safety-fault-campaign** artifact from the GitHub Actions run.

## Hardware-free Arm runtime validation

QEMU models the **Arm MPS2-AN505 Cortex-M33** board. The
`safety-cortexm33-smoke` crate is a genuinely `no_std` executable that
links the same safety-core policy as the RP2350 application. It validates
representative thermal, deadline and escalation state changes under Arm CPU
emulation, without pretending the original RP2350 firmware can boot on AN505.

It runs on the separate `arm-emulator` GitHub Actions job and returns a
process success or failure through Arm semihosting. A timeout also fails CI.

**It does not validate** the RP2350's peripheral registers, multicore launch,
USB networking, physical watchdog timeout, scratch-register retention,
interrupt latency, thermal readings, power consumption, or actual execution
deadlines.

## Diagnostic-backed recovery and reset evidence

The worker now executes a bounded 96-round arithmetic diagnostic every
100 ms period, even when SAFE permits **zero application iterations**.
It publishes a separate `verified_windows` counter only for on-time periods
whose diagnostic returned the expected known-good value.

The monitor requires three distinct increases of verified work to clear
its fault latch. A failed probe raises `DiagnosticFailure`, which can
escalate to a watchdog reset if it persists. A timer wake-up without
verified diagnostic progress does not count as recovery.

The reset-cause record stored in SCRATCH3 is versioned and contains an
inverted reason byte to reject many accidental corruptions. A previous
escalated reset blocks automatic BENCH startup, even if the boot flag is set.

### Evidence boundaries

| Scope | Verified by | Not established |
|---|---|---|
| Thermal and deadline policy | Rust host tests and fault cases | Real sensor or hard deadline timing |
| Verified recovery | Bounded ALU diagnostic, counter and fault campaign | Independent hardware supervision |
| Reset persistence format | Encoding, corruption and startup-inhibit tests | Physical retention across RP2350 reset |
| Arm CPU portability | QEMU AN505 policy-runtime smoke | RP2350 multicore, USB or watchdog execution |
| Firmware integration | Three Cortex-M33 release builds | Silicon behavior, timing or energy use |

The diagnostic and benchmark run on the same worker CPU, so common-mode
processor faults can defeat both. Reset records are best-effort and can
be lost on a power cycle. Real hardware acceptance needs an RP2350 board.

Run the dedicated recovery example using Windows PowerShell:

```powershell
cargo run --locked -p safety-core --example recovery_campaign --target x86_64-pc-windows-msvc
```

See [architecture](../architecture/README.md) for full integration details.
