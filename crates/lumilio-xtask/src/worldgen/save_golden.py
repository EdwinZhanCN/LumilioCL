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
import socket
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[4]
SEEDS = [262, 9876543210, -123456789012345]


def collect(version, seed):
    source = ROOT / "target/worldgen" / version
    expected = json.loads((source / "version.json").read_text())["downloads"]["server"]["sha1"]
    if hashlib.sha1((source / "server.jar").read_bytes()).hexdigest() != expected:
        raise ValueError(f"original server hash mismatch: {version}")
    work = source / "saves" / str(seed)
    complete = work / ".complete"
    if complete.exists() and complete.read_text() == expected:
        print(f"{version} {seed}: reuse completed vanilla world", flush=True)
        return
    oracle = json.loads((source / "probe.json").read_text())
    samples = [r for r in oracle["samples"] if r["seed"] == seed]
    chosen = set()
    for dim in ["overworld", "nether", "end"]:
        rows = [r for r in samples if r["dimension"] == dim]
        # Runtime goldens already cover 96 positions per height. Saved-world
        # verification checks all dimensions/heights and adds new-biome points.
        for index in [0]:
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
        process = subprocess.Popen([java, "-Xms256m", "-Xmx1G", "-jar", str(source / "server.jar"), "nogui"],
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
            for dim, x, z in sorted(chosen):
                command(f"execute in minecraft:{dim if dim == 'overworld' else 'the_' + dim} run forceload add {x*4} {z*4}")
            for i, (dim, x, z) in enumerate(sorted(chosen)):
                dimension = dim if dim == "overworld" else "the_" + dim
                deadline = time.monotonic() + 180
                while f"wg_ready_{i}" not in log.read_text(errors="replace"):
                    if time.monotonic() > deadline:
                        raise TimeoutError(f"chunk timeout: {version} {seed} {dim} {x} {z}")
                    command(f"execute in minecraft:{dimension} if loaded {x*4} 64 {z*4} run say wg_ready_{i}")
                    time.sleep(0.5)
            locates = []
            targets = [("overworld", "#minecraft:village", "Village"), ("overworld", "minecraft:stronghold", "Stronghold"),
                       ("the_nether", "minecraft:fortress", "Fortress"), ("the_end", "minecraft:end_city", "EndCity")]
            if version == "26.3":
                targets.append(("overworld", "#minecraft:abandoned_camp", "AbandonedCamp"))
            for dim, target, kind in targets:
                before = len(log.read_text(errors="replace"))
                command(f"execute in minecraft:{dim} positioned 0 64 0 run locate structure {target}")
                text = wait_for(lambda text: re.search(r"at \[(-?\d+), (?:~|-?\d+), (-?\d+)\]", text[before:]) is not None)
                match = re.search(r"at \[(-?\d+), (?:~|-?\d+), (-?\d+)\]", text[before:])
                locates.append({"seed": seed, "dimension": dim, "kind": kind, "x": int(match[1]), "z": int(match[2])})
            (work / "locates.json").write_text(json.dumps(locates))
            command("save-all flush")
            command("stop")
            process.wait(timeout=90)
            if process.returncode:
                raise RuntimeError(f"server failed: {log}")
            complete.write_text(expected)
            print(f"{version} {seed}: saved {len(rows)} biome samples and {len(locates)} locates", flush=True)
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


if __name__ == "__main__":
    jobs = [(version, seed) for version in sys.argv[1:] for seed in SEEDS]
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        list(pool.map(lambda job: collect(*job), jobs))
