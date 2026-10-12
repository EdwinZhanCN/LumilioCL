"""Generate disposable vanilla worlds for independent saved-chunk verification.

Only Python's standard library is used. The original server jars must have
been prepared and hashed by `cargo xtask worldgen-prepare` first. All servers
bind to loopback, are offline, and operate under ignored target/worldgen.
"""
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import socket
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[4]
SEEDS = [262, 9876543210, -123456789012345]
# Bumped whenever a run records more; older completed worlds are regenerated.
FORMAT = "3"
LOCATED = re.compile(r"at \[(-?\d+), (?:~|-?\d+), (-?\d+)\]")
MISSING = re.compile(r"Could not find|There is no structure|There is no biome")
# Structures whose saved starts are read back: several are located from
# spread-out origins and the chunks within `around` of each start loaded.
STARTS = [
    ("the_nether", "minecraft:bastion_remnant", 0, [(0, 0), (1500, 0), (0, 1500), (-1500, 0), (0, -1500), (1500, 1500)]),
    ("overworld", "minecraft:igloo", 0, [(0, 0), (4000, 0), (0, 4000), (-4000, 0), (0, -4000)]),
    ("the_end", "minecraft:end_city", 0, [(1500, 0), (0, 1500), (-1500, 0), (0, -1500), (2000, 2000), (-2000, -2000)]),
    ("the_nether", "minecraft:nether_fossil", 0, [(0, 0), (400, 400), (-400, 400), (400, -400), (-400, -400), (800, 0)]),
    # Chests sit near the pyramid's middle, in the start chunk or the next.
    ("overworld", "minecraft:desert_pyramid", 1, [(0, 0), (3000, 0), (0, 3000), (-3000, 0), (0, -3000)]),
]
# Squares of fully generated chunks, read back block by block: around the
# origin, or around the nearest of a biome.
AREAS = [
    ("overworld", None, 8),
    ("overworld", "minecraft:desert", 8),
    ("overworld", "minecraft:swamp", 6),
    ("the_nether", "minecraft:soul_sand_valley", 6),
]


def collect(version, seed):
    source = ROOT / "target/worldgen" / version
    expected = json.loads((source / "version.json").read_text())["downloads"]["server"]["sha1"]
    if hashlib.sha1((source / "server.jar").read_bytes()).hexdigest() != expected:
        raise ValueError(f"original server hash mismatch: {version}")
    work = source / "saves" / str(seed)
    complete = work / ".complete"
    if complete.exists() and complete.read_text() == f"{expected}:{FORMAT}":
        print(f"{version} {seed}: reuse completed vanilla world", flush=True)
        return
    if work.exists():
        # An older or interrupted run: start from a fresh world.
        shutil.rmtree(work)
    legacy = version.startswith("1.18")
    # Releases older than the runtime probe still record saved structures.
    probe = source / "probe.json"
    oracle = json.loads(probe.read_text()) if probe.exists() else {"samples": []}
    samples = [r for r in oracle["samples"] if r["seed"] == seed]
    chosen = set()
    for dim in ["overworld", "nether", "end"]:
        rows = [r for r in samples if r["dimension"] == dim]
        # Runtime goldens already cover 96 positions per height. Saved-world
        # verification checks all dimensions/heights and adds new-biome points.
        # The committed saved goldens hold these four oracle positions.
        for index in [0, 23, 47, 71]:
            if index < len(rows):
                chosen.add((dim, rows[index]["x"], rows[index]["z"]))
    for biome in ["minecraft:pale_garden", "minecraft:sulfur_caves", "minecraft:dappled_forest"]:
        row = next((r for r in samples if r["biome"] == biome), None)
        if row:
            chosen.add((row["dimension"], row["x"], row["z"]))
    rows = [r for r in samples if (r["dimension"], r["x"], r["z"]) in chosen
            and (r["dimension"] == "overworld" or r["y"] >= 0)]
    work.mkdir(parents=True, exist_ok=True)
    (work / "coordinates.json").write_text(json.dumps(rows))
    # This is the original game's development test server, with no players.
    (work / "eula.txt").write_text("eula=true\n")
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    (work / "server.properties").write_text(
        f"server-ip=127.0.0.1\nserver-port={port}\nlevel-seed={seed}\n"
        "online-mode=false\nenforce-secure-profile=false\nwhite-list=true\n"
        "view-distance=2\nsimulation-distance=2\nspawn-protection=0\n"
        "sync-chunk-writes=true\nmax-tick-time=-1\n"
    )
    log = work / "server.log"
    java = os.environ.get("WORLDGEN_JAVA", "java")
    with log.open("w") as output:
        # The force-loaded areas and their neighbours stay in memory.
        process = subprocess.Popen([java, "-Xms256m", "-Xmx3G", "-jar", str(source / "server.jar"), "nogui"],
                                   cwd=work, stdin=subprocess.PIPE, stdout=output, stderr=output, text=True)
        def command(text):
            process.stdin.write(text + "\n")
            process.stdin.flush()
        def wait_for(predicate, seconds=180):
            deadline = time.monotonic() + seconds
            while time.monotonic() < deadline:
                text = log.read_text(errors="replace")
                if predicate(text):
                    return text
                if process.poll() is not None:
                    raise RuntimeError(f"server exited: {log}\n{text[-2000:]}")
                time.sleep(0.25)
            raise TimeoutError(f"server timeout: {log}")
        try:
            wait_for(lambda text: "Done (" in text)
            def said(text, name):
                # Only the server's own chat line: an error echoes the command.
                return re.search(r"\[Server\] " + re.escape(name) + r"$", text, re.MULTILINE)
            def loaded(dim, points, tag):
                """Wait until every block column (x, z) is in a loaded chunk.
                `unless block` fails on an unloaded position in every release
                (1.18 has no `execute if loaded`)."""
                pending = set(points)
                deadline = time.monotonic() + 600
                while pending:
                    if time.monotonic() > deadline:
                        raise TimeoutError(f"chunk timeout: {version} {seed} {dim} {sorted(pending)[:4]}")
                    for x, z in sorted(pending):
                        command(f"execute in minecraft:{dim} unless block {x} 0 {z} minecraft:structure_void run say {tag}_{x}_{z}")
                    time.sleep(1)
                    text = log.read_text(errors="replace")
                    pending = {(x, z) for x, z in pending if not said(text, f"{tag}_{x}_{z}")}
            for dim, x, z in sorted(chosen):
                command(f"execute in minecraft:{dim if dim == 'overworld' else 'the_' + dim} run forceload add {x*4} {z*4}")
            for dim in ["overworld", "nether", "end"]:
                points = [(x * 4, z * 4) for d, x, z in chosen if d == dim]
                loaded(dim if dim == "overworld" else "the_" + dim, points, "wg_ready")
            def locate(dim, target, x, z, biome=False):
                before = len(log.read_text(errors="replace"))
                if legacy:
                    # 1.18: `locate` takes structures, `locatebiome` biomes.
                    verb = "locatebiome" if biome else "locate"
                else:
                    verb = "locate biome" if biome else "locate structure"
                command(f"execute in minecraft:{dim} positioned {x} 64 {z} run {verb} {target}")
                text = wait_for(lambda text: LOCATED.search(text[before:]) or MISSING.search(text[before:]))
                match = LOCATED.search(text[before:])
                return (int(match[1]), int(match[2])) if match else None
            def load(dim, cx0, cz0, cx1, cz1, tag):
                """Force-load a chunk rectangle and wait until every chunk is."""
                for x0 in range(cx0, cx1 + 1, 16):
                    for z0 in range(cz0, cz1 + 1, 16):
                        x1, z1 = min(x0 + 15, cx1), min(z0 + 15, cz1)
                        command(f"execute in minecraft:{dim} run forceload add {x0 * 16} {z0 * 16} {x1 * 16} {z1 * 16}")
                loaded(dim, [(cx * 16, cz * 16) for cx in range(cx0, cx1 + 1) for cz in range(cz0, cz1 + 1)], tag)
            locates = []
            targets = [("overworld", "#minecraft:village", "Village"), ("overworld", "minecraft:stronghold", "Stronghold"),
                       ("the_nether", "minecraft:fortress", "Fortress"), ("the_end", "minecraft:end_city", "EndCity")]
            if version == "26.3":
                targets.append(("overworld", "#minecraft:abandoned_camp", "AbandonedCamp"))
            for dim, target, kind in targets:
                found = locate(dim, target, 0, 0)
                if found is None:
                    raise RuntimeError(f"{version} {seed}: no {target} near the origin; see {log}")
                x, z = found
                locates.append({"seed": seed, "dimension": dim, "kind": kind, "x": x, "z": z})
            (work / "locates.json").write_text(json.dumps(locates))
            # Structure starts live in the start chunk's NBT once it is loaded.
            starts = []
            for dim, target, around, origins in STARTS:
                found = {locate(dim, target, x, z) for x, z in origins} - {None}
                for x, z in sorted(found):
                    starts.append({"seed": seed, "dimension": dim, "structure": target, "x": x, "z": z})
                    cx, cz = x >> 4, z >> 4
                    load(dim, cx - around, cz - around, cx + around, cz + around, f"wg_start_{len(starts)}")
            (work / "starts.json").write_text(json.dumps(starts))
            areas = []
            for dim, biome, edge in AREAS:
                at = (0, 0) if biome is None else locate(dim, biome, 0, 0, biome=True)
                if at is None:
                    continue
                cx0, cz0 = (at[0] >> 4) - edge // 2, (at[1] >> 4) - edge // 2
                cx1, cz1 = cx0 + edge - 1, cz0 + edge - 1
                load(dim, cx0, cz0, cx1, cz1, f"wg_area_{len(areas)}")
                areas.append({"seed": seed, "dimension": dim, "biome": biome,
                              "cx0": cx0, "cz0": cz0, "cx1": cx1, "cz1": cz1})
            (work / "areas.json").write_text(json.dumps(areas))
            command("save-all flush")
            command("stop")
            # Saving the loaded areas takes a while.
            process.wait(timeout=600)
            if process.returncode:
                raise RuntimeError(f"server failed: {log}")
            complete.write_text(f"{expected}:{FORMAT}")
            print(f"{version} {seed}: saved {len(rows)} biome samples, {len(locates)} locates, "
                  f"{len(starts)} structure starts and {len(areas)} areas", flush=True)
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


def attempt(version, seed):
    """One world; a failure is reported and the other worlds still run."""
    try:
        collect(version, seed)
        return None
    except Exception as error:
        print(f"{version} {seed}: FAILED: {error}", flush=True)
        return f"{version} {seed}"


if __name__ == "__main__":
    jobs = [(version, seed) for version in sys.argv[1:] for seed in SEEDS]
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        failed = [f for f in pool.map(lambda job: attempt(*job), jobs) if f]
    if failed:
        sys.exit("failed worlds: " + ", ".join(failed))
