/* Bare-metal aarch64 image layout (Pi 4 / QEMU raspi4b).
 *
 * Firmware loads kernel8.img at 0x80000 and jumps to its first byte, so
 * .text.boot (the arch _start boot arm) must be the first output bytes.
 * The link is non-PIE static (the target default): absolute addresses are
 * baked at link time, which is the honest model for an image nobody
 * relocates — the arch_test! distributed-slice entries hold absolute
 * pointers that must be correct as loaded.
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

    /* arch_test! registrations. The testrt runner walks
     * [__start_reovim_arch_tests, __stop_reovim_arch_tests); a script-defined
     * output section does not get the linker's automatic bracket symbols, so
     * they are defined here explicitly. KEEP: nothing references the entries
     * by name, only the bracket walk, so GC must not drop them.
     */
    reovim_arch_tests : {
        __start_reovim_arch_tests = .;
        KEEP(*(reovim_arch_tests))
        __stop_reovim_arch_tests = .;
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
