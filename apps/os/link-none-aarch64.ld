/* Bare-metal aarch64 image layout (Pi 4 / QEMU raspi4b).
 *
 * Firmware loads kernel8.img at 0x80000 and jumps to its first byte, so
 * `.text.boot` (the arch _start boot arm) must be the first output bytes.
 */
ENTRY(_start)

SECTIONS
{
    . = 0x80000;

    .text : {
        KEEP(*(.text.boot))
        *(.text*)
    }

    .rodata : {
        *(.rodata*)
    }

    .data : {
        *(.data*)
    }

    /* 8-aligned bounds: the _start BSS clear stores xzr in 8-byte steps. */
    .bss (NOLOAD) : {
        . = ALIGN(8);
        __bss_start = .;
        *(.bss*)
        *(COMMON)
        . = ALIGN(8);
        __bss_end = .;
    }

    /* Boot stack: 512 KiB above BSS, full-descending from __stack_top. */
    . = ALIGN(16);
    . += 0x80000;
    __stack_top = .;

    /DISCARD/ : {
        *(.eh_frame*)
        *(.note*)
    }
}
