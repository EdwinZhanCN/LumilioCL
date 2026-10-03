# Content updates behavior

- Only **enabled files** of the checked kind are considered; disabled files are
  the user's decision and folder packs have no single hash. Files that cannot be
  read are skipped.
- Two lookups by SHA-1: which Modrinth versions the files are (`identify`) and
  the newest compatible version for the instance's game version and loader
  (`latest_for`). No request is made when there is nothing to check.
- Classification: a file Modrinth does not know is *unknown*; a known file whose
  newest compatible version lists the file's own hash (or has no newer
  compatible version) is *up to date*; otherwise it is an *update*.
- Applying an update downloads the new file to the right folder, verified by the
  published size and SHA-1, and only then removes the old file. A failed
  download leaves the old file untouched. If the new version has the same file
  name it replaces the old one in place.
- POST requests are not retried against other sources.
