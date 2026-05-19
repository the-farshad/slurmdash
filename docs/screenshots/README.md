# Screenshots

The main README's Tour section uses Unicode renderings of each view. They
read directly in any markdown viewer and stay accurate when the layout
changes.

For richer captures (with theme colors, real cluster data), drop PNGs into
this directory and reference them from the main README. The expected files
and their content:

| File                | Capture                                                                |
|---------------------|-------------------------------------------------------------------------|
| `dashboard.png`     | The default view: sparkline strip, resources/queue/ending-soon row, partition cards, job table. |
| `details.png`       | A running job opened with `Enter`, including the progress bar and the History section. |
| `details-pending.png` | A pending job opened with `Enter`, showing the explained `Reason`. |
| `log-viewer.png`    | `tail -F` running, with follow mode on and search hits highlighted. |
| `confirm-cancel.png`| The confirm modal for `scancel`, showing the exact remote command. |
| `assist.png`        | `Ctrl+K` assist modal mid-conversation with a numbered proposed command. |
| `web-dashboard.png` | Browser view of `slurmdash web` on a loopback port. |
| `recommend-cli.png` | `slurmdash recommend` output in a terminal. |

## Capture tips

- **Terminal:** any modern terminal (Ghostty, Alacritty, iTerm2, kitty,
  WezTerm, Windows Terminal) with a font that supports the block characters
  (`█ ░ ▁ ▂ ▃ ▄ ▅ ▆ ▇`) and at least 200×40 cells.
- **Theme:** the default `dark` theme matches the colors documented in the
  README; light or `high-contrast` are fine but call them out in the
  filename (e.g. `dashboard-light.png`).
- **Tools:**
  - macOS: `Cmd+Shift+5` (window picker) or `screencapture -i out.png`.
  - Linux: GNOME Screenshot, Spectacle (KDE), `grim` (Wayland), `maim` (X11),
    or `flameshot`.
  - Cross-platform recordings: [`vhs`](https://github.com/charmbracelet/vhs)
    (declarative scripted captures) or [`asciinema`](https://asciinema.org/)
    (terminal cast files).
- **Browser screenshots:** the embedded UI is dark-themed by default; let
  the page render a refresh cycle before capturing so the panels are
  populated.

## License

Screenshots committed to this directory are released under the same
GPL-3.0 license as the rest of the project unless noted otherwise.
