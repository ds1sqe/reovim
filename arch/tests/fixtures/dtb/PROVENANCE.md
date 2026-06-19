# Vendored device-tree fixture — provenance and license

## File

- `bcm2711-rpi-4-b.dtb` — flattened device tree blob (FDT v17, magic `0xd00dfeed`)
  for the Broadcom BCM2711 SoC / Raspberry Pi 4 Model B.
- Size: 56407 bytes.
- SHA-256: `75761b73c284e26623e4d1624bff13e67bce2ae620880efd81d6571a3739fcfb`.

## Source

- Origin: Raspberry Pi firmware distribution, the canonical home of the compiled
  Pi 4 device-tree blobs.
- URL: `https://github.com/raspberrypi/firmware/raw/master/boot/bcm2711-rpi-4-b.dtb`
- Retrieved: 2026-06-19.
- The blob is compiled from the Linux / Raspberry Pi kernel device-tree sources;
  the canonical source is `arch/arm/boot/dts/broadcom/bcm2711-rpi-4-b.dts` and its
  `#include`d `.dtsi` files in the Linux kernel tree.

## License

- SPDX-License-Identifier: `GPL-2.0`.
- The source DTS (`bcm2711-rpi-4-b.dts` and every `.dtsi` it includes) carries the
  SPDX header `GPL-2.0`. The compiled blob inherits that license.
- Corresponding source: the Linux / Raspberry Pi kernel repositories
  (`torvalds/linux`, `raspberrypi/linux`), publicly available.

## Why this is license-clean (runtime input only)

reovim is licensed `AGPL-3.0-only` **with an owner exception**: the copyright
holder retains the right to relicense the work under other terms. That right
only holds if nothing the owner cannot relicense becomes part of the licensable
work. A GPL-2.0-only kernel DTB is **not** relicensable by the owner, so it is
confined to a role where it never enters any reovim artifact:

- This blob is used **only as a QEMU `-dtb` runtime input file** when booting the
  bare-metal `bootcore` fixture on the `raspi4b` machine. QEMU loads it from disk
  at run time and passes its address to the guest.
- It is **never `include_bytes!`'d, never compiled, and never linked into any
  reovim crate, binary, or test binary.** The FDT reader's compiled-in unit-test
  fixture is a separate, original, AGPL-licensed device tree at
  `arch/src/sys/none_aarch64/fdt/testdata/` — see that directory's `README.md`.

Passing an independent GPL-2.0 file to a program as runtime data is **mere
aggregation**, not a combined or derivative work. No combined work is created;
the blob stays GPL-2.0 and reovim's own source stays AGPL-3.0-only (owner
exception intact). This file records the attribution and corresponding-source
pointer that GPL-2.0 requires when the blob is redistributed.

## Regenerating

```sh
curl -fsSL -o bcm2711-rpi-4-b.dtb \
  https://github.com/raspberrypi/firmware/raw/master/boot/bcm2711-rpi-4-b.dtb
# Inspect:  dtc -I dtb -O dts bcm2711-rpi-4-b.dtb
```
