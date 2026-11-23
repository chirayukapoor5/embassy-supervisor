# Cortex-M33 Power, Thermal & Reliability Supervisor — Architecture

## Contribution and provenance

This fork adds an original safety application to Cedric Rivard's
\`embassy-supervisor\` workspace. The existing task supervisor, RP2350
application, USB networking, bootloader, OTA and watchdog infrastructure
originate upstream and retain their authorship and licenses.

The additions are the \`safety-core\` thermal/deadline/escalation policies,
an optional \`thermal-demo\` workload controller, bounded diagnostic-work
verification, reset-cause checking, automated tests and traceability reports,
and a separate Cortex-M33 QEMU policy smoke executable.

**This is an educational safety-engineering demonstration, not ISO 26262
or ASPICE certification.** All thermal thresholds are simulated examples.

## Task topology

\`\`\`mermaid
flowchart TB
  subgraph CORE0["RP2350 core 0: supervisor and safety demo"]
    TEMP["Scripted temperature input"] --> POLICY["Thermal SafetyController"]
    DEADLINE["DeadlineMonitor: lateness / stall / diagnostic mismatch"]
    POLICY --> GATE["Cross-core atomic workload gate"]
    DEADLINE --> GATE
    DEADLINE --> ESC["Persistent fault EscalationPolicy"]
    ESC --> WD["Existing hardware watchdog feeder"]
    ESC --> REASON["SCRATCH3: checked reason; SCRATCH5: BENCH inhibit"]
  end
  subgraph CORE1["RP2350 core 1: benchmark task"]
    WORK["Budgeted application computation"] --> PROBE["96-round bounded ALU diagnostic"]
    PROBE --> COUNTERS["Finished / verified / missed / failed counters"]
  end
  GATE --> WORK
  COUNTERS --> DEADLINE
  WD --> RESET["Hardware reset if feed withheld long enough"]
  REASON --> AFTER["Next boot: read-once reset reason"]
\`\`\`

The thermal-demo task and existing watchdog execute under Embassy scheduling.
The benchmark runs on core 1. The safety gates use atomic publication; there
is no independently clocked safety island or redundant hardware sensor.

## Runtime contract

| Aspect | Demonstration behavior | Limitation |
|---|---|---|
| Thermal | ACTIVE/IDLE/THROTTLED/SAFE; no workload at simulated 85°C | Not hardware operating limits |
| Benchmark | At most 50,000 baseline iterations per 100 ms window | Iteration budget is not a measured CPU clock/power control |
| Deadline | 100 ms target; >5 ms late means a miss | Scheduling latency not bounded on silicon |
| Stall | No completed window >250 ms | Monitor is cooperative on core 0 |
| Diagnostic | 96-round known-vector ALU computation per period | Shares worker CPU; detects limited fault classes |
| Recovery | Three distinct on-time periods whose diagnostic passed | Not evidence of full functional throughput |
| Escalation | Persistent fault grace of at least 5 s, evaluated every ~2 s | Feed check is not a hard timer |
| Watchdog | Existing RP2350 watchdog 8 s since last feed | Hardware reset has not been exercised |
| Reset evidence | SCRATCH3 versioned reason with byte complement; SCRATCH5 inhibits BENCH | Lost on power reset, possible stale/corrupted scratch |
| Arm runtime | Shared policy executes on QEMU MPS2 AN505 Cortex-M33 | RP2350 firmware not emulated |

A SAFE application-workload budget of zero **does not** mean all processing
stops: the bounded diagnostic is deliberately allowed to continue to prove
that core 1 can perform a small amount of integer work.

## Deadline recovery evidence

The worker publishes four distinct monotonic counters:
\`completed_windows\`, \`verified_windows\`, \`missed_windows\`,
\`failed_probes\`, plus its last completion timestamp.

A window increments \`verified_windows\` **only** if its diagnostic check
returns the known expected output and the window meets its deadline.

The monitor requires progress in both completed and verified counters to
advance the recovery latch. A failed probe itself immediately trips
\`DiagnosticFailure\`. A backward counter jump does not count as progress;
ordinary \`u32\` rollover does.

The counters are individually atomic, **not an atomic multi-field snapshot**.
The monitor can conservatively delay recovery if it samples during a
publication sequence. This implementation is not a formal memory-coherence
proof or an independently verified hardware supervisor.

## Reset evidence and startup inhibition

\`WATCHDOG.SCRATCH3\` stores a version marker, escalation reason code and
inverse-code check byte. A mismatched bit or unknown version is rejected.
\`take_safety_reset_reason\` clears the scratch field after a boot-time read.

\`bench_enabled\` uses both the existing SCRATCH5 flag and the prior reset
reason. A recognized safety escalation blocks automatic BENCH restart even
if the enable flag was left set. Manual starts after boot remain possible.

This proves the **software format and policy decision**, not physical
scratch retention, full reset-loop behavior, or reboot timing.

## Evidence tiers

1. **Host simulation:** Deterministic Rust unit tests and fault campaigns.
2. **Traceability:** Source-controlled requirement IDs mapped to fault cases,
   verified by a fail-closed report generator.
3. **Arm execution:** Shared \`no_std\` policies executed on the emulated
   Cortex-M33 CPU in the QEMU AN505 board.
4. **Cross-compilation:** Original firmware and thermal-demo variants built
   for \`thumbv8m.main-none-eabihf\`.
5. **Physical runtime validation:** Not yet performed.

For exact evidence, see [verification](../verification/README.md). The
repository does **not** claim ASIL allocation, ISO 26262 certification,
hardware WCET validation, physical energy data or RP2350 silicon tests.
