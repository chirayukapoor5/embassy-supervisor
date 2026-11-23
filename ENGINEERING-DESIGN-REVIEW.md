# Engineering Design Review — RP2350 Cortex-M33 Safety Supervisor

**Scope:** Original, opt-in `thermal-demo` extension to [upstream Embassy Supervisor](https://github.com/cedrivard/embassy-supervisor), not a rewrite of its task framework or watchdog driver. **Evidence level:** host-tested, QEMU policy-executed, firmware cross-compiled; **physical RP2350 behavior unverified**. [Case study](NVIDIA-FIRMWARE-CASE-STUDY.md) · [Draft PR / diff](https://github.com/chirayukapoor5/embassy-supervisor/pull/1/files).

## Engineering problem
An optional core-1 workload may continue despite invalid thermal samples, missed deadlines, lost progress, or failed self-checks. The design **fails closed on application work**, preserves enough diagnostic execution to support recovery, and escalates persistent runtime faults through the *existing* watchdog. These are software policies, not measured silicon protections.

## System architecture
```text
Core 0: scripted temperature -> SafetyController ----+
         BENCH counters ---> DeadlineMonitor --------+--> atomic work gate (0/10/35/100%)
         monitor freshness --------------------------+         |  Acquire read
         persistent deadline/probe/stall fault -> EscalationPolicy
              -> existing watchdog feed / SCRATCH3 reset reason       v
Core 1: BENCH work quota -> 96-round diagnostic -> completion/verified/miss/failure counters
                                             (Release/AcqRel writes -> core-0 Acquire reads)
```
**Code:** [policy](safety-core/src/lib.rs) · [core-0 monitor and fail-closed gate](firmware/src/thermal_demo.rs) · [core-1 BENCH](firmware/src/bench.rs) · [deadline state machine](safety-core/src/deadline.rs) · [watchdog integration](firmware/src/watchdog.rs) · [reset record](safety-core/src/reset_record.rs).

## Key design decisions and trade-offs
- **Four states:** ACTIVE (100% quota) and IDLE (10%) distinguish requested work from background activity; THROTTLED (35%) limits optional computation; SAFE (0%) blocks it on unsafe/unknown input. **Trade-off:** quotas control iteration counts, **not** clock frequency, watts, or measured utilization.
- **Hysteresis:** throttle at 70 °C, release below/equal 65 °C to avoid noisy threshold oscillation; leave SAFE only after **three valid samples at or below 60 °C**. These are *synthetic demo thresholds*, not RP2350 ratings.
- **Diagnostic-backed recovery:** a timer can wake while the worker executes **no useful instructions**. Therefore `completed_windows` alone is insufficient. A separate `verified_windows` advances only after the **96-round ALU self-check passes and the window is on time**; the monitor requires **three distinct completed *and* verified advances** before clearing a deadline fault. **Trade-off:** probe overhead and conservative recovery; this checks limited arithmetic progress, **not** full workload correctness or an independent safety channel.
- **Watchdog separation:** transient synthetic heat closes the gate without forcing a reset; persistent deadline/stall/diagnostic/monitor faults enter a 5 s grace policy, then sticky reset-pending and feed inhibition through the existing 8 s watchdog. Grace is evaluated at feed opportunities, not a proven response bound.

## Concurrency contract
Core 0 publishes `THERMAL_LIMIT`, `DEADLINE_FAULT` and `MONITOR_LAST_TICK_MS` via **Release** stores; core 1 reads the work gate with **Acquire** loads. Core 1 publishes `BENCH_RUNNING`, last-completion time, and completion/verified/miss/probe counters using **Release or AcqRel**, read by the core-0 monitor with **Acquire**. The diagnostic-only iteration tally uses **Relaxed** because it is telemetry, not an ordering fence. Publish monitor freshness **after** its decision; stale (>250 ms) or absent freshness forces quota 0. **Caveat:** the counters form individually atomic fields, **not one atomic snapshot**; intermediate reads may delay recovery. No formal linearizability proof or silicon memory/timing audit exists.

## Failure scenarios and expected policy
| Injected condition | Expected decision |
| --- | --- |
| Missing / out-of-range temperature | SAFE; application quota 0 |
| Critical synthetic temperature (≥85 °C) | SAFE; require cool consecutive samples to recover |
| Missed 100 ms worker period (>5 ms grace) or stalled progress (>250 ms) | Latch deadline fault; gate 0 |
| Failed ALU probe | `DiagnosticFailure`; no verified-window credit |
| Same persistent runtime fault past 5 s grace | Sticky `ResetPending`; withhold watchdog feed; encode reason for possible next boot |

## Verification — reproduce from repository root
CI's **Linux host** commands (Rust target installed, Python 3 available):
```sh
cargo test --locked -p safety-core --target x86_64-unknown-linux-gnu
cargo run --locked -p safety-core --example recovery_campaign --target x86_64-unknown-linux-gnu
mkdir -p reports
cargo run --locked -p safety-core --example fault_matrix --target x86_64-unknown-linux-gnu > reports/raw-fault-campaign.jsonl
python3 tools/verify_fault_report.py
cargo build --locked --release -p firmware --features thermal-demo
```
**Expected:** 42 Rust tests pass; recovery rejects timer-only progress; report verifier accepts **27/27** requirement-linked scenarios; firmware cross-build succeeds. Separately, CI runs **6 report-verifier tests**, two more RP2350 firmware variants and the [QEMU MPS2-AN505 Cortex-M33 smoke executable](safety-cortexm33-smoke/src/main.rs). **Recorded evidence:** [successful CI run, 12 Nov 2026](https://github.com/chirayukapoor5/embassy-supervisor/actions/runs/43253866401) ([historical commit](https://github.com/chirayukapoor5/embassy-supervisor/commit/fb6c1f786944b8e75efdb170da6b097aff71848d)); [SR-001–027 ⇄ FI-001–027](docs/verification/requirements.json) · [verification guide and artifact](docs/verification/README.md). These are recorded historical results, **not** a fresh run for this document.

## Limitations / review gates
QEMU runs **portable policy code** on emulated AN505, **not the dual-core RP2350 firmware**. Temperatures are scripted. No on-board watchdog reset, scratch retention, physical power/temperature readings, interrupt latency, WCET, hard-deadline or memory-ordering stress results are evidenced. The diagnostic shares the worker CPU (common-mode failure risk); scratch records are best-effort. **Before deployment:** instrument a real board, inject each fault, capture reset/reboot traces and measured timing margins, then repeat under load and power/reset cycling. No ASIL/ISO 26262 claim.
