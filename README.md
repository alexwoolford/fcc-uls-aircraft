# fcc-uls-aircraft

Weekly **FCC ULS Part 87 Active aircraft radio licenses** from `l_aircr.zip`. A **label**, not a leading observable: the FCC licensee *is* the radio-station licensee. This crate does not emit a name-match score, does not guess N-number from call sign, and is not Form 603.

Production is a systemd oneshot on Linux (`deploy/install.sh`). Capture contract: [docs/CAPTURE.md](docs/CAPTURE.md). Ops: [docs/DAILY_OPS.md](docs/DAILY_OPS.md). Mosaic spec: [FCC_ULS_AIRCRAFT.md](https://github.com/alexwoolford/mosaic/blob/main/docs/research/FCC_ULS_AIRCRAFT.md) (private mosaic).

```bash
cargo run --release -- ingest --zip /path/to/l_aircr.zip --min-hd-rows 1
cargo run --release -- ingest --cache-dir data
cargo run --release -- status
cargo run --release -- lookup N100AS
cargo run --release -- lookup 100AS
```

Laptop default db: `data/fcc-uls-aircraft.sqlite` (`FCC_ULS_SQLITE`). `cargo test` uses fixtures only. It does not need the network.

Ingest exits 0 when `ingest_runs.status` is `ok` or `skipped`. `failed` exits 1.

Pin `capturable-state` git tag `v0.1.1`. Never `path = "../capturable-state"`.
