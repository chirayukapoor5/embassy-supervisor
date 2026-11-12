/* QEMU MPS2 AN505 only. NOT RP2350. Place stack in SSRAM-0,
   which is accessible at address zero at secure boot. Keep code/RAM
   disjoint within the 4 MiB SSRAM-0 window. */
MEMORY
{
    FLASH : ORIGIN = 0x00000000, LENGTH = 128K
    RAM : ORIGIN = 0x00020000, LENGTH = 128K
}
