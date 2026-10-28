# Screenshots

How to capture the app window for the README, bug reports and design reviews. Capture both
themes (Settings → Appearance) at the default window size, 1024 × 700, with the window on a
plain background. Store the files in `docs/screenshots/` as `<page>-<theme>.png`, half scale
when they go into the repository.

## Which tool

Run `command -v grim gnome-screenshot spectacle import` and use the first one that is
installed. The commands below capture the Cartload window after a short delay, so the app can
be brought to the front first.

### Wayland

The window itself cannot be captured by X11 tools when the app runs natively on Wayland;
every compositor has its own way.

- **wlroots compositors (Sway, Hyprland, river):** `grim`, with `slurp` to pick the window.

  ```sh
  grim -g "$(slurp -o)" docs/screenshots/library-light.png
  ```

  On Hyprland, `hyprshot -m window -o docs/screenshots` picks the window by itself.

- **GNOME:** `gnome-screenshot` (package `gnome-screenshot`; the built-in Print-key tool
  cannot be scripted). `-w` captures the focused window, `-d` waits.

  ```sh
  gnome-screenshot -w -d 3 -f docs/screenshots/library-light.png
  ```

  Without it, press Print, choose *Window*, and copy the file from `~/Pictures/Screenshots`.
  The GNOME Shell screenshot D-Bus interface refuses third-party callers since GNOME 41, so a
  `gdbus` call is not an option.

- **KDE Plasma:** `spectacle` from the `spectacle` package.

  ```sh
  spectacle -a -b -n -d 3000 -o docs/screenshots/library-light.png
  ```

  (`-a` active window, `-b` background mode, `-n` no notification, `-d` delay in ms.)

- **Any compositor, through XWayland:** start the app with `GDK_BACKEND=x11`; it is then an
  X11 window and ImageMagick's `import` (package `imagemagick`) can capture it by name.

  ```sh
  GDK_BACKEND=x11 pnpm tauri dev &
  sleep 8
  import -window "$(xdotool search --name '^Cartload$' | head -1)" docs/screenshots/library-light.png
  # without xdotool: import -window Cartload docs/screenshots/library-light.png
  ```

### X11

- `gnome-screenshot -w -d 3 -f out.png`, `spectacle -a -b -n -d 3000 -o out.png` or
  `scrot -u -d 3 out.png` (focused window, 3 s delay).
- ImageMagick: `import -window Cartload out.png` (any window titled Cartload) or
  `import -window root out.png` for the whole screen.
- `xdotool search --name '^Cartload$'` gives the window id for `import -window <id>`.

### macOS and Windows

- macOS: `screencapture -w -W -T 3 out.png` prompts for the window after 3 s, or Cmd+Shift+4,
  then Space, then click the window.
- Windows: Alt+Print copies the active window to the clipboard; the Snipping Tool
  (`snippingtool /clip`) or PowerToys do the same with a file.

## Without a display

The pages render without a backend from the Vite preview fixtures, which is how the design
pass is checked in CI-like conditions. Headless Chrome captures them:

```sh
pnpm dev --port 1420 &
google-chrome --headless=new --window-size=1280,800 --hide-scrollbars \
  --screenshot=docs/screenshots/library-light.png \
  "http://localhost:1420/?page=library&library=many&sync=plan&theme=light"
```

Fixtures: `?preview=disconnected|fel|booting|connected|stock` (Console), `&flow=
idle|running|waiting-fel|removing|failed|done|cancelled&package=cached|none|problem` with
`&window=open` for the progress window, `&modules=cached|installed` (Emulators),
`&remove=<module>` (the removal dialog), `&emulator=<module>` (Library filter), `?page=library&library=some|
empty|many&sync=none|plan|review|warning|running|done|failed&import=running|done`, `?page=settings`. `&theme=light|dark`
forces a theme, `&log=open|closed` the activity log panel; on the Library page `&folder=4/1`
opens a folder of the fixture (index path), `&select=N` selects its first N games (the detail
panel) and `&dialog=description|advanced|changes` opens one of the editors. These are the
preview fixtures, not the WebKitGTK window: fonts and form controls can differ slightly from
the desktop app. Vite can keep serving a component's old CSS after an edit: restart `pnpm dev`
before capturing.

## What to capture

| File | State |
| --- | --- |
| `console-connected-<theme>.png` | Console page with a hakchi console identified |
| `console-nothing-<theme>.png` | Console page with nothing attached |
| `kernel-running-<theme>.png` | Progress window during the upload step (`?flow=running&window=open`) |
| `kernel-done-<theme>.png` | Progress window after a successful install (`?flow=done&window=open`) |
| `library-<theme>.png` | Library page with folders, covers, a selected game and the sync bar |
| `library-plan-<theme>.png` | Library page with the sync plan open |
| `settings-<theme>.png` | Settings page, scrolled to the About panel |
