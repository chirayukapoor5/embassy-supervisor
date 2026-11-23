/* QEMU MPS2-AN505 ONLY, not RP2350.
   AN505 starts its secure vector table at 0x10000000 (INIT_SVTOR).
   The on-chip secure code window is at 0x10000000. */
MEMORY
{
    FLASH : ORIGIN = 0x10000000, LENGTH = 2M
    RAM : ORIGIN = 0x28000000, LENGTH = 2M
}
