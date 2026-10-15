---
title: Schematic studio
description: Examine the 3D model and the materials of Litematica schematics, inspect blocks, slice and select, replace blocks, convert versions, edit info, find duplicate files, and restore from backups.
---

The schematic studio opens the `.litematic` files in the `schematics` folder of a game.
You can examine the 3D model and the materials, and you can change the files.
Before each change, the launcher makes a backup of the original file.

## Scope

- The studio reads and writes only Litematica `.litematic` files, and only in the `schematics` folder of the game and its subfolders.
- It supports the blocks and entities of release versions 1.13.2 and later.
- The studio comes from the **Litematica schematics** plugin. The plugin is on by default. To turn it off, go to **Settings › Plugins**.

## Preparation

1. Install the Litematica mod in the game, or put `.litematic` files in the `schematics` folder of the game.
2. On the game page, open the **Plugins** tab, and then select **Schematics**.

If the `schematics` folder is empty and Litematica is not installed, **Schematics** does not show.

The game page shows a catalogue of the schematics: one card for each file, with a thumbnail.
Select a card, and then click **Open studio** (or double-click the card).
The studio opens the schematic in its own window.

### Block assets

The preview needs the block textures, the block models, and the entity shapes of the game.
They come from a bundle of block assets that the launcher downloads from launcher.lumilio.org.
The bundle has every release version from 1.13.2 to the latest.

- The launcher downloads the bundle one time, examines its checksum, and keeps it in the launcher data folder.
- **Activity** shows the download as "Prepare block assets for …". The progress shows in bytes, and you can cancel the download.
- If there is no network connection and the bundle is not downloaded, the viewport shows a message. If the server answers but does not have the bundle yet, the message tells you that your network is not the problem.

To stop the automatic download, turn off **Settings › Downloads & storage › Download block assets automatically**.
The launcher then downloads nothing. A bundle that is already downloaded stays in use.

### Mods and resource packs

The mods of the game, its resource packs that are on, and the resource packs that you add show in **Info › Technical details › Resource sources**.
Each row has a switch:

- The switches are off by default. When you turn one on, the viewport draws the model again with that source on top. The priority is: added resource packs, then resource packs, then mods, then the base version.
- The launcher keeps the switches for each game and each schematic folder. They stay when you open the window again.
- Each row shows how many kinds of blocks the source has, and how many models use a custom loader and cannot be drawn.

To add a resource pack that is not in the game, click **Add resource pack…** in **Resource sources**, and then select a `.zip` file.
The launcher keeps only the path of the file. It does not copy the file.
To take the pack off again, click **Remove** in its row. The file itself does not change.

## The studio window

- **Head**: the file switcher (a thumbnail and the file name; click it to get a list of the files that you can search). Then the size and the versions, for example "1.20.1 schematic · game 26.3". If the two versions are the same, the head shows one version. Then the orange key **Replace blocks…**, and the **More** menu with **Convert version…**, **Export PNG…**, **Export materials (CSV)**, **Edit info…**, **Show in Finder** (the name changes with the operating system), and **Delete…**.
- **Viewport**: the 3D model. A tool strip is at the bottom. Above the strip, a plate shows the block under the pointer.
- **Materials panel**: a search field and the block counts at the top, then three tabs: **Materials**, **Info**, and **History**.

While the game runs, you cannot change a schematic. The menu items and keys for changes are disabled, and their tooltips tell you why.

## Examine a schematic

### Viewport

- **Orbital**: drag to turn the model. Use the mouse wheel to zoom. The arrow keys turn the model, and `+` / `−` zoom.
- **Explore**: click the picture or push Enter to capture the mouse. Use WASD to move, Space to go up, and Shift to go down. Push Esc to release the mouse. While the mouse is captured, a crosshair shows at the center of the picture.
- The **Angle** list has six presets: North, East, South, West, Top, and Isometric. If you select a preset in Explore, the viewport changes to Orbital first.
- **Reset view** goes back to the first view.
- If this version cannot draw a block, the viewport shows it as a full block with the magenta and black missing texture. The shape stays complete.

### Inspect one block

In Orbital, click a block without a drag. In Explore, while the mouse is captured, click the block under the crosshair.
An outline shows around the block, and an inspect card shows above it. The card moves with the view.

- The card shows the name, the block ID, and the position. Each property shows as a tag. Hover over a tag to see the property as `key=value`.
- For a container, the card shows the slots as the game screen does, with each item and its count. Hover over a slot to see the name and the stacks. For a sign or a command block, the card shows the text.
- **Select this kind** selects this kind of block in **Materials**. **Replace…** opens Replace blocks with the state of this block.

To release the block, click empty space, push Esc, or click the close key of the card.

### Materials

The **Materials** tab shows one tile for each kind of block: an icon and the count. The kinds with the most blocks come first.
Hover over a tile to see the name and the stacks, for example "1 stack + 22".

- When you hover over a tile, the viewport keeps that kind in color and fades the other blocks.
- Click a tile to select that kind. A detail shows at the bottom of the panel: the name, the ID, the count, the count of each block state, and the keys **Replace…** and **Show only these**.
- If the game version does not have some of the blocks, the head shows an orange tag, for example "3 block kinds 26.3 does not have", and the key **Convert to 26.3…**. The tiles of those blocks show ✕. Turn on **Only incompatible** to show only those tiles.

### Slice

Click **Slice** in the tool strip. The viewport then draws one layer and the layers below it. The cut shows the top faces of the layer.

- Use **X**, **Y**, or **Z** to change the axis. Use the ‹ and › keys, or push `[` and `]` in the viewport, to go down or up one layer.
- The read-out shows the axis, the layer number, and the number of blocks in the layer. **Materials** counts only this layer.
- Click **Slice** again to turn it off.

### Select a box

In Orbital, hold Ctrl (⌘ on macOS) and drag from one block to another block.
The box between the two blocks is selected. A line shows along its edges, and the cells in it get a light color.

- **Materials** shows a line such as "Selection 4×3×6 · 52 blocks" and counts only the blocks in the box.
- To go back to the full schematic, click **Clear selection**, or push Esc in the viewport.
- If you open **Replace blocks…** while a box is selected, the scope is the box.

### Info

The **Info** tab shows the name, the author, the description, and the times. Click **Edit…** to change them.
The tab also shows the entities, the duplicate files, and **Technical details** (the data version, the resource sources, and more).

### Export a PNG

1. In **More**, select **Export PNG…**.
2. Enter the width and the height, from 1 to 4096 pixels each.
3. Select the background: **Panel color** or **Transparent**.
4. Select the angle: **Current view** or one of the six presets.
5. Click **Export**, and then select a location in the save dialog.

### Export the materials

In **More**, select **Export materials (CSV)**, and then select a location in the save dialog.
The list starts with the kinds that have the most blocks. It has a column for the stacks.

## Change a schematic

Each change follows the same steps:

1. The launcher copies the original file to its own backup folder. This folder is in the launcher data folder, not in the game folder.
2. The launcher makes sure that no other program changed the file after the studio read it. If the file changed, the launcher writes nothing, and the dialog tells you.
3. The launcher writes the new file.
4. **History** records the change.

### Replace blocks

Replace blocks is a table. Each row goes from a block to a block.

1. Open **Replace blocks…**:
   - From the orange key in the head. The first row is the kind with the most blocks.
   - From **Replace…** in the detail of a tile. The row matches each state of that kind.
   - From **Replace…** on the inspect card. The row matches the state of that block, each property exactly.
2. On the left, select the block to replace. When **Any properties** is on, the row matches each state of the block. When it is off, select a value or **Any** for each property.
3. On the right, select the block to use instead. The list has the blocks of the game version of the schematic. The blocks that you used last come first. You can search the list.
4. To replace more kinds at the same time, click **Add a row**. All rows match the blocks as they were before the replacement. One row does not replace the result of another row.
5. In **Scope**, select **All regions**, one region, **Current slice**, or the selection.
6. Each row shows how many blocks it replaces. The commit key shows the total, for example "Replace 128 blocks". Click it to write the file.

The new block keeps the properties that have the same name in the old block, for example the facing of stairs.
A block that keeps its block ID keeps its block entity data.

To use the table again later, enter a name in **Schemes**, and then click **Save as scheme**.
The launcher keeps the scheme in its data, for all games. In another schematic, select the scheme in **Apply a scheme…**.
When you replace blocks with a scheme, **History** shows the name of the scheme.

### Convert a version

1. In **More**, select **Convert version…**, or click **Convert to 26.3…** in the head. The target is the version of the game by default.
2. The dialog tries the conversion first:
   - A conversion to a newer version loses nothing.
   - For a conversion to an older version, the dialog lists each change in three groups: **Lost**, **Approximated**, and **Kept**. **Kept** has the blocks that the target version does not have. The launcher writes them as they are, and they become air in that version.
   - Under **Kept**, click **Choose replacements for these blocks…**. Replace blocks opens with one row for each kept block. Select the block to use instead for each row.
3. By default, the launcher saves the result as "name (26.3).litematic" in the same folder. The original file does not change, and the studio selects the new file. To write over the original file, turn on **Replace the original file**.
4. If you click **Cancel**, the launcher writes nothing.

### Edit the info

1. On the **Info** tab, click **Edit…**, or select **Edit info…** in **More**.
2. Change the name, the author, or the description, and then save.

The name cannot be empty. When you save, the launcher updates the modification time. It does not change the blocks.

### Duplicate schematics

The **Duplicates** section of **Info** lists the files that have the same blocks as this schematic.
The launcher compares only the blocks. It does not compare file names or metadata.

To keep only one file, click **Keep only this** in the row of the file to keep, and then confirm.
The launcher moves the other files of the group to its backups. It does not delete them. The launcher never deletes duplicate files automatically.

### Delete

In **More**, select **Delete…**, and then confirm.
The launcher moves the file to its backups. It does not delete the file. You can restore it from **History**.

### Restore from history

1. Open the **History** tab. Each row is one change: the time, the operation, the file, and a summary.
2. In the row of the change to undo, click **Restore**, and then confirm.

The launcher first makes a backup of the current file. Then it puts back the backup from before the change.
**History** records the restore too, so you can also restore a restore.
If the change wrote a new file (for example, a conversion saved as a new file), the restore moves that file to the backups. It does not delete the file.

## Known limitations

- The studio supports only Litematica `.litematic` files.
- The studio cannot place or delete single blocks. It can only replace blocks by block state and edit the info.
- A click selects a full cell, not the real shape of the block. With blocks that are not full blocks, and with translucent blocks, a click can select the cell next to the block. You cannot select entities.
- Entities show as the game shows them at that moment, before an animation starts. Evoker fangs and fishing bobbers do not show (the game does not show them when it loads a save). The glow around shulker bullets, the flash and swell of primed TNT, and the beam of end crystals do not show.
- A replacement keeps only the properties that the block assets name. Properties that no model uses, such as `waterlogged`, get their default value.
- In a conversion to an older version, the launcher writes properties that the older version does not know as they are. That version uses the default of the block for them. If the launcher cannot get the block list of the target version, the dialog tells you that it did not examine these blocks.
- For versions before 1.17, entities and block entities use the shapes of 1.17. The textures stay the textures of their own version.
- Animated textures, shaders, and custom entity models (CEM) of resource packs are not supported.
- In version 26.2 and later, the text on signs does not show.
- If stained glass of different colors is next to each other, the faces between them do not show.
- Translucent entity parts are sorted only by entity, from far to near. At some angles, they can overlap incorrectly.
- On Wayland, you cannot capture the mouse. Use Orbital.
- The launcher does not remove backups automatically.
