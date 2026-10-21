# Version jsons from other launchers

Version jsons written by PCL and HMCL, for the import test in
`src/import_game/tests.rs` (issue #58). This folder stands in for a
`.minecraft`: each `versions/<name>/<name>.json` is copied unchanged from a
public repository at the commit below, and the `LICENSE` beside it is that
repository's license file at the same commit. These files stay under their own
licenses, not this project's. Nothing here was edited by hand.

None of them has `inheritsFrom`, and most ids are display names: the launcher
merged the vanilla json and the loader profile into one file.

| Version folder | Written by | Expected | Source repository | Commit | Path there | License |
|---|---|---|---|---|---|---|
| `1.20.4-Fabric 0.16.10` | PCL (`clientVersion`) | 1.20.4, Fabric 0.16.10 | [jiangaoMartin/F21CA-Games3-Minecraft-Murder-Mystery](https://github.com/jiangaoMartin/F21CA-Games3-Minecraft-Murder-Mystery) | `d2482e192aa291053ebbacc1d546ed993420a4a9` | `1.20.4-Fabric 0.16.10/1.20.4-Fabric 0.16.10.json` | GPL-3.0 |
| `Bunker Server AOT` | PCL | 1.21.1, Fabric 0.18.4 | [alkalimc/Cell-Server-AOT](https://github.com/alkalimc/Cell-Server-AOT) | `ba76ec3d3110bb8a0293d78c8a5f8953d5f6bf30` | `.minecraft/versions/Bunker Server AOT/Bunker Server AOT.json` | Apache-2.0 (no NOTICE file) |
| `HereBeDragons` | PCL | 1.21.1, NeoForge 21.1.193 | [JesterRomut/HereBeDragons](https://github.com/JesterRomut/HereBeDragons) | `64a5d5946557d0e9eb2872f4feba2e2ab24b7156` | `HereBeDragons.json` | GPL-3.0 |
| `BACraft` | PCL | 1.20.1, Forge 47.4.0 | [chisaato/BACraft](https://github.com/chisaato/BACraft) (branch `1.20.1`) | `f306aafd991222c3ebc6e4545921b34f4d6a0a4e` | `.minecraft/versions/BACraft/BACraft.json` | MIT |
| `1.20.1-Forge` | HMCL (`patches`) | 1.20.1, Forge 47.3.11 | [takanashisatou/GTL-CEU](https://github.com/takanashisatou/GTL-CEU) | `2a937e9ed5f315d19496b15b256c24ca3237a664` | `.minecraft/versions/1.20.1-Forge/1.20.1-Forge.json` | CC0-1.0 |
| `WizardryCraft` | HMCL | 1.21.1, NeoForge 21.1.251 | [MidnightPigeon/WizardryCraft](https://github.com/MidnightPigeon/WizardryCraft) | `0d7b9c6d197fb6dd54c717e971ff436331c15067` | `versions/WizardryCraft/WizardryCraft.json` | MIT |

Copied 2026-10-21. GTL-CEU's earlier commits carried GPL-3.0; the commit
above carries CC0-1.0. The expected values were read from the files
themselves (`clientVersion` or the `game` patch, the loader library and the
`--fml.*` arguments), not from this launcher.
