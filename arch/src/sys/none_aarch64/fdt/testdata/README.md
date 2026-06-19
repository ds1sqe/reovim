# FDT reader test fixtures (compiled-in)

`reovim-bcm2711.dtb` is an **original** BCM2711 device tree authored for this
repository (`reovim-bcm2711.dts`, SPDX `AGPL-3.0-only`, © ds1sqe). It is
embedded into the FDT reader / enumerator unit tests with `include_bytes!`.

It is deliberately kept free of any third-party copyleft material. reovim is
licensed AGPL-3.0-only **with an owner exception** (the copyright holder retains
the right to relicense), so nothing that gets compiled into a reovim artifact
may carry a license the owner cannot relicense. A vendored kernel DTB is
GPL-2.0-only and not relicensable, so it must never be `include_bytes!`'d.

The numeric `reg` addresses, `compatible` strings, and cell counts in the `.dts`
are factual hardware constants of the BCM2711 SoC (not copyrightable
expression); the node selection and structure are original.

The **real** Raspberry Pi 4 firmware device tree (GPL-2.0) used for the on-target
QEMU run lives separately at `arch/tests/fixtures/dtb/` and is passed to QEMU via
`-dtb` as a pure runtime input — it is never compiled into any binary. See that
directory's `PROVENANCE.md`.

## Regenerating the blob

```sh
dtc -I dts -O dtb -o reovim-bcm2711.dtb reovim-bcm2711.dts
```

The compiled `.dtb` is committed so the build never needs `dtc`.
