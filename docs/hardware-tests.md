# Hardware tests

Manual checklists for steps that need a real console. Automated tests never touch hardware;
anything that does lives here instead.

Checklists land per crate as each one gains hardware-facing functionality:

- `crates/fel`: FEL memboot (console in FEL mode, `1f3a:efe8`).
- `crates/console`: discovery, SSH and FTP against a booted hakchi kernel (`04e8:6863`).
- `crates/hmod`: kernel package install and version check on the console.
- `crates/library`: sync of a small library and verification on the console menu.

Each checklist states the starting console state, the exact command to run, and the expected
output, so that a run can be reproduced and compared.
