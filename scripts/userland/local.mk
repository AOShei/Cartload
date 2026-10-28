# Packages built with Buildroot 2015.11.1's recipes from newer sources, the versions hakchi
# v1.0.4-126 shipped (build.sh unpacks them into /work/src).
E2FSPROGS_OVERRIDE_SRCDIR = /work/src/e2fsprogs-1.44.1
DROPBEAR_OVERRIDE_SRCDIR = /work/src/dropbear-2018.76
KEXEC_OVERRIDE_SRCDIR = /work/src/kexec-tools-2.0.15
RSYNC_OVERRIDE_SRCDIR = /work/src/rsync-3.1.3

# Linked like the shipped binaries (their NEEDED entries):
# - dropbear: dynamically. Buildroot passes --enable-static to every package, which dropbear
#   2018 reads as "link the program statically".
DROPBEAR_CONF_OPTS += --disable-static
# (rsync's recipe sets its options with "=", so its change is a patch: buildroot-patches.)
