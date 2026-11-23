# NVIDIA firmware case study — Cortex-M33 power, thermal & reliability supervision

**Status: software-verified prototype; NOT physically validated on RP2350 silicon.**  
**My contribution:** an additive Rust `no_std` safety-policy crate, opt-in RP2350 BENCH integration, bounded diagnostics, watchdog-escalation policy, reset-record encoding, fault campaigns, and QEMU smoke coverage. **Not my work:** the upstream [Embassy Supervisor](https://github.com/cedrivard/embassy-supervisor) task-supervision framework, original RP2350 firmware, USB/OTA/bootloader, or underlying watchdog driver. Existing authorship and licenses are preserved.

**Design review (maintainer brief):** [ENGINEERING-DESIGN-REVIEW.md](ENGINEERING-DESIGN-REVIEW.md) — architecture, decisions, concurrency, failure handling, test commands and validation limits.

**Start here:** [Review the complete change in draft PR #1](https://github.com/chirayukapoor5/embassy-supervisor/pull/1) · [Architecture](docs/architecture/README.md) · [Verification/traceability](docs/verification/README.md) · [Historical successful CI run (12 Nov 2026)](https://github.com/chirayukapoor5/embassy-supervisor/actions/runs/43253866401).

## Engineering problem and approach

A dual-core embedded benchmark must not continue optional computation when a synthetic thermal input is implausible/critical, a period overruns, a worker stops making progress, or a diagnostic fails. Restoring normal work must require **verified progress**, not mere timer wakeups.

1. **Policy without board dependencies:** [`SafetyController`](safety-core/src/lib.rs) chooses ACTIVE / IDLE / THROTTLED / SAFE, applies hysteresis and recovery checks, and outputs **iteration quotas** (100/10/35/0%); these are **not measured clock, power or CPU-utilization values**.
2. **Cross-core enforcement:** the opt-in [thermal/deadline monitor](firmware/src/thermal_demo.rs) publishes atomic limits to the existing core-1 [BENCH worker](firmware/src/bench.rs). The worker publishes completion, overrun and diagnostic counters; a stale monitor closes the application-work gate.
3. **Recovery with actual work:** a [96-round bounded arithmetic diagnostic](safety-core/src/diagnostic.rs) continues even in SAFE, while the [deadline monitor](safety-core/src/deadline.rs) requires **three distinct on-time, diagnostic-verified windows** to reopen the gate. The nominal period is 100 ms and demonstration stall threshold 250 ms; neither is a proven hard-real-time bound.
4. **Sticky escalation and reset evidence:** the [escalation policy](safety-core/src/escalation.rs) withholds permission to feed the **existing** watchdog after a persistent 5 s failure. [Firmware watchdog integration](firmware/src/watchdog.rs) uses the existing 8 s watchdog configuration; [reset-record encoding](safety-core/src/reset_record.rs) and [boot integration](firmware/src/bootflag.rs) provide a version/complement check and BENCH auto-start inhibition after a recognized prior escalation. These are designed behaviors, **not board-observed reset results**.

```text
Synthetic temperature --> no_std thermal policy ----+
                                                     +--> atomic BENCH work gate --> core-1 iterations
Core-1 counters ------> deadline/probe monitor -----+              |
         ^                                                          |
         +---------- bounded 96-round diagnostic (also in SAFE) -----+
Deadline / stall / diagnostic faults --> escalation --> existing watchdog feed path
                                                \--> reset reason / next-boot inhibit
```

## Verified results and where to inspect them

The following were **recorded on the historical CI commit** [`fb6c1f7`](https://github.com/chirayukapoor5/embassy-supervisor/commit/fb6c1f786944b8e75efdb170da6b097aff71848d) in [successful GitHub Actions run #43253866401](https://github.com/chirayukapoor5/embassy-supervisor/actions/runs/43253866401), **not newly rerun for this case-study edit**.

| Verification | Recorded result | Inspect implementation / evidence |
| --- | --- | --- |
| Host Rust safety-policy tests | **42 passed, 0 failed** | [Test modules](safety-core/tests/) · [CI safety-core job](https://github.com/chirayukapoor5/embassy-supervisor/actions/runs/43253866401) |
| Requirements-linked synthetic fault injection | **27/27 scenarios passed** | [Requirements manifest](docs/verification/requirements.json) · [Fault matrix](safety-core/examples/fault_matrix.rs) · CI artifact `safety-fault-campaign` |
| Report-verifier behavior | **6 passed** | [Validator tests](tools/tests/test_verify_fault_report.py) · [Report validator](tools/verify_fault_report.py) |
| Recovery and escalation simulations | **Passed** | [Recovery campaign](safety-core/examples/recovery_campaign.rs) · [Deadline campaign](safety-core/examples/deadline_campaign.rs) · [Escalation campaign](safety-core/examples/escalation_campaign.rs) |
| Emulated Arm Cortex-M33 execution | **Passed** on QEMU MPS2-AN505 | [Standalone smoke binary](safety-cortexm33-smoke/src/main.rs) · [CI recipe](.github/workflows/safety-core.yml) |
| RP2350 firmware builds | **3 variants compiled** | [CI build matrix](.github/workflows/safety-core.yml) · [Firmware integration](firmware/src/main.rs) |

**Evidence boundary:** the QEMU smoke binary executes the shared safety policy on an **emulated MPS2-AN505 Cortex-M33**; it does not boot or emulate the dual-core RP2350 application. Build success is not silicon validation. Download the `safety-fault-campaign` artifact from the linked CI run to inspect JSONL, JSON and Markdown reports.

## Known limitations and next acceptance steps

**Not tested on physical RP2350 hardware:** the thermal stream is synthetic; there are no measured on-silicon workload deadlines/WCET, power/temperature readings, watchdog timeouts, scratch-register retention, multicore timing results, or physical reset/recovery traces. The diagnostic shares the benchmark CPU and is **not independent safety hardware**. No automotive functional-safety certification or ASIL claim is made.

**To earn a “physically validated” claim:** flash an identified RP2350 board and exact firmware commit; capture on-device serial/defmt traces for normal-to-throttle-to-SAFE transitions, a missed deadline/stall, diagnostic fault, recovered verified windows, persistent-fault watchdog timeout, and SCRATCH3 reset-cause readback after reboot; measure the execution/timing margins and repeat under power-cycle/reset conditions. Publish raw logs, toolchain, board revision, setup, procedures, and observed results with a reproducible CI/build commit. Until then this is **software verification and firmware cross-compilation only**.

**Reviewer shortcuts:** [Policy](safety-core/src/lib.rs) · [Deadline](safety-core/src/deadline.rs) · [Diagnostic](safety-core/src/diagnostic.rs) · [RP2350 monitor](firmware/src/thermal_demo.rs) · [Core-1 worker](firmware/src/bench.rs) · [Watchdog integration](firmware/src/watchdog.rs) · [Draft PR diff](https://github.com/chirayukapoor5/embassy-supervisor/pull/1/files).
