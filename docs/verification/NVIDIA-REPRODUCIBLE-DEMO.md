# NVIDIA reviewer demo — reproducible firmware reliability verification

**Repository:** [embassy-supervisor fork](https://github.com/chirayukapoor5/embassy-supervisor/tree/feature/power-thermal-supervisor) · **Scope:** original opt-in `safety-core` extension and RP2350 firmware integration · **Design:** [ENGINEERING-DESIGN-REVIEW.md](../../ENGINEERING-DESIGN-REVIEW.md) · **Review diff:** [PR #1](https://github.com/chirayukapoor5/embassy-supervisor/pull/1/files)

> **Evidence tier:** deterministic host software-model tests; QEMU MPS2-AN505 Arm Cortex-M33 **policy** execution; RP2350 **cross-compilation**. No physical RP2350 runtime, sensor, clock, power, watchdog-reset or worst-case execution-time measurement is claimed.

## 1. Two-minute reviewer walkthrough

1. Read the [fault recovery implementation](../../safety-core/examples/recovery_campaign.rs) and its [deadline monitor](../../safety-core/src/deadline.rs). The first injected diagnostic mismatch at **t=100 ms** latches `DiagnosticFailure`.
2. Run the recovery campaign below. At **t=500 ms**, timer-only completed windows **must not** clear the latch. At **t=800 ms**, three distinct verified diagnostic completions **must** clear it.
3. Run the 27-case fault matrix and fail-closed report validator. Inspect [requirements.json](requirements.json), `reports/fault-injection-results.json`, and `reports/fault-injection-report.md`.
4. Inspect [RP2350 shared gate](../../firmware/src/thermal_demo.rs), [core-1 BENCH counters](../../firmware/src/bench.rs), and [watchdog integration](../../firmware/src/watchdog.rs). Verify that the implemented restriction is an **iteration quota**, not a measured clock/power change.
5. Review the [QEMU smoke test](../../safety-cortexm33-smoke/src/main.rs), [CI workflow](../../.github/workflows/safety-core.yml), and physical-hardware limitations below.

## 2. Prerequisites and reproducible Linux commands

A Git checkout, stable Rust toolchain (`cargo`, `rustup`) with the `thumbv8m.main-none-eabihf` target, Python 3, and GitHub Actions-compatible firmware build dependencies. Run commands from the repository root. The historical CI used Linux with Rust stable; a different host toolchain can require adaptation. The ARM cross-build must have the repository's linker/configuration prerequisites.

```sh
git clone --branch feature/power-thermal-supervisor --single-branch \
  https://github.com/chirayukapoor5/embassy-supervisor.git
cd embassy-supervisor
git rev-parse HEAD
rustup target add thumbv8m.main-none-eabihf

# 42 host tests across safety-core test binaries on historical verified commit
cargo test --locked -p safety-core --target x86_64-unknown-linux-gnu

# Deterministic diagnostic-failure -> blocked timer-only recovery -> verified recovery
cargo run --locked -p safety-core --example recovery_campaign --target x86_64-unknown-linux-gnu

# Six report-validator refusal/acceptance tests and all 27 traceability scenarios
python3 -m unittest discover -s tools/tests -v
mkdir -p reports
cargo run --locked -p safety-core --example fault_matrix \
  --target x86_64-unknown-linux-gnu > reports/raw-fault-campaign.jsonl
python3 tools/verify_fault_report.py --sha "$(git rev-parse HEAD)"

# Compile the shared policy for Arm, and all three RP2350 firmware variants
cargo check --locked -p safety-core --target thumbv8m.main-none-eabihf
cargo build --locked --release -p firmware --features thermal-demo
cargo build --locked --release -p firmware --no-default-features --features thermal-demo
cargo build --locked --release -p firmware
```

**Expected recovery output**, from the scripted host example (logical timestamps, **not** physical timer measurements):

```text
t=100ms: injected diagnostic mismatch; fault latched
t=500ms: no recovery from timer-only progress
t=800ms: three bounded diagnostic completions verified
PASS: recoverability backed by verified computation, not timer wakes
```

**Expected fault validator output:** `PASS: 27 traced fault scenarios`; the structured report must identify 27 requirements, 27 corresponding fault cases, zero failed cases, and `hardware_executed: false`. Rust unit tests should report 42 passing cases across their constituent test binaries on the previously validated commit. All three build commands must exit zero.

## 3. Cortex-M33 instruction execution (not an RP2350 emulator)

On Ubuntu with `qemu-system-arm` installed:

```sh
sudo apt-get update -qq
sudo apt-get install -y --no-install-recommends qemu-system-arm
cargo build --locked --release -p safety-cortexm33-smoke \
  --target thumbv8m.main-none-eabihf
timeout 20s qemu-system-arm -M mps2-an505 -nographic -semihosting \
  -kernel target/thumbv8m.main-none-eabihf/release/safety-cortexm33-smoke
echo "QEMU process exit code: $?"
```

**Expected:** QEMU returns exit code `0`; the [emulated smoke binary](../../safety-cortexm33-smoke/src/main.rs) runs the shared no-std safety policy, verifies selected thermal/deadline/escalation transitions, and signals process success through semihosting. This is an Arm CPU policy smoke check **only**.

## 4. Recorded independent evidence

The [successful GitHub Actions run of 12 November 2026](https://github.com/chirayukapoor5/embassy-supervisor/actions/runs/43253866401), for commit [`fb6c1f786944b8e75efdb170da6b097aff71848d`](https://github.com/chirayukapoor5/embassy-supervisor/commit/fb6c1f786944b8e75efdb170da6b097aff71848d), recorded:

| Check | Result |
| --- | --- |
| Rust `safety-core` tests | **42 passed** |
| Deterministic traceability faults | **27/27 passed** |
| Report-verifier tests | **6 passed** |
| Diagnostic recovery / deadline / escalation campaigns | **Passed** |
| Arm Cortex-M33 QEMU MPS2-AN505 runtime policy smoke | **Passed** |
| Three `firmware` build configurations | **Compiled successfully** |

That run publishes the `safety-fault-campaign` report artifact. **These counts apply to that historical commit.** A subsequent GitHub Actions run, if triggered by this documentation commit, is separate evidence and must be checked on its own commit SHA. Do not describe historical counts as fresh local execution.

## 5. What to say during a live review

> “I extended an existing dual-core Embassy firmware benchmark with a host-testable Rust `no_std` safety policy. The monitor blocks optional work after invalid synthetic temperatures or stalled/failed work, while a bounded diagnostic still runs. I deliberately required verified diagnostic progress—not timer wakeups—to release the deadline fault latch. This demonstration injects a failure, checks blocked recovery, then shows recovery after three verified periods. I verified policy behavior with host test campaigns and Cortex-M33 QEMU, and cross-compiled the RP2350 integration. I have **not** measured hardware timing, physical watchdog resets, power or thermal behavior on an actual board.”

## 6. Open acceptance gates

To claim physical firmware validation, run this feature on an identified RP2350 board and firmware SHA; log real watchdog reset, boot-cause scratch registers, multicore gate behavior, deadline jitter, persistent and transient fault traces, and external power/temperature measurement as appropriate. Collect raw serial/defmt logs and repeatable setup steps. The diagnostic is on the same worker core; no independently clocked safety island exists. Do not claim ISO 26262 or ASPICE compliance from this demo.
