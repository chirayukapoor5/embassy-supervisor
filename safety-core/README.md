# safety-core: hardware-independent thermal safety policy

This is an **original, additive** \`no_std\` crate in the embassy-supervisor fork. The existing task supervisor and firmware are upstream code; this crate is a policy component intended for later integration.

## Implemented policy

- Startup **SAFE** until 3 consecutive valid cool samples have been received.
- **ACTIVE** (100% policy cap) when work is requested; **IDLE** (10%) otherwise.
- **THROTTLED** (35%) at 70 °C or above, staying throttled until 65 °C or below.
- Immediate **SAFE** (0%) for missing sensor data, readings outside -40 to 125 °C, or temperature at/above 85 °C.
- Exit SAFE only after 3 consecutive valid samples at or below 60 °C. A failed or insufficiently cool sample resets the counter.
- Configurations are validated on initialization; there is no heap allocation or floating-point dependency.

**All thresholds and limits are illustrative simulation policies, not RP2350 electrical/thermal limits.** The 35% workload limit is a recommendation returned to the caller, not CPU-frequency control. This crate does not read a hardware temperature sensor, provide physical timing guarantees, or implement ISO 26262 certification. The firmware will need to enforce each decision and manage sensor sample freshness, watchdog policy, and task deadlines.

## Run on Windows (from repository root)

    cargo test -p safety-core --target x86_64-pc-windows-msvc
    cargo run -p safety-core --example simulate --target x86_64-pc-windows-msvc
    cargo check -p safety-core --target thumbv8m.main-none-eabihf

## Run on Linux (from repository root)

    cargo test -p safety-core --target x86_64-unknown-linux-gnu
    cargo run -p safety-core --example simulate --target x86_64-unknown-linux-gnu

## Further engineering work

1. Provide fresh validated temperature samples using an RP2350 sensor-adapter task or a deterministic test provider.
2. Add an Embassy task to periodically call SafetyController::update.
3. Enforce the returned workload cap in the bench task and stop nonessential work in SAFE.
4. Connect watchdog/liveness, verified deadline handling, event logging, and hazard-to-test traceability.

The library's test suite validates state transitions and safety policy without hardware.


## RP2350 firmware integration (opt-in)

The policy **is now wired to the existing second-core BENCH task** under the
\`thermal-demo\` firmware feature. It is not active in the default build.

- \`firmware/src/thermal_demo.rs\` runs scripted, **synthetic** temperature
  samples every 2 seconds on core 0, and publishes the safety decision through
  a release/acquire \`AtomicU8\`.
- \`firmware/src/bench.rs\` reads that limit from core 1 and executes at most
  50,000 (ACTIVE), 5,000 (IDLE), 17,500 (THROTTLED), or 0 (SAFE)
  xorshift iterations per 100 ms period.
- SAFE suspends benchmark computation, but the task remains alive to respond
  to supervisor shutdown requests. This is a cooperative budget, **not**
  physical thermal/power management or a hard deadline guarantee.
- \`GET /api/safety\` returns the synthetic temperature, mode, configured
  benchmark budget and the wrapping cumulative executed iteration count.
- The existing OTA, watchdog, network and supervisor task graph remain intact.
  The benchmark still needs to be started using the existing control API or
  the existing boot flag.

Build from the repository root:

    cargo build --release -p firmware --features thermal-demo
    cargo test -p safety-core --target x86_64-pc-windows-msvc

After flashing compatible RP2350 hardware and establishing USB networking,
the current demo's API can start the benchmark and inspect the applied quota:

    curl -X POST 'http://10.42.0.61/api/control?node=bench&op=start'
    curl 'http://10.42.0.61/api/safety'

**No hardware run has been performed.** The above endpoints are firmware
integration points, not a remotely hosted simulator. The \`thermal-demo\`
feature is a learning/demo path; replace its scripted values with a real
sensor adapter, freshness watchdog and independent safety verification before
using it with physical thermal control.


## Deadline monitoring and fault-recovery verification (new)

Under the opt-in \`thermal-demo\` firmware feature, BENCH on RP2350 core 1
now publishes actual **100 ms work-period completion** timestamps, completed
window count and missed deadline count. A period is marked missed if its
completion exceeds the 100 ms target by more than **5 ms** (demo jitter margin).

The independent core-0 safety task checks this data approximately every 50 ms:
- Task running with no completion for more than 250 ms -> **task-stalled**.
- A new missed-period counter value -> **missed-period**.
- Either fault closes the workload gate, so the benchmark executes **zero**
  application iterations while the fault is latched.
- Only **three distinct new, on-time completions** with fresh heartbeats
  can clear the latch; repeated polling, stopping the worker or temperature
  changes alone cannot clear it.
- If the safety monitor stops publishing for 250 ms, BENCH independently
  fails closed and allows zero application iterations.

No physical board or external temperature probe is required to run the tests.

    cargo test --locked -p safety-core --target x86_64-pc-windows-msvc
    cargo run --locked -p safety-core --example deadline_campaign --target x86_64-pc-windows-msvc
    cargo build --locked --release -p firmware --features thermal-demo

\`GET /api/safety\` (when running on an RP2350 board) also shows
\`deadline_fault\`, \`missed_windows\`, \`completed_windows\`,
\`monitor_fresh\`, \`deadline_recovery_remaining\` and
\`completion_age_ms\`.

On physical hardware, the pre-existing fault injection API can exercise a
**best-effort** stall experiment after BENCH is started:

    curl -X POST "http://10.42.0.61/api/fault?node=bench&kind=hog"
    curl "http://10.42.0.61/api/safety"

This is a **demonstration of detection and recovery policy**, not a proven
worst-case execution-time bound, independently verified safety mechanism, or
ISO 26262 compliance. The core-0 monitor shares the cooperative thread tier,
so it cannot promise an immediate response when that tier is stalled. Safety
watchdog escalation and physical-board timing validation remain separate work.


## Persistent-fault watchdog escalation (opt-in firmware)

The existing RP2350 8-second hardware watchdog now integrates a separate
\`no_std\` escalation policy (no second watchdog is instantiated):

1. BENCH deadline misses or task stalls are identified by the existing
   deadline monitor; an unresponsive safety monitor is a third cause.
2. The watchdog feeder samples these causes every approximately 2 seconds.
   A continuing, *same-reason* fault receives a configurable **5-second
   grace window**; transient faults clear the timer.
3. On escalation, the watchdog feeder latches RESET-PENDING, records the
   reason in \`WATCHDOG.SCRATCH3\`, disables the persistent BENCH autostart
   flag in \`SCRATCH5\` (if \`enabled-if\` is enabled), and **stops feeding**
   the watchdog. The existing hardware watchdog expires within about
   8 seconds of its last feed, assuming normal operation.
4. On next boot the diagnostic is read once, logged, exposed through
   \`GET /api/safety\` as \`previous_safety_reset\`, and cleared.
5. A transient thermal SAFE in the synthetic scenario **does not** trigger a
   reset. Stopping BENCH through normal control also does not by itself
   trigger escalation.

The grace time is evaluated only at feed opportunities, so it is **not** a
5-second hard deadline. Hardware reset and scratch-register persistence are
not physically tested. A stall of the thread executor itself may prevent the
diagnostic from being written even though the hardware watchdog eventually
resets the chip. The watchdog does not prevent a crash before the feed loop.
The benchmark autostart flag is cleared to avoid a demo reset loop.

\`GET /api/safety\` now includes \`watchdog_escalation_stage\`,
\`watchdog_escalation_reason\`, \`watchdog_escalation_age_ms\` and
\`previous_safety_reset\`, in addition to thermal and deadline information.

Verify with host-only tests:

    cargo test --locked -p safety-core --target x86_64-pc-windows-msvc
    cargo run --locked -p safety-core --example escalation_campaign --target x86_64-pc-windows-msvc

Firmware cross-build:

    cargo build --locked --release -p firmware --features thermal-demo

**The watchdog path has been cross-compiled, not exercised on silicon.**
The policy is an educational reliability demonstration, not a certified
safety-monitor architecture.


## Bounded diagnostic-work recovery and checked reset causes

The core-1 BENCH task now executes an additional **96-round deterministic
integer diagnostic** each period, including in SAFE where all application
iterations are disabled. A \`verified_windows\` counter increments only
when that computation passes and the period was on time. Three distinct,
new verified completions are required to release a latched deadline fault.

A diagnostic mismatch raises \`DiagnosticFailure\`, closes the application
workload gate, and is eligible for the existing watchdog escalation policy.
Counter rollback does not qualify as progress; normal u32 rollover does.

RP2350 SCRATCH3 now uses a versioned and complement-checked reset reason
format, read once at boot. Previous escalation inhibits BENCH auto-start even
if the startup flag is unexpectedly still set.

These are **software safety-policy checks**, not independent hardware
diagnostics, power measurements, or certified functional-safety evidence.
Physical watchdog resets and scratch retention remain untested.

Run \`cargo run -p safety-core --example recovery_campaign --target
x86_64-pc-windows-msvc\` to reproduce the diagnostic-gated recovery model.
See [architecture](../docs/architecture/README.md) and
[traceability](../docs/verification/README.md).
