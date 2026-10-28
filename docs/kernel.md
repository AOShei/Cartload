# Cartload's hakchi kernel (experimental)

The kernel Cartload installs today is the one in hakchi v1.0.4-126 (`data/hakchi.hmod`):
`3.4.113.29-madmonkey`, built 2020-03-20. Its exact source was never published; the public
kernel tree stops in May 2019 at `.24`. This is a rebuild from that public tree, brought level
with the shipped kernel, so that the kernel Cartload installs can come with its full source. It
is not used by the app yet.

## Source

`AOShei/sun-nontendocm-kernel`, branch `cartload` (on top of the public `f8c6e0b`):

- `sun_nontendocm_cartload_defconfig`: the configuration of the shipped kernel, read from its
  package's `modules.order` and `modules.builtin`. The published `sun_nontendocm_defconfig` is
  not what shipped: it builds about 80 drivers in that the shipped kernel has as modules or
  leaves out, and embeds its config, which the shipped kernel does not.
- The sunxi NAND driver as a module, as shipped: hakchi's boot ramdisk loads `nand.ko` itself so
  the console can boot from memory without touching the NAND.
- Three Realtek USB Wi-Fi drivers the shipped kernel has and the public tree lacks, at the vendor
  versions the shipped modules report: rtl8812au `v4.3.20_16317.20160108`, rtl8192eu
  `v4.4.1_17696.20160509_BTCOEX20160412-0042`, rtl8188fu `v4.3.23.6_20964.20170110`. The commit
  message names the public repository and commit of each.
- Version `3.4.113.30-cartload`, so it only ever loads its own modules.

Mali (r4p0-00rel0, `modules/mali`) and the controller driver (`clovercon/`) are built outside
`make modules`, as for the shipped package's `extra/` modules.

How close it is, by the shipped package's lists: the same 179 built-in drivers and the same 116
modules, plus two that the tree builds unconditionally (`sunxi-knob`, `crc-t10dif`). zImage
1,973,376 bytes against 1,968,736. Known differences:

- rtl8188fu has no sunxi platform hooks (the public source dropped them); it works as a plain
  USB driver. The shipped one switches the USB host controller on itself.
- rtl8192eu uses the sunxi platform file of rtl8188eu and rtl8812au; its own targets a later
  Allwinner board API this kernel does not have.
- clovercon has the same parameters as the shipped module but a different `srcversion`: the
  shipped one was built from a later, unpublished revision of Cluster's driver.
- The compiler is Ubuntu's GCC 4.9.3, the shipped kernel used Buildroot 2015.11.1's GCC 4.9.3.

Like every Allwinner kernel, the tree carries two binary blobs: the ARISC power-management
firmware (`drivers/arisc/binary/arisc_sun8iw5p1.code`) and, unused by this build,
`modules/nand/sun8iw5p1/libnand_sun8iw5p1`. The NAND driver this build uses
(`drivers/block/nand`) is source.

## Building

```sh
scripts/kernel/build.sh          # into target/kernel/out
scripts/kernel/build.sh --clean  # from scratch
```

It needs podman or docker and fetches the Ubuntu 16.04 image and the two pinned repositories on
the first run (about 25 s afterwards on 24 cores). The build is reproducible: dates, user and
host in the kernel, the vendor drivers' `__DATE__`, Mali's build date and the archive times are
all fixed to the kernel commit, so two builds of the same commits give the same bytes.

Output: the kernel (`zImage`, `System.map`, `.config`), a `boot.img` and a test package
`hakchi-3.4.113.30-cartload-test.hmod`. The boot image is the pinned package's, with the kernel
replaced and a second ramdisk archive appended that puts this kernel's modules in
`/lib/modules-ramfs`; hakchi links `/lib/modules/$(uname -r)` there when the NAND has no modules
for the running kernel. The script first checks that repacking the package's own kernel and
ramdisk reproduces its `boot.img`.

## Testing

**The test package is for memboot only.** Its `boot.img` is 6.8 MB, the boot partition takes 4 MB,
and Cartload has no size check of its own yet. Never install it.

```sh
cargo run -p cartload-flow -- memboot --hmod target/kernel/out/hakchi-3.4.113.30-cartload-test.hmod
```

boots it from RAM into hakchi's shell (from a running console through kexec, otherwise through
FEL). Return with `sync; umount -ar; reboot -f` (plain `reboot` does nothing in that shell) or by
power-cycling. Checklists and results: `docs/hardware-tests.md`, section "Cartload kernel".

Still to do before the app can offer it: boot to the menu through FEL, then test video, audio,
controllers, USB storage and Wi-Fi; a boot image that fits 4 MB (modules in the rootfs, not the
ramdisk); the install and uninstall paths; the NES and Famicom Minis.
