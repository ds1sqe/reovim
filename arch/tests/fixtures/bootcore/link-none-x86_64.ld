/* Bare-metal x86_64 image layout (QEMU q35, Multiboot1).
 *
 * QEMU's `-kernel` Multiboot1 loader reads the ELF, places its PT_LOAD
 * segments at their link addresses, and jumps to the entry point (_start) in
 * 32-bit protected mode. The Multiboot1 header must lie in the first 8 KiB of
 * the image, so .multiboot is the first output section; .text.boot (the arch
 * _start climb) follows. The conventional kernel load address is 1 MiB.
 *
 * The link is non-PIE static (the target default): absolute addresses are
 * baked at link time, which the 32-bit prologue relies on (it loads symbol
 * addresses as disp32 immediates before paging is on).
 *
 * Boot-core links the real kernel, so it has no arch_test! section — unlike the
 * selftest fixture's x86 script, there is no reovim_arch_tests bracket here.
 */
ENTRY(_start)

SECTIONS
{
    . = 0x100000;

    /* Multiboot1 header first, within the first 8 KiB the loader scans. */
    .multiboot : {
        KEEP(*(.multiboot))
    }

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

    /* 8-aligned bounds: the _start BSS clear stores in 4-byte steps over a
     * range the linker keeps 8-aligned. */
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
