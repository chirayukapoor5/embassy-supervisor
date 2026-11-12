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

## Next integration

1. Provide fresh validated temperature samples using an RP2350 sensor-adapter task or a deterministic test provider.
2. Add an Embassy task to periodically call SafetyController::update.
3. Enforce the returned workload cap in the bench task and stop nonessential work in SAFE.
4. Connect watchdog/liveness, verified deadline handling, event logging, and hazard-to-test traceability.

The library's test suite validates state transitions and safety policy without hardware.
