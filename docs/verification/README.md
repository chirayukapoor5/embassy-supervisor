# Requirements traceability and hardware-free runtime validation

This directory contains engineering-style verification artifacts for the original
safety-core extension of embassy-supervisor. It is **not** an ISO 26262 or
Automotive SPICE certification claim.

## Source-controlled traceability

The [requirements.json](requirements.json) manifest contains 18 individual
requirements (SR-001 through SR-018). Each one maps to an implementation
source file and one fault-injection scenario (FI-001 through FI-018).

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

## Important known limitation

The deadline monitor currently counts timely benchmark periods to clear a
fault, even if the safety gate permits zero application iterations during
those periods. That is software evidence of task responsiveness but **not**
proof that full computation recovered. Introduce a separate bounded
diagnostic-work probe and independent heartbeat witness before asserting
complete functional recovery. Full RP2350 runtime validation still needs
compatible hardware or a sufficiently faithful multicore/peripheral emulator.
