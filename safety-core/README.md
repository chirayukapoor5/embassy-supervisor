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
