> **Reviewer / NVIDIA firmware entry point (this fork):** [**Cortex-M33 Power & Thermal Supervisor — case study**](NVIDIA-FIRMWARE-CASE-STUDY.md) · [**My changes: draft PR #1**](https://github.com/chirayukapoor5/embassy-supervisor/pull/1/files) · [**Policy source**](safety-core/src/lib.rs) · [**Firmware integration**](firmware/src/thermal_demo.rs) · [**Verified CI evidence**](https://github.com/chirayukapoor5/embassy-supervisor/actions/runs/43253866401).
>
> **Evidence boundary:** 42 host Rust tests, 27 deterministic fault scenarios and Cortex-M33 QEMU policy smoke succeeded in the linked **12 Nov 2026 historical CI run**; three firmware variants compiled. **Physical RP2350 validation is not documented or supported by the linked evidence.** The underlying Embassy framework and original board application are [upstream work](https://github.com/cedrivard/embassy-supervisor); the case study isolates this fork's original contributions.

---

<div align="center">

# embassy-supervisor

**Run-time supervision for bare-metal async Rust.**

Dependency-ordered task lifecycle, elastic pools, runtime control and verified dataflow
for [embassy](https://embassy.dev) firmware. No kernel, no per-task stacks, no allocator.

[![crates.io](https://img.shields.io/crates/v/embassy-supervisor.svg)](https://crates.io/crates/embassy-supervisor) [![docs.rs](https://docs.rs/embassy-supervisor/badge.svg)](https://docs.rs/embassy-supervisor) [![CI](https://github.com/cedrivard/embassy-supervisor/actions/workflows/ci.yml/badge.svg)](https://github.com/cedrivard/embassy-supervisor/actions/workflows/ci.yml) [![MSRV](https://img.shields.io/crates/msrv/embassy-supervisor.svg)](#compatibility) [![no_std](https://img.shields.io/badge/no__std-yes-success.svg)](#compatibility) [![license](https://img.shields.io/crates/l/embassy-supervisor.svg)](#license)

[Website](https://embassy-supervisor.github.io/) · [Playground](https://embassy-supervisor.github.io/playground/) · [Library reference](supervisor/README.md) · [API docs](https://docs.rs/embassy-supervisor) · [Demo firmware](firmware/README.md)

[<img src=".github/assets/playground.png" alt="The embassy-supervisor playground running the cellular asset tracker scenario: the graph declaration beside the live task graph" width="800">](https://embassy-supervisor.github.io/playground/)

</div>

---

## Fork extension: Cortex-M33 Power, Thermal & Reliability Supervisor

**Embedded Rust | Arm Cortex-M33 | Embassy async | Dual-core RP2350 | Watchdog | Fault injection | QEMU**

This fork extends the existing [embassy-supervisor](https://github.com/cedrivard/embassy-supervisor) firmware with an **original, hardware-independent Rust `no_std` safety-policy library** and opt-in integration with its RP2350 benchmark task. The work addresses a concrete embedded reliability problem: **detect abnormal temperature samples, missed execution deadlines, task stalls and failed diagnostic checks; restrict nonessential computation; and require evidence of recovery before restoring work.**

> **Contribution boundary:** The core `embassy-supervisor` framework, original RP2350 firmware, USB networking, OTA, bootloader and base watchdog are upstream work by **Cedric Rivard and contributors**. The fork-specific additions described below are the thermal/deadline/escalation policies, workload integration, bounded diagnostic verification, reset records, tests, fault-campaign reporting and QEMU policy smoke test. All upstream license and attribution notices are retained.

[**NVIDIA firmware case study**](NVIDIA-FIRMWARE-CASE-STUDY.md) · [**Architecture**](docs/architecture/README.md) · [**Safety policy source**](safety-core/src/lib.rs) · [**Requirements & verification**](docs/verification/README.md) · [**Draft pull request**](https://github.com/chirayukapoor5/embassy-supervisor/pull/1) · [**Verified safety CI run**](https://github.com/chirayukapoor5/embassy-supervisor/actions/runs/43253866401)

### What I implemented

| Engineering area | Fork-specific implementation |
| --- | --- |
| **Thermal safety state machine** | Four operating modes — ACTIVE, IDLE, THROTTLED and SAFE — with simulated sensor plausibility checks, configurable thresholds, hysteresis, fail-safe startup and guarded recovery. |
| **Dual-core workload control** | An opt-in `thermal-demo` task publishes atomic workload limits to the existing benchmark on RP2350 core 1. SAFE disables application iterations; THROTTLED and IDLE apply reduced computation quotas. |
| **Execution deadline monitoring** | Tracks 100 ms benchmark periods, records late completions using a 5 ms demonstration jitter margin and detects stalled progress after 250 ms. These are *policy settings*, not proven hardware WCET limits. |
| **Diagnostic-backed recovery** | Executes a bounded **96-round arithmetic self-check** even while application computation is blocked. Recovery requires **three distinct on-time periods** with successful diagnostic work; timer wakeups alone cannot clear the fault latch. |
| **Fault escalation and watchdog** | Extends the **existing** watchdog feed path with a persistent-fault escalation policy (5 s grace window, 8 s hardware watchdog configuration). Diagnostic, deadline, task-stall and monitor-staleness faults can trigger a sticky reset-pending decision. |
| **Reset evidence and safer startup** | Encodes the escalation reason with a version marker and complement check in watchdog scratch register 3; inhibits automatic benchmark restart after a recognized previous escalation. |
| **Verification and traceability** | Maps 27 requirements to 27 deterministic fault-injection cases; validates report completeness and generates JSON/Markdown evidence in GitHub Actions. |
| **Arm portability** | Runs representative `safety-core` logic on an **emulated Cortex-M33 CPU** using QEMU MPS2-AN505, separately from cross-compiling the RP2350 firmware. |

### Architecture at a glance

```text
Synthetic temperature input ──> Thermal SafetyController ─┐
                                                            ├──> Atomic workload gate
RP2350 core 1: BENCH task ──> Period + diagnostic counters │         │
                                  │                        │         v
                                  v                        │    BENCH work quota
                   Deadline / progress monitor ────────────┘    (0–100% of iterations)
                                  │
                                  v
                        Escalation policy
                                  │
                                  v
                        Existing watchdog feeder
                                  │
                    Reset cause + startup inhibition
```

This design integrates with Embassy's existing async task lifecycle and multicore application. The `safety-core` policies are dependency-free and testable on a host independently of the board.

### Verification evidence

The following results were recorded in [GitHub Actions run #43253866401](https://github.com/chirayukapoor5/embassy-supervisor/actions/runs/43253866401):

| Verification | Result | Evidence boundary |
| --- | --- | --- |
| Rust policy unit/integration tests | **42 passed** | Host software |
| Requirement-linked fault injections | **27/27 passed** | Deterministic simulated inputs and counters |
| Report-generator validation tests | **6 passed** | Missing/incorrect evidence rejected |
| Diagnostic recovery campaign | **Passed** | Proved that timer-only progress does not release a fault |
| Cortex-M33 QEMU runtime smoke | **Passed** | The shared policy executed on an emulated Arm CPU |
| RP2350 firmware build matrix | **3 variants compiled** | Cross-compilation, **not** physical runtime testing |

The CI run publishes a **`safety-fault-campaign`** artifact with raw JSONL evidence, machine-readable results and a reviewer-friendly Markdown report. The requirements manifest is in [`docs/verification/requirements.json`](docs/verification/requirements.json).

### Reproduce the verification

From the repository root on **Windows PowerShell**:

```powershell
git switch feature/power-thermal-supervisor
git pull --ff-only

cargo test -p safety-core --target x86_64-pc-windows-msvc
cargo run -p safety-core --example recovery_campaign --target x86_64-pc-windows-msvc

New-Item -ItemType Directory -Force reports | Out-Null
cargo run -p safety-core --example fault_matrix --target x86_64-pc-windows-msvc > reports/raw-fault-campaign.jsonl
python tools/verify_fault_report.py

cargo build --release -p firmware --features thermal-demo
```

QEMU is run automatically in CI; and alsi verify offline using RP2350 board to run the host campaigns or review the reports. For the full setup, see [the verification guide](docs/verification/README.md).

### Project description for applications

**Cortex-M33 Power, Thermal & Reliability Supervisor** — Developed an additive `no_std` Rust safety layer for an Embassy-based dual-core RP2350 firmware project. Implemented thermal state transitions, cross-core workload throttling, deadline and stall monitoring, bounded diagnostic-based recovery, persistent-fault watchdog escalation, and integrity-checked reset diagnostics. Verified the policies through **42 Rust tests, 27 traced fault-injection scenarios, 6 report-validator tests, Cortex-M33 QEMU execution, and three target firmware builds**.

**Resume-ready technical bullets (describe the fork-specific work):**

- Engineered a `no_std` Rust safety-policy layer for Cortex-M33 firmware, combining four thermal states, hysteresis and atomic cross-core workload gating to suspend nonessential computation on simulated critical faults.
- Implemented 100 ms deadline monitoring, 250 ms task-stall detection and a 96-round bounded diagnostic probe; required three verified on-time diagnostic periods before clearing a latched fault.
- Integrated persistent-fault escalation with the existing watchdog feed path, added corruption-checked reset records, and verified behavior with **42 automated Rust tests, 27 requirement-linked fault scenarios and QEMU Cortex-M33 policy execution**.

**Technology:** Rust (`no_std`), Embassy, Arm Cortex-M33, RP2350, atomics, watchdog supervision, fault injection, QEMU, Python, GitHub Actions, CI.

**Validation limits:** This is a *software-verified, firmware-cross-compiled demonstration*. Temperatures are synthetic; QEMU executes the portable policy on **AN505, not the RP2350 board**. The real dual-core runtime, physical watchdog reset, scratch retention, thermal behavior, worst-case timing and energy consumption have **not** been validated on silicon. The shared-core diagnostic is not independent safety hardware; no ISO 26262 certification, ASIL claim or Automotive SPICE compliance is implied.

---

## Overview

embassy gives firmware an RTOS-like task model at close to bare-metal cost. It does not
provide the lifecycle: which task starts before which, what stops before sleep, how workers
scale under load, how an update is applied safely.

`embassy-supervisor` is that layer. You declare the task graph once; the supervisor brings
it up in dependency order, tears it down in reverse, scales pools, and serves runtime
control, across thread-mode executors, interrupt executors and a second core.

- **Checked at compile time.** Cycles, unknown or duplicate deps, duplicate names and bad
  pool bounds are compile errors. The topological order is computed during expansion.
- **Fails closed.** A missing resource, a dependency that never becomes ready and a missed
  shutdown ack each return a `NodeFault` naming the node. None of them panics.
- **Small and portable.** `no_std`, `#![forbid(unsafe_code)]`, no allocator, HAL-agnostic.
  Depends only on embassy crates and `portable-atomic`.
- **Pay for what you use.** Control plane, pools, dataflow, tracing and health are Cargo
  features. A minimal build is the dependency-ordered core.

## Quick start

```sh
cargo add embassy-supervisor
```

```rust
use embassy_executor::Spawner;
use embassy_supervisor::{Supervisor, TaskNode, supervisor_graph};

// One declaration generates the node statics and a single `GRAPH` bundling the
// node slots, deps and compile-time order. APP depends on NET, so it starts after it.
supervisor_graph! {
    node NET = Terminate, task: net_task;
    node APP = Terminate, deps: [NET], task: app_task;
}

async fn net_task(node: &'static TaskNode) { /* ... */ }
async fn app_task(node: &'static TaskNode) { /* ... */ }

#[embassy_executor::task]
async fn supervisor_task(spawner: Spawner) {
    let sup = Supervisor::new(&GRAPH); // infallible: a cycle is a compile error
    if let Err(fault) = sup.start(&spawner).await {
        panic!("supervisor: {fault}"); // names the node and the cause
    }
}
```

### With runtime control and pools

`start()` brings the graph up and returns. With the `control` or `pool` feature, `run()`
replaces it: it brings the graph up best-effort, then serves control commands and pool
scaling. It returns only when a node misses its shutdown ack or a restart is refused.

```sh
cargo add embassy-supervisor --features control,pool
```

```rust
use embassy_supervisor::{ControlOp, request_control};

// In the supervisor task:
let fault = sup.run(&spawner).await;
panic!("supervisor: {fault}");

// Anywhere else, such as an HTTP handler. Stopping NET stops APP first.
request_control(&NET, ControlOp::Deactivate).await;
```

Next steps:

- Try a graph in the [in-browser playground](https://embassy-supervisor.github.io/playground/),
  which runs the real crate compiled to WebAssembly.
- Read the [reference guide](https://docs.rs/embassy-supervisor/latest/embassy_supervisor/guide/index.html): model, lifecycle matrix,
  `supervisor_graph!` DSL, dataflow supervision and recipes.
- Flash the [demo firmware](firmware/README.md) on an RP2350.

## Architecture

### From declaration to running tasks

```mermaid
flowchart LR
    FRAG["supervisor_fragment!<br/>one per module or crate"]
    DSL["supervisor_graph!<br/>or compose_graph!"]
    GEN["static GRAPH and task shells<br/>node slots · deps · order · pools"]
    SUP["Supervisor task<br/>one per graph"]

    FRAG -- composed into --> DSL
    DSL -- generates --> GEN
    GEN --> SUP

    subgraph TIERS["Spawn targets, picked per node with executor:"]
        T0["Thread executor<br/>cooperative tier, the default"]
        IE["Interrupt executor<br/>preemptive tier"]
        C1["Second core<br/>no migration"]
    end
    POOL["Elastic pool<br/>min..max workers, on any tier"]

    SUP --> T0
    SUP --> IE
    SUP --> C1
    SUP <-- scales --> POOL
```

The graph is validated where it is written. Fragments let a module or a whole crate declare
its own slice, and `name:` lets one binary hold several graphs, so a subsystem can be a
sub-graph that an app state machine brings up and drops per phase.

Each node runs on the executor its `executor:` annotation names:

| Tier | Scheduling | Typical use |
|------|------------|-------------|
| Thread executor (default) | Cooperative, yields at `.await` | Networking, servers, housekeeping |
| Interrupt executor | Preempts the thread tier | Latency-sensitive sensor and control work |
| Second core | Own thread executor, no task migration | Compute load, whole pools |

The supervisor never runs task code itself. It spawns, parks and tears down tasks on these
tiers and scales the pools.

### Run-time behavior

```mermaid
flowchart TD
    APP["Anywhere in the app<br/>an HTTP handler, a button, an ISR"]
    MBOX["Control mailbox<br/>request_control, or try_request_control"]
    PROV["A provider on a bound edge<br/>set_ready or clear_ready"]
    SUP["Supervisor<br/>run: start, then one driver loop"]
    UP["start / activate<br/>dependency order, every spawn gated"]
    DOWN["deactivate / teardown<br/>dependents first, every stop acked"]
    SCALE["pool_pass<br/>grow under load, shrink after a cooldown"]
    BIND["apply_bind<br/>stops bound dependents, brings them back"]
    MON["monitor<br/>polices beat_timeout: beside the loop"]
    BUSY["Resource or readiness missing<br/>→ ResourceMissing or ReadyDepTimeout, naming the node"]
    TO["Ack missed<br/>→ ShutdownTimeout naming the node"]
    HEALTH["wait_health<br/>Stale or Recovered, report only"]

    APP --> MBOX --> SUP
    PROV --> SUP
    SUP --> UP
    SUP --> DOWN
    SUP --> SCALE
    SUP --> BIND
    SUP --> MON
    UP -.-> BUSY
    DOWN -.-> TO
    MON -.-> HEALTH
```

Both failure edges are returned values. Bring-up refuses to spawn a task whose peripherals
or dependencies are not ready, and a node that misses its shutdown ack is named to the
application, which decides whether to retry, log or reset. The monitor only reports: a
stalled node arrives as a `Stale` event, and the supervisor does not stop it.

The mailbox needs the `control` feature, pools `pool`, bound edges `bound-deps` and the
monitor `liveness-monitor`.

### Anatomy of a node

```mermaid
flowchart LR
    NODE["A node<br/>Terminate · Pause · OnDemand"]
    RES["resources:<br/>a value handed over at spawn, from main or a provides: node"]
    STATE["state:<br/>heap per activation, freed on exit"]
    DONE["completion<br/>a task that returns is observed; exit: keeps its value"]
    READY["readiness<br/>dependents wait for a state it asserts, where no value crosses"]
    LIVE["liveness<br/>a heartbeat a watchdog can read"]
    TRACE["trace<br/>ticks · polls · stall detection"]

    NODE --> RES
    NODE --> STATE
    NODE --> DONE
    NODE --> READY
    NODE --> LIVE
    NODE -.-> TRACE
```

Only the mode is required. What each operation does per mode is in the
[lifecycle reference](https://docs.rs/embassy-supervisor/latest/embassy_supervisor/guide/model/index.html#lifecycle-reference).

## Features

Rows marked *default* need no extra Cargo feature.

| Capability | Cargo feature | Description |
|------------|---------------|-------------|
| Ordered bring-up and teardown | *default* | Compile-time order up, reverse order down |
| Lifecycle modes | *default* | `Terminate`, `Pause` and `OnDemand` |
| Executor placement | *default* | `executor:` targets an interrupt tier or a second core |
| Resource threading | *default* | `resources:` hands owned values to a task at spawn |
| Runtime control | `control` | A stop cascades to dependents, a start to dependencies |
| Elastic pools | `pool` | Workers grow under load and shrink after a cooldown |
| Readiness gating | `readiness` | `deps: [NET ready]` waits for `set_ready()` |
| Restart | `restart` | Restarts a node together with its dependents |
| Restart budget | `restart-intensity` | `restart_budget: 3 in 60000` faults a crash loop |
| Settings-gated bring-up | `enabled-if` | `enabled_if:` decides at `start()` if a node comes up |
| Declared dataflow | `coupling` | `reads:` and `writes:` name the signals tasks exchange |
| Derived dataflow | `dataflow` | `#[dataflow]` derives the tables from the task body |
| Polled liveness | `coupling-observe` | A declared write is polled as the node's heartbeat |
| Heartbeat | `liveness` | `beat()` and a staleness check per node |
| Watchdog gate | `liveness-monitor` | `GRAPH.all_fresh()` gates the hardware watchdog feed |
| Lifecycle events | `lifecycle-events` | Every start, stop, exit and fault, named by node |
| Bring-up breadcrumb | `breadcrumb` | Names the node that hung bring-up, after the reset |
| Tracing | `trace` | CPU ticks and poll counts per task, idle time per executor |

The complete list is under
[Cargo features](supervisor/README.md#cargo-features).

The supervisor does not allocate, own a HAL, manage power states or catch panics. Pair it
with a hardware watchdog for crashes and the liveness heartbeat for stalled tasks.

## When to use it

The supervisor pays off on devices that run several interdependent services and have to
manage power and updates:

- **Battery-powered sensor nodes**: bring the radio and stack up in order, sample, publish,
  tear down, deep-sleep.
- **Gateways and edge hubs**: strict start-up order, elastic worker pools, runtime
  reconfiguration.
- **Building and motion controllers**: a preemptive control tier beside cooperative
  telemetry, with graceful degradation.
- **Field-updatable products**: an update drains live services, frees resources, swaps,
  and rolls back on failure.

It composes with a fallible heap (stopping a subsystem returns its memory), deep-sleep
power management (services drain in dependency order before the rails drop) and A/B OTA
update with rollback. The demo firmware runs the OTA path end to end.

## Demo firmware

[`firmware/`](firmware/) is a complete RP2350 application that exercises every part of the
supervisor:

- USB networking with an HTTP control and observability plane
- An elastic pool of keep-alive workers
- A heartbeat on the interrupt-priority tier and a control-started compute load on the
  second core
- OTA update with A/B swap and rollback

Build and run steps, the per-task breakdown and a guide to the observability data are in
[`firmware/README.md`](firmware/README.md).

## Tooling

Two host tools read a graph straight out of its Rust source: `supervisor-mermaid` draws it
as a Mermaid flowchart, and `supervisor-lint` reports signals that are read but never
written, or written but never read.

```sh
cargo install embassy-supervisor-tools
cd firmware && supervisor-mermaid
```

Prebuilt binaries for Linux, macOS and Windows are attached to each
`embassy-supervisor-tools-v*` [release](https://github.com/cedrivard/embassy-supervisor/releases).
Options and output formats are in [`supervisor-tools/README.md`](supervisor-tools/README.md).

## Workspace

| Crate | Description |
|-------|-------------|
| [`supervisor/`](supervisor/) | The `embassy-supervisor` library. [Reference](supervisor/README.md) |
| [`supervisor-macros/`](supervisor-macros/) | Proc-macros: the graph declaration and the `#[dataflow]` attribute |
| [`supervisor-syntax/`](supervisor-syntax/) | The graph DSL's parser and AST, shared by the proc-macros and the tooling |
| [`supervisor-observe/`](supervisor-observe/) | Observation traits and the `Counted` wrapper for signal libraries |
| [`supervisor-tools/`](supervisor-tools/) | The `supervisor-mermaid` and `supervisor-lint` host tools |
| [`firmware/`](firmware/) | The demo application |
| [`bootloader/`](bootloader/) | The embassy-boot A/B bootloader the demo's OTA swaps against |

## Compatibility

`#![no_std]` and `#![forbid(unsafe_code)]`. The minimum Rust version and the embassy
versions each release targets are listed under
[no_std / MSRV](supervisor/README.md#no_std--msrv) in the library reference.

Upgrade notes are in the [migration guide](supervisor/README.md#migration) and the crate
changelogs.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
