# Capture contract (work sqlite)

Decision: **capture Active FCC ULS Part 87 aircraft radio licenses, not the rest of ULS and not Form 603.** Work sqlite is `{--db}` (prod `/var/lib/fcc-uls-aircraft/fcc-uls-aircraft.sqlite`). Logical name `fcc-uls-aircraft`. There is no published `current/` copy. The collector watches work sqlite only.

Canonical contract: [capturable-state design principles](https://github.com/alexwoolford/capturable-state/blob/main/docs/design-principles.md) §0 / §7 and [datetime.md](https://github.com/alexwoolford/capturable-state/blob/main/docs/datetime.md). Capture the trickle, not the hose.

Pin: `capturable-state` git tag `v0.1.1` (not a path dep; do not copy `src/*.rs`).

## What is captured

| Table / stream | Capture? | Mode | Why |
| --- | --- | --- | --- |
| `licenses` | **yes** | after | Product. Key `uls_id` |
| `ingest_runs` | **yes** | after | Did last week finish? (`ok` / `skipped` / `failed`) |
| `parse_errors` | **no** | — | Local quarantine; may carry `ingest_id` |
| Zip bytes | **no** | — | Optional `--cache-dir` only |
| Other ULS `.dat` / Form 603 | **no** | — | Not the capture set |
| `_outbox` | platform | — | Generated |

Identity: `uls_id`. Do **not** put `ingest_id` on `licenses` (weekly rewrite would hose `_outbox`). Identical live rows emit no extra outbox `U` (`ON CONFLICT … WHERE` any captured fact differs **or** `deleted_at IS NOT NULL`). Returning licenses clear `deleted_at`. Retract: `deleted_at = CAST(strftime('%s','now') AS INTEGER)` where the `uls_id` is not in this week’s **captured** Active HD+AC set (missing AC is not a keep). No `DELETE`.

`n_number` is canonical (leading `N`) from `AC.n_number`. Blank fleet/portable → NULL. Do not guess from `call_sign`. EN `L` folds onto `licensee_*`; EN `CL` folds onto `contact_*` (this dump currently has zero CL rows). Missing EN `L` still captures HD+AC with NULL licensee fields and a `parse_errors` row. These are **labels**, not a name-match score.

Do not `collect --snapshot` this database.

Same `zip_hash` as last **ok** run is `status=skipped` (exit 0) unless `--force`. Skipped runs are captured After. HD row count below `--min-hd-rows` (default 100000) is `failed` (exit 1).

## Clocks

| Layer | Columns | Type |
| --- | --- | --- |
| Facts | `grant_date`, `expired_date`, `cancellation_date`, `effective_date`, `last_action_date`, `as_of_date` | TEXT `YYYY-MM-DD` |
| Facts | run `started_at` / `finished_at` | TEXT `YYYY-MM-DDTHH:MM:SSZ` |
| Envelope | `_outbox.ts`, `deleted_at` | INTEGER Unix seconds |

## Announce / nudge

`install()` on work sqlite. Announce file stem equals `db_name` `fcc-uls-aircraft`. `ReadWritePaths` include `/var/lib/state-capture/announce` (required when the collector is present) and `-/run/state`. `fcc` must be in group `state-capture`. Collector host inventory lives in mosaic `deploy/ct-firehose/`, not in this crate.
