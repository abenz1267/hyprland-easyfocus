# hyprland-easyfocus

A hyprland window switcher inspired by [sway-easyfocus](https://github.com/edzdez/sway-easyfocus).

## Features

- focus windows with predefined labels
- handles fullscreen windows
- allows cycling before X amount of windows to save keystrokes

## Installation

```bash
// Arch
yay -S hyprland-easyfocus
```

## Config

`$XDG_CONFIG_HOME/hyprland-easyfocus/` or `$HOME/.config/hyprland-easyfocus/`.

Default `config.json`:

```json
{
  "labels": "asdfjkl;gh",
  "cycle_before": 3,
  "label_position": "Center",
  "box_size": 30,
  "ignore_current": true
}
```

Default `style.css`:

```css
window {
  font-family: monospace;
  background: rgba(0, 0, 0, 0);
}

window box {
  background: #c8c093;
}

window label {
  font-size: 20px;
  color: #1f1f28;
}

.current {
}
```

- `labels`: The labels to use for the windows.
- `cycle_before`: The number of windows needed to display labels, otherwise cycle forward.
- `label_position`: The position of the labels. Can be `TopCenter`, `BottomCenter`, `TopLeft`, `BottomLeft`, `TopRight`, `BottomRight`, or `Center`.
- `box_size`: The size of the boxes around the labels.
- `ignore_current`: If true, the current window will not get a label.

## Keybindings

The keybindings are based on the labels. Escape will close the window. Pressing a label will focus the window with that label.
