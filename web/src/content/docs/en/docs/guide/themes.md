---
title: Themes
description: Select a built-in theme, or make a theme in a JSON file.
---

A theme sets the colours of the launcher.
It also sets the corner radius of fields and dialogs, and the interface fonts.
The launcher uses one theme for light appearance and one theme for dark appearance.

## Select a theme

1. Open **Settings › Appearance**.
2. In the **Light theme** row, click **Edit**.
3. In the dialog, select a theme.
4. In the **Dark theme** row, click **Edit**.
5. In the dialog, select a theme.

The launcher applies and saves your selection immediately.
When **Light or dark** is **System**, the launcher changes between the two themes with the system.
To use the built-in theme again, click **Restore defaults** in the dialog.

The launcher includes these themes:

| Theme | Appearance | Source |
| --- | --- | --- |
| Aluminium | Light | Launcher default |
| Night | Dark | Launcher default |
| Catppuccin Latte | Light | [Catppuccin](https://github.com/catppuccin/catppuccin), MIT License |
| Catppuccin Mocha | Dark | [Catppuccin](https://github.com/catppuccin/catppuccin), MIT License |
| Rosé Pine Dawn | Light | [Rosé Pine](https://github.com/rose-pine/rose-pine-theme), MIT License |
| Rosé Pine | Dark | [Rosé Pine](https://github.com/rose-pine/rose-pine-theme), MIT License |

## Add a local theme

1. In **Settings › Appearance › Local themes**, click the folder button.
   On macOS, the button is **Show in Finder**.
   On Windows, the button is **Show in File Explorer**.
   On Linux, the button is **Show in file manager**.
2. Put your `.json` theme file in the `themes` folder.
3. Click **Reload**.
4. Select the new theme in **Light theme** or **Dark theme**.

If the `themes` folder does not exist, the launcher makes it.
The default locations of the folder are:

| System | Location |
| --- | --- |
| macOS | `~/Library/Application Support/LumilioCL/themes` |
| Windows | `%APPDATA%\LumilioCL\themes` |
| Linux | `$XDG_DATA_HOME/lumilio/themes`, or `~/.local/share/lumilio/themes` if `XDG_DATA_HOME` is not set |

If you set the `LUMILIO_HOME` environment variable, the folder is `$LUMILIO_HOME/themes`.
The **Local themes** row shows the path that the launcher uses.

The launcher reads only the `.json` files at the top level of the folder.
The folder also contains the Home wallpaper picture.
Do not delete the wallpaper picture.
The launcher shows each file or theme that it cannot use below **Local themes**, with the reason.
An error in one theme does not stop the other themes in the same file.

## File format

A theme file is JSON.
Its structure follows the theme files of [Zed](https://zed.dev).
A file has one `themes` array, which can contain many themes.

This file defines one light theme.
It gives only some colours.
The built-in Aluminium theme supplies the other colours.

```json
{
  "themes": [
    {
      "name": "Paper",
      "appearance": "light",
      "source": "My own palette",
      "colors": {
        "page": "#f4f1ea",
        "panel": "#faf8f3",
        "ink": "#1f1d1a",
        "orange": "#2f6fdb",
        "orange_text": "#1f56b3"
      },
      "radius": 6,
      "radius_lg": 10,
      "fonts": { "sans": "Inter" }
    }
  ]
}
```

For complete examples, read the included [rose-pine.json](https://github.com/EdwinZhanCN/LumilioCL/blob/main/crates/lumilio-ui/themes/rose-pine.json) and [catppuccin.json](https://github.com/EdwinZhanCN/LumilioCL/blob/main/crates/lumilio-ui/themes/catppuccin.json) files.

### Theme fields

| Field | Necessary | Description |
| --- | --- | --- |
| `name` | Yes | The theme name in the theme list |
| `appearance` | Yes | `"light"` or `"dark"`. It sets the list for the theme and the built-in theme that supplies missing colours |
| `source` | No | The source and license of the colours. Use it when you adapt colours from a different theme |
| `colors` | No | The colours in the table below. The built-in theme of the same appearance supplies missing colours |
| `radius` | No | The corner radius of fields, lists, and menus, in pixels. The default is `4`. The keys and panels of the launcher keep their own corners |
| `radius_lg` | No | The corner radius of dialogs and notifications, in pixels. The default is `6` |
| `fonts` | No | Font family names: `sans` for the interface, `mono` for monospace text, and `cjk` for Chinese text |
| `window_background` | No | The window material behind the page: `"opaque"` (default), `"transparent"`, or `"blurred"`. You can see it only through colours with opacity |

### Colours

Write each colour as `#rrggbb`, or as `#rrggbbaa` for a colour with opacity.
The `#` is optional.
The colour names come from the default colours.
For example, `orange` is the accent colour, but its value can be any colour.

| Colour | Use |
| --- | --- |
| `page` | The page background |
| `panel` | Panels, list blocks, and fields |
| `hairline` | Rules and borders |
| `ink` | Text |
| `muted` | Captions and secondary values |
| `key_white` | White keys: secondary keys and segment keys |
| `key_black` | Black keys: the More key and secondary keys on pictures |
| `key_grey` | Grey keys: tabs that are not open, disabled keys, and plain tags |
| `on_white` | Text on white keys |
| `on_black` | Text on black keys |
| `orange` | Accent fill: the primary key, the open tab, and lit LEDs |
| `orange_text` | Accent colour for text |
| `on_orange` | Text on the accent fill |
| `danger` | The key that confirms a destructive action |
| `danger_text` | Text of destructive menu items and failure messages |
| `led_off` | LEDs that are off |
| `ok` | The LED that shows success |
| `display` | The background of display windows, for example the game cover window |
| `display_ink` | Lit segments and bars in a display window |
| `display_dim` | Segments that are not lit in a display window |
| `display_label` | Labels and units in a display window |

## Rules and limits

- The corner radius is between `0` and `16` pixels. The launcher uses `16` for a larger value.
- A font that you select in **Settings › Appearance** has priority over the theme font.
- If a theme font is not installed, the launcher uses its own font.
- The launcher ignores a local theme that has the same name and appearance as a known theme.
- The launcher ignores fields that it does not know.
- If a theme does not give all colours, **Settings › Appearance** shows the number of missing colours. An incorrect colour name also causes this message.
- `"blurred"` needs a system that can blur the window. On other systems, the window does not change.
- After you change a theme file, click **Reload**.
- If you remove the theme that is in use and reload, the launcher uses the built-in theme of the same appearance.
