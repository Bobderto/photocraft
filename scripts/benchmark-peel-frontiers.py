"""Run both release revisions on the same runner and save the raw samples."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import statistics
import subprocess
import tempfile

BASE = "c66a95dee22ee5838bc2b1b6bb92397b45b8a001"
HEAD = "1383fce1c75768727e4983cfcf98b7fb24780b78"
REPS = 9
WORKERS = 2


def run(args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)


def build(checkout, env):
    run(
        ["cargo", "build", "--locked", "--release", "-p", "photocraft-algo",
         "--example", "bench_remove_targets"],
        cwd=checkout,
        env=env,
    )


def measure(binary, size, env):
    result = run(
        ["/usr/bin/time", "-l", binary, "--mode", "e2e", "--mp", "24",
         "--hole", "large", "--large", size, "--reps", "1"],
        env=env,
        text=True,
        capture_output=True,
    )
    sample = json.loads(result.stdout.splitlines()[0])
    rss = re.search(r"(\d+)\s+maximum resident set size", result.stderr)
    cpu = re.search(r"([\d.]+) user\s+([\d.]+) sys", result.stderr)
    if rss is None or cpu is None:
        raise RuntimeError("no CPU or resident memory report from /usr/bin/time -l")
    return {
        "ms": sample["ms"],
        "hash": sample["hash"],
        "peak_rss_bytes": int(rss.group(1)),
        "cpu_ms": (float(cpu.group(1)) + float(cpu.group(2))) * 1000.0,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", type=Path, required=True)
    options = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    os.chdir(root)
    env = os.environ.copy()
    env.setdefault("CARGO_TARGET_DIR", str(root / "target"))
    env["RAYON_NUM_THREADS"] = str(WORKERS)
    target = Path(env["CARGO_TARGET_DIR"]).resolve()
    executable = target / "release/examples/bench_remove_targets"

    for revision, repo in ((HEAD, "Bobderto/photocraft"), (BASE, "storytold/photocraft")):
        existing = subprocess.run(["git", "cat-file", "-e", revision], stderr=subprocess.DEVNULL)
        if existing.returncode != 0:
            run(["git", "fetch", "--depth=1", f"https://github.com/{repo}.git", revision])
    run(["git", "diff", "--exit-code", HEAD, "HEAD", "--", "crates/algo/src", "Cargo.toml", "Cargo.lock"])

    with tempfile.TemporaryDirectory(prefix="peel-compare-") as temp:
        temp = Path(temp)
        base = temp / "base"
        run(["git", "worktree", "add", "--detach", base, BASE])
        try:
            build(base, env)
            reference = temp / "baseline"
            shutil.copy2(executable, reference)
            build(root, env)

            phase_exe = temp / "discovery"
            run(["rustc", "--edition=2024", "-O", "-C", "lto=thin", "-C", "codegen-units=1",
                 "scripts/benchmark-peel-discovery.rs", "-o", phase_exe])
            discovery = run([phase_exe], capture_output=True, text=True).stdout
            print(discovery, end="", flush=True)

            samples = {}
            rows = []
            for size in ("128x96", "768x512"):
                results = {"baseline": [], "frontier": []}
                for binary in (reference, executable):
                    measure(binary, size, env)
                for rep in range(REPS):
                    order = [("baseline", reference), ("frontier", executable)]
                    if rep % 2:
                        order.reverse()
                    for name, binary in order:
                        sample = measure(binary, size, env)
                        print(json.dumps({"size": size, "variant": name, **sample}), flush=True)
                        results[name].append(sample)
                hashes = {row["hash"] for variant in results.values() for row in variant}
                if len(hashes) != 1:
                    raise RuntimeError(f"output hashes differ for {size}: {sorted(hashes)}")
                samples[size] = results
                for name, variant in results.items():
                    times = [sample["ms"] for sample in variant]
                    rss = statistics.median(sample["peak_rss_bytes"] for sample in variant)
                    rows.append({"name": f"{size} {name}", "samples_ms": times, "peak_rss_bytes": rss})

            options.json.write_text(json.dumps({
                "base": BASE, "head": HEAD, "workers": WORKERS, "discovery": discovery.splitlines(),
                "samples": samples, "rows": rows,
            }, indent=2) + "\n")
        finally:
            run(["git", "worktree", "remove", base])


if __name__ == "__main__":
    main()
