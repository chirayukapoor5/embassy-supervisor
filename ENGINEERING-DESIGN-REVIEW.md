# Engineering Design Review — RP2350 Cortex-M33 Safety Supervisor

**Scope:** Opt-in safety extension to [upstream Embassy Supervisor](https://github.com/cedrivard/embassy-supervisor), not a rewrite of the supervisor, bootloader or watchdog. **Status:** software-verified prototype; physical RP2350 validation **not evidenced**. [Case study](NVIDIA-FIRMWARE-CASE-STUDY.md) · [Draft PR / diff](https://github.com/chirayukapoor5/embassy-supervisor/pull/1/files).

## Engineering problem
An optional core-1 workload can continue during bad thermal input, missed deadlines, worker stalls or failed diagnostics. The design blocks application work on uncertain health, demands evidence before recovery, and escalates persistent runtime faults through the *existing* watchdog.

## System architecture
```text
CORE 0: scripted temperature -> thermal policy -----------+
        core-1 progress ---> deadline/diagnostic monitor -+--> atomic workload gate
        monitor freshness -------------------------------+          |
        persistent runtime fault -> escalation -> existing watchdog   v
CORE 1: BENCH quota -> bounded ALU probe -> progress/miss/probe counters
        ^ Acquire loads                     Release/AcqRel writes -> core 0
```
[Policy](safety-core/src/lib.rs) · [Monitor and shared gate](firmware/src/thermal_demo.rs) · [BENCH](firmware/src/bench.rs) · [Watchdog integration](firmware/src/watchdog.rs) · [Reset record](safety-core/src/reset_record.rs).

## Key design decisions / trade-offs
- **Four states:** ACTIVE/IDLE/THROTTLED/SAFE apply **100/10/35/0%** benchmark iteration quotas. Separating idle from heat-driven throttling preserves distinct operational intents; SAFE blocks optional computation. Quotas **do not** set clocks or prove watts saved.
- **Hysteresis:** throttle at synthetic **70 °C**, release at **≤65 °C** to avoid oscillation; recover from SAFE after **three consecutive valid ≤60 °C** samples. Stable but intentionally slower to resume.
- **Recovery based on work, not wakeups:** `completed_windows` can advance even when **zero application iterations** run. Each 100 ms period therefore executes a **96-round ALU diagnostic**, including in SAFE. Only an **on-time, successful** probe increments `verified_windows`; the [deadline monitor](safety-core/src/deadline.rs) requires **three distinct completed + verified advances** to clear a latched fault. Trade-off: overhead and possible conservative delay; this is **not** proof of full application health.
- **Reset only on persistent runtime failure:** transient heat closes the gate; a continuing deadline/stall/probe/monitor fault enters **5 s grace**, then sticky reset-pending and watchdog-feed inhibition (existing **8 s** watchdog). Grace checking is cooperative, not a hard response bound.

## Concurrency contract
Core 0 **Release-stores** `THERMAL_LIMIT`, `DEADLINE_FAULT`, `MONITOR_LAST_TICK_MS`; core 1 **Acquire-loads** these through the workload gate. Core 1 **Release-stores / AcqRel-increments** `BENCH_RUNNING`, last-completion and completed/verified/missed/failed counters; core 0 **Acquire-loads** their snapshot. `EXECUTED_ITERATIONS` telemetry uses **Relaxed**. Freshness is published **last**; stale (>250 ms) or missing monitoring forces quota 0. **Trade-off:** separate atomics avoid locks, but the multi-field snapshot is **not transactional** and can postpone recovery; memory-ordering and latency require silicon stress tests.

## Failure scenarios
| Input / injected event | Expected behavior |
| --- | --- |
| Missing / implausible sensor reading | SAFE; optional-work quota 0 |
| Synthetic critical temperature **≥85 °C** | SAFE; cool-sample recovery required |
| Missed period (**>5 ms** tolerance) / stalled worker (**>250 ms**) | Latch deadline fault; gate 0 |
| Failed bounded diagnostic | `DiagnosticFailure`; no verified-window credit |
| Persistent same-reason runtime fault after grace | `ResetPending`; withhold feed; encode reset reason |

## Verification — exact Linux commands, repository root
```sh
cargo test --locked -p safety-core --target x86_64-unknown-linux-gnu
cargo run --locked -p safety-core --example recovery_campaign --target x86_64-unknown-linux-gnu
mkdir -p reports
cargo run --locked -p safety-core --example fault_matrix --target x86_64-unknown-linux-gnu > reports/raw-fault-campaign.jsonl
python3 tools/verify_fault_report.py
cargo build --locked --release -p firmware --features thermal-demo
```
**Expected:** **42** Rust tests pass; recovery rejects timer-only progress; verifier accepts **27/27** [SR-to-FI scenarios](docs/verification/requirements.json); firmware builds. [Successful historical CI run (12 Nov 2026)](https://github.com/chirayukapoor5/embassy-supervisor/actions/runs/43253866401) additionally recorded **6** verifier tests, two other firmware build variants and the [QEMU AN505 Cortex-M33 policy smoke](safety-cortexm33-smoke/src/main.rs). [Verification procedure and CI artifacts](docs/verification/README.md). Results refer to the linked **historical commit**, not a new run.

## Limitations / acceptance gates
**QEMU runs the shared policy, not RP2350 dual-core firmware.** Sensor values are scripted; the probe shares its CPU with BENCH. No evidence yet for physical resets, scratch retention, WCET, interrupt latency, temperature/power reduction, or hard deadlines. Before deployment: flash RP2350, inject faults, capture watchdog/reboot logs, measure timing under load, and repeat across resets. **No safety certification claim.**
