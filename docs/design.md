# Design brief

Cartload should feel like a well-made hardware accessory: calm, precise, a little playful.
It is the opposite of a tool window full of buttons. People use it a few times a year, so it
has to explain itself every time.

## Identity

- Name: Cartload. Motif: the cartridge. Wordmark in a geometric sans; the cartridge glyph is the
  icon and the loading indicator.
- Palette: a neutral warm grey scale for surfaces, one accent taken from the 16-bit era (a
  saturated purple-blue, like the SNES logo's) used only for the primary action and focus,
  plus semantic green, amber, red. Both light and dark themes are first-class.
- Typography: system UI font for text; a tabular numeric variant for sizes and timings.

## Principles

1. One primary action per screen, in the accent colour. Everything else is quiet.
2. The console's state is always visible (status bar) and always explained in one plain sentence
   with the next action.
3. Progress is shown in place, never in modal dialogs. Long tasks show steps, timing and a log.
   Editors are the one exception: a long text or a rarely used group of fields may open in a
   dialog (`Dialog.svelte`) with Save and Cancel, so the panel it belongs to keeps its shape.
4. Errors name the likely cause and what to do, in words a non-engineer understands. The raw
   detail is available under a disclosure.
5. Nothing destructive without a way back: undo where possible, an explicit confirmation
   otherwise, worded as the action ("Remove 3 games") not "OK".
6. Keyboard and screen reader friendly. Visible focus. Labels on every icon button.
7. Dense where it helps (the library grid), spacious where it guides (kernel install).
8. No external assets at runtime except game art and the kernel package. Icons are inline SVG.

## Pages

- Console: status card, details, live log.
- Kernel: guided install with illustrated FEL step, step list, result.
- Library: folder tree, cover grid, detail panel, sync bar.
- Settings (small): theme, library location, TheGamesDB key, advanced toggles.
