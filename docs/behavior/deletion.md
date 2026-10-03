# Instance deletion behavior

`service.delete_instance(id)` is a journaled operation so a failure or crash at
any step can be undone or finished. Spec: [L-LIB-06](../workflows/instance-lifecycle.md#l-lib-06--删除实例).

Folder: `<root>/state/operations/delete-<id>-<unix>/` holds `delete.json` (the
instance record and a schema number) and, once moved, `profile/`.

Order, under the instance lease:

1. Journal the record (atomic write). Failure: nothing changed.
2. Move `profiles/<id>` to `profile/` with a rename (same volume). Failure:
   journal discarded, record and files unchanged. A missing profile is fine.
3. Remove the record from the library. Failure: the profile is moved back and the
   journal removed; if even that fails the journal stays for recovery.
4. Remove `profile/` first, then the journal. Failure here does not undo the
   deletion (the library already committed); leftovers are retried at next start.

Deleting an unknown id reports `NoSuchInstance`. `meta/`, runtimes and other
instances are never touched. The rename assumes `profiles/` and `state/` share a
volume; if it fails the deletion is refused with the I/O error.

Not built: a deletion UI/confirmation, undo after cleanup, cross-volume moves.
