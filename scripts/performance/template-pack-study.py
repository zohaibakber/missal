"""Compare template storage formats in temporary files, without opening Missal user data.

This is a Python format study using repeated copies of the current document corpus.
It does not benchmark Effect, Drizzle, the real app schema, or Windows cold storage.
"""

import argparse
import gzip
import hashlib
import json
import platform
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time
from pathlib import Path


def peak_rss_kib():
    status = Path("/proc/self/status")
    if status.exists():
        for line in status.read_text().splitlines():
            if line.startswith("VmHWM:"):
                return int(line.split()[1])
    return None


def run_worker(args):
    folder = Path(args.fixture)
    destination = folder / f"{args.worker}.sqlite"
    initial_rss = peak_rss_kib()
    started = time.perf_counter()
    digest = None
    if args.worker in ("copy-seed", "inflate-seed"):
        digest = hashlib.sha256()
        source = (
            (folder / "seed.sqlite").open("rb")
            if args.worker == "copy-seed"
            else gzip.open(folder / "seed.sqlite.gz", "rb")
        )
        with source, destination.open("wb") as output:
            while block := source.read(1024 * 1024):
                output.write(block)
                digest.update(block)
    else:
        database = sqlite3.connect(destination)
        database.execute("CREATE TABLE templates (id INTEGER PRIMARY KEY, document TEXT NOT NULL)")
        retained = []
        if args.worker == "eager-json":
            retained = [json.loads((folder / f"{index}.json").read_bytes()) for index in range(args.count)]
        batch_size = args.count if retained else args.batch_size
        for start in range(0, args.count, batch_size):
            batch = []
            for index in range(start, min(args.count, start + batch_size)):
                if retained:
                    document = retained[index]
                elif args.worker == "bounded-gzip-json":
                    with gzip.open(folder / f"{index}.json.gz", "rb") as source:
                        document = json.load(source)
                else:
                    document = json.loads((folder / f"{index}.json").read_bytes())
                batch.append((index, json.dumps(document, ensure_ascii=False, separators=(",", ":"))))
            with database:
                database.executemany("INSERT INTO templates VALUES (?, ?)", batch)
            del batch
        database.close()
    elapsed = (time.perf_counter() - started) * 1000
    peak_rss = peak_rss_kib()
    with sqlite3.connect(destination) as database:
        count = database.execute("SELECT count(*) FROM templates").fetchone()[0]
    if count != args.count:
        raise RuntimeError(f"Expected {args.count} rows, got {count}")
    print(json.dumps({
        "mode": args.worker,
        "milliseconds": round(elapsed, 2),
        "initialRssKiB": initial_rss,
        "peakRssKiB": peak_rss,
        "additionalPeakRssKiB": None if peak_rss is None else peak_rss - initial_rss,
        "outputBytes": destination.stat().st_size,
        "templateCount": count,
        "outputSha256": None if digest is None else digest.hexdigest(),
    }))


def study(args):
    root = Path(__file__).resolve().parents[2]
    pack = root / "bundled-templates"
    entries = json.loads((pack / "index.json").read_text())
    if not entries:
        raise RuntimeError("The compiled pack is empty")
    bodies = [(pack / f"{entry['name']}.json").read_bytes() for entry in entries]
    sizes = [len(body) for body in bodies]
    compressed_sizes = [len(gzip.compress(body, compresslevel=6, mtime=0)) for body in bodies]
    modes = ["eager-json", "bounded-json", "bounded-gzip-json", "copy-seed", "inflate-seed"]
    with tempfile.TemporaryDirectory(prefix="missal-pack-study-") as directory:
        folder = Path(directory)
        for index in range(args.count):
            body = bodies[index % len(bodies)]
            (folder / f"{index}.json").write_bytes(body)
            (folder / f"{index}.json.gz").write_bytes(gzip.compress(body, compresslevel=6, mtime=0))
        with sqlite3.connect(folder / "seed.sqlite") as database:
            database.execute("CREATE TABLE templates (id INTEGER PRIMARY KEY, document TEXT NOT NULL)")
            database.executemany(
                "INSERT INTO templates VALUES (?, ?)",
                ((index, bodies[index % len(bodies)].decode()) for index in range(args.count)),
            )
        with (folder / "seed.sqlite").open("rb") as source:
            with gzip.GzipFile(filename=str(folder / "seed.sqlite.gz"), mode="wb", compresslevel=6, mtime=0) as output:
                shutil.copyfileobj(source, output, length=1024 * 1024)
        expected_seed_hash = hashlib.sha256((folder / "seed.sqlite").read_bytes()).hexdigest()
        measurements = []
        for mode in modes:
            result = subprocess.run(
                [sys.executable, __file__, "--worker", mode, "--fixture", directory,
                 "--count", str(args.count), "--batch-size", str(args.batch_size)],
                capture_output=True, text=True, check=True,
            )
            measurement = json.loads(result.stdout)
            if mode in ("copy-seed", "inflate-seed") and measurement["outputSha256"] != expected_seed_hash:
                raise RuntimeError("Copied seed differs from the source")
            measurements.append(measurement)
        return {
            "purpose": "Local format and allocation study, not application performance acceptance",
            "environment": {"platform": platform.platform(), "python": platform.python_version()},
            "limitations": [
                "Synthetic fixture repeats the current corpus; it is not 100 distinct templates",
                "Warm local filesystem; no Windows or slow-storage measurements",
                "Minimal SQLite table; no Effect, Drizzle, schema validation, field remapping, or projections",
                "Copy/decompression timings exclude fsync, atomic publication, migrations, and app startup",
                "One sample per mode; timings are exploratory, not statistically established",
                "Process memory uses Linux VmHWM after exec; other platforms report no memory sample",
            ],
            "corpus": {
                "templateCount": len(entries),
                "manifestBytes": (pack / "index.json").stat().st_size,
                "totalDocumentBytes": sum(sizes),
                "totalPerDocumentGzipBytes": sum(compressed_sizes),
                "largestDocumentBytes": max(sizes),
            },
            "fixture": {
                "templateCount": args.count,
                "batchSize": args.batch_size,
                "jsonBytes": sum(sizes[index % len(sizes)] for index in range(args.count)),
                "perDocumentGzipBytes": sum(compressed_sizes[index % len(sizes)] for index in range(args.count)),
                "seedBytes": (folder / "seed.sqlite").stat().st_size,
                "gzipSeedBytes": (folder / "seed.sqlite.gz").stat().st_size,
            },
            "measurements": measurements,
        }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--count", type=int, default=100)
    parser.add_argument("--batch-size", type=int, default=4)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--worker", choices=["eager-json", "bounded-json", "bounded-gzip-json", "copy-seed", "inflate-seed"])
    parser.add_argument("--fixture")
    args = parser.parse_args()
    if args.count < 1 or args.batch_size < 1 or (args.worker and not args.fixture):
        parser.error("Use positive counts and supply --fixture for a worker")
    if args.worker:
        run_worker(args)
        return
    result = study(args)
    rendered = json.dumps(result, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered)
    print(rendered)


if __name__ == "__main__":
    main()
