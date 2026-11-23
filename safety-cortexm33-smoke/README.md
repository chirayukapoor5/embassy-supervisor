# Cortex-M33 runtime verification using QEMU MPS2-AN505

This is an independent bare-metal executable that links the SAME `safety-core` Rust library used by the RP2350 firmware. It tests thermal, deadline and escalation policy branches on an emulated Arm Cortex-M33 CPU.

The dedicated QEMU AN505 linker layout is intentionally separate from the RP2350 binary and its bootloader.

Run in Linux/WSL2:

```bash
sudo apt-get install qemu-system-arm
rustup target add thumbv8m.main-none-eabihf
cargo build --locked --release -p safety-cortexm33-smoke --target thumbv8m.main-none-eabihf
timeout 20s qemu-system-arm -M mps2-an505 -nographic -semihosting \
  -kernel target/thumbv8m.main-none-eabihf/release/safety-cortexm33-smoke
```

QEMU exits with status 0 on policy success and nonzero on a failed policy check. A hang triggers timeout and fails CI.

**Scope:** Arm instruction/runtime execution of policy only. Not RP2350 watchdog, peripherals, dual core, USB, ADC, actual scheduling, physical reset or worst-case timing.
