---
title: Content sources
description: Change between Modrinth and CurseForge on the Discover page, install CurseForge modpacks, and add files that you can download only on a website.
---

The Discover page finds and installs mods, resource packs, shaders, and modpacks from content sources.
The launcher includes two content sources: Modrinth and CurseForge.
Each content source is a plugin.
To turn a source on or off, select it in **Settings › Plugins**, and then use the **Turn on plugin** switch.

## Change the source

When the two sources are on, the top of the Discover filter bar shows two segment keys: **Modrinth** and **CurseForge**.
If the window is narrow, the filter bar is not on the page. Click the **Filters** key, to the left of **Sort by**, to open it.
Click a segment key.
The Discover page then searches again in that source:

- It keeps the search text, the content type, and the game version.
- It clears the loader, the categories, and the sort order. The filters change to the filters of the source.
- The next time that you open the Discover page, it uses the same source.

When only one source is on, the segment keys do not show.
The sort list shows only the sort orders that the current source supports.
For example, CurseForge cannot sort by followers.

## Install a CurseForge modpack

Use one of these procedures:

- On the Discover page, click **CurseForge**, and then select **Modpack**. In a result row, click **Install**. Or, on the details page, click **Install as a new game**.
- In **Library**, click **Import modpack**, and then select the `.zip` file of the CurseForge modpack. You can also drag the file onto the launcher window.

**Activity** shows the progress.
When the installation is complete, a message lets you open the new game.
If the modpack author turned off a file, the launcher installs the file in the off state.
To import a CurseForge modpack, the CurseForge source must be on.

## Files that you can download only on the website

On CurseForge, some authors let people download their files only on the CurseForge website.
The launcher does not go around this setting:

- A result row and a details page show **Website download only** for these projects.
- During installation, the launcher first looks for the same file on Modrinth by its SHA-1. If Modrinth has the file, the launcher downloads it from Modrinth.
- If Modrinth does not have the file of a single project, the launcher tells you that the author allows the download only on the website. It also gives **Open download page**.
- If Modrinth does not have a file of a modpack, the launcher installs the modpack without the file. The **Content** tab of the game lists the missing files.

### Add the missing files

1. Open the **Content** tab of the game. The missing files show below "N files are missing".
2. For each file, click **Download page**, and then download the file in your browser.
3. Click **Add downloaded files…**, and then select the downloaded files. You can select many files at one time.

The launcher examines the SHA-1 of each file and puts the file in the correct folder.
A file that is in the correct folder goes off the list.
For each file that the game does not need, the launcher shows a message.
You cannot add files while the game runs.

## Recognition and updates

The **Content** tab of the game asks the sources that are on to recognize the installed files.
Modrinth is first. CurseForge examines only the files that Modrinth does not recognize.
For a recognized file, the tab shows the project name, the author, and the version.
You can update the file or change its version.
The source that recognized the file finds its updates.
If the launcher cannot connect to a source, the tab shows the files and tells you about the problem.

On the Discover page, **Installed** and **Update** show only for files that the current source recognizes.

## Limits

- The CurseForge source needs an API key. The releases on GitHub include the key. If you build the launcher without the `CF_API_KEY` variable, the CurseForge source is off by default, and its plugin description gives the reason.
- The launcher does not send requests to the CurseForge API through download mirrors.
- **Import from another launcher…** in **Library** cannot read instance folders of the CurseForge app. Export a modpack from the CurseForge app, and then use **Import modpack**.
- The Discover page shows the results of one source at a time. It does not merge the results of the two sources.
