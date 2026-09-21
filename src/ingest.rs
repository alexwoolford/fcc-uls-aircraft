//! Weekly FCC ULS complete aircraft zip → Active licenses.

use std::time::Instant;

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, Transaction};

use crate::db::{open_work, upsert_license};
use crate::download::{self, DEFAULT_ZIP_URL};
use crate::parse::{join_licenses, parse_ac, parse_en, parse_hd, License, ParseError};
use crate::time::{require_utc_date, require_utc_instant, utc_date, utc_iso};

pub const DEFAULT_MIN_HD_ROWS: usize = 100_000;

#[derive(Debug, Clone)]
pub struct IngestOptions {
    pub db_path: std::path::PathBuf,
    pub zip_path: Option<std::path::PathBuf>,
    pub cache_dir: Option<std::path::PathBuf>,
    pub zip_url: String,
    pub min_hd_rows: usize,
    pub force: bool,
    pub user_agent: String,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self {
            db_path: std::path::PathBuf::from("data/fcc-uls-aircraft.sqlite"),
            zip_path: None,
            cache_dir: None,
            zip_url: DEFAULT_ZIP_URL.to_string(),
            min_hd_rows: DEFAULT_MIN_HD_ROWS,
            force: false,
            user_agent: crate::download::DEFAULT_FCC_USER_AGENT.to_string(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct IngestStats {
    pub source: String,
    pub zip_hash: String,
    pub status: String,
    pub hd_rows: usize,
    pub en_rows: usize,
    pub ac_rows: usize,
    pub active_upserted: usize,
    pub unchanged_rows: usize,
    pub retracted: usize,
    pub parse_errors: usize,
    pub skipped_same_zip: bool,
}

pub fn ingest(opts: &IngestOptions) -> Result<IngestStats> {
    let started = Instant::now();
    let (zip_bytes, source) = if let Some(path) = &opts.zip_path {
        (
            download::read_zip_file(path)?,
            format!("file:{}", path.display()),
        )
    } else {
        (
            download::download_zip(&opts.zip_url, &opts.user_agent)?,
            opts.zip_url.clone(),
        )
    };
    if opts.zip_path.is_none() {
        if let Some(dir) = &opts.cache_dir {
            if let Err(error) = download::write_zip_cache(dir, &zip_bytes) {
                tracing::warn!(error = %error, cache = %dir.display(), "failed to write zip cache");
            }
        }
    }
    let zip_hash = download::zip_hash(&zip_bytes);
    let as_of = utc_date();
    require_utc_date(&as_of, "as_of_date")?;
    let started_at = utc_iso(chrono::Utc::now());
    require_utc_instant(&started_at, "started_at")?;

    let mut work = open_work(&opts.db_path)?;
    if !opts.force {
        if let Some(prev) = last_ok_zip_hash(&work)? {
            if prev == zip_hash {
                record_skipped_run(&work, &as_of, &started_at, &source, &zip_hash)?;
                work.nudge.send();
                tracing::info!(
                    zip_hash = %zip_hash,
                    duration_ms = started.elapsed().as_millis() as u64,
                    "same zip_hash as last ok run; skipping"
                );
                return Ok(IngestStats {
                    source,
                    zip_hash,
                    status: "skipped".into(),
                    skipped_same_zip: true,
                    ..IngestStats::default()
                });
            }
        }
    }

    let (hd_bytes, en_bytes, ac_bytes) = match (
        download::extract_named(&zip_bytes, "HD.dat"),
        download::extract_named(&zip_bytes, "EN.dat"),
        download::extract_named(&zip_bytes, "AC.dat"),
    ) {
        (Ok(hd), Ok(en), Ok(ac)) => (hd, en, ac),
        (hd, en, ac) => {
            let err = hd
                .err()
                .or(en.err())
                .or(ac.err())
                .expect("at least one extract failed");
            record_failed_run(
                &work,
                &as_of,
                &started_at,
                &source,
                &zip_hash,
                0,
                0,
                0,
                0,
                &err.to_string(),
            )?;
            work.nudge.send();
            return Err(err);
        }
    };

    let hd = parse_hd(&hd_bytes);
    let en = parse_en(&en_bytes);
    let ac = parse_ac(&ac_bytes);
    let mut parse_errors = Vec::new();
    parse_errors.extend(hd.errors);
    parse_errors.extend(en.errors);
    parse_errors.extend(ac.errors);

    if hd.records.len() < opts.min_hd_rows {
        let msg = format!(
            "HD.dat has {} data rows; refusing ingest below floor of {}",
            hd.records.len(),
            opts.min_hd_rows
        );
        record_failed_run(
            &work,
            &as_of,
            &started_at,
            &source,
            &zip_hash,
            hd.records.len(),
            en.records.len(),
            ac.records.len(),
            parse_errors.len(),
            &msg,
        )?;
        work.nudge.send();
        tracing::error!(
            hd_rows = hd.records.len(),
            min_hd_rows = opts.min_hd_rows,
            duration_ms = started.elapsed().as_millis() as u64,
            "HD.dat below floor; refusing ingest"
        );
        bail!("{msg}");
    }

    let (licenses, active_ids, join_errors) = join_licenses(&hd.records, &en.records, &ac.records);
    parse_errors.extend(join_errors);

    let mut stats = IngestStats {
        source: source.clone(),
        zip_hash: zip_hash.clone(),
        status: "ok".into(),
        hd_rows: hd.records.len(),
        en_rows: en.records.len(),
        ac_rows: ac.records.len(),
        parse_errors: parse_errors.len(),
        ..IngestStats::default()
    };

    match apply_licenses(
        &mut work,
        &as_of,
        &started_at,
        &source,
        &zip_hash,
        &licenses,
        &active_ids,
        &parse_errors,
        &mut stats,
    ) {
        Ok(()) => {
            work.nudge.send();
            tracing::info!(
                zip_hash = %zip_hash,
                hd_rows = stats.hd_rows,
                active_upserted = stats.active_upserted,
                unchanged = stats.unchanged_rows,
                retracted = stats.retracted,
                parse_errors = stats.parse_errors,
                duration_ms = started.elapsed().as_millis() as u64,
                "ingest complete"
            );
            Ok(stats)
        }
        Err(err) => {
            let _ = record_failed_run(
                &work,
                &as_of,
                &started_at,
                &source,
                &zip_hash,
                stats.hd_rows,
                stats.en_rows,
                stats.ac_rows,
                stats.parse_errors,
                &err.to_string(),
            );
            work.nudge.send();
            Err(err)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_licenses(
    work: &mut crate::db::WorkDb,
    as_of: &str,
    started_at: &str,
    source: &str,
    zip_hash: &str,
    licenses: &[License],
    active_ids: &[String],
    parse_errors: &[ParseError],
    stats: &mut IngestStats,
) -> Result<()> {
    let tx = work.transaction().context("begin ingest transaction")?;
    let ingest_id = insert_run_start(&tx, as_of, started_at, source, zip_hash)?;
    let mut upserted = 0usize;
    let mut unchanged = 0usize;
    for lic in licenses {
        if upsert_license(&tx, lic)? {
            upserted += 1;
        } else {
            unchanged += 1;
        }
    }
    stats.active_upserted = upserted;
    stats.unchanged_rows = unchanged;
    stats.retracted = retract_missing(&tx, active_ids)?;
    insert_parse_errors(&tx, ingest_id, parse_errors)?;
    finish_run(&tx, ingest_id, "ok", stats, None)?;
    tx.commit().context("commit ingest")?;
    Ok(())
}

fn retract_missing(tx: &Transaction<'_>, active_ids: &[String]) -> Result<usize> {
    tx.execute_batch("CREATE TEMP TABLE active_uls (uls_id TEXT PRIMARY KEY)")?;
    {
        let mut ins = tx.prepare("INSERT INTO active_uls (uls_id) VALUES (?1)")?;
        for id in active_ids {
            ins.execute(params![id])?;
        }
    }
    let n = tx.execute(
        "UPDATE licenses
         SET deleted_at = CAST(strftime('%s','now') AS INTEGER)
         WHERE deleted_at IS NULL
           AND uls_id NOT IN (SELECT uls_id FROM active_uls)",
        [],
    )?;
    tx.execute_batch("DROP TABLE active_uls")?;
    Ok(n)
}

fn insert_parse_errors(tx: &Transaction<'_>, ingest_id: i64, errors: &[ParseError]) -> Result<()> {
    let mut stmt = tx.prepare(
        "INSERT INTO parse_errors (ingest_id, file_name, line_number, raw_line, error)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for err in errors {
        stmt.execute(params![
            ingest_id,
            err.file_name,
            err.line_number as i64,
            err.raw_line,
            err.error
        ])?;
    }
    Ok(())
}

fn last_ok_zip_hash(conn: &Connection) -> Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT zip_hash FROM ingest_runs
         WHERE status = 'ok' AND zip_hash IS NOT NULL AND zip_hash != ''
         ORDER BY id DESC LIMIT 1",
    )?;
    let mut rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    match rows.next() {
        Some(hash) => Ok(Some(hash?)),
        None => Ok(None),
    }
}

fn record_skipped_run(
    conn: &Connection,
    as_of: &str,
    started_at: &str,
    source: &str,
    zip_hash: &str,
) -> Result<()> {
    let finished = utc_iso(chrono::Utc::now());
    require_utc_instant(&finished, "finished_at")?;
    conn.execute(
        "INSERT INTO ingest_runs (
            as_of_date, started_at, finished_at, source, zip_hash, status
         ) VALUES (?1, ?2, ?3, ?4, ?5, 'skipped')",
        params![as_of, started_at, finished, source, zip_hash],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn record_failed_run(
    conn: &Connection,
    as_of: &str,
    started_at: &str,
    source: &str,
    zip_hash: &str,
    hd_rows: usize,
    en_rows: usize,
    ac_rows: usize,
    parse_errors: usize,
    error: &str,
) -> Result<()> {
    let finished = utc_iso(chrono::Utc::now());
    require_utc_instant(&finished, "finished_at")?;
    conn.execute(
        "INSERT INTO ingest_runs (
            as_of_date, started_at, finished_at, source, zip_hash,
            hd_rows, en_rows, ac_rows, parse_errors, status, error
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'failed', ?10)",
        params![
            as_of,
            started_at,
            finished,
            source,
            zip_hash,
            hd_rows as i64,
            en_rows as i64,
            ac_rows as i64,
            parse_errors as i64,
            error
        ],
    )?;
    Ok(())
}

fn insert_run_start(
    conn: &Connection,
    as_of: &str,
    started_at: &str,
    source: &str,
    zip_hash: &str,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO ingest_runs (as_of_date, started_at, source, zip_hash, status)
         VALUES (?1, ?2, ?3, ?4, 'running')",
        params![as_of, started_at, source, zip_hash],
    )?;
    Ok(conn.last_insert_rowid())
}

fn finish_run(
    conn: &Connection,
    ingest_id: i64,
    status: &str,
    stats: &IngestStats,
    error: Option<&str>,
) -> Result<()> {
    let finished = utc_iso(chrono::Utc::now());
    require_utc_instant(&finished, "finished_at")?;
    conn.execute(
        "UPDATE ingest_runs SET
            finished_at = ?1,
            hd_rows = ?2,
            en_rows = ?3,
            ac_rows = ?4,
            active_upserted = ?5,
            unchanged_rows = ?6,
            retracted = ?7,
            parse_errors = ?8,
            status = ?9,
            error = ?10
         WHERE id = ?11",
        params![
            finished,
            stats.hd_rows as i64,
            stats.en_rows as i64,
            stats.ac_rows as i64,
            stats.active_upserted as i64,
            stats.unchanged_rows as i64,
            stats.retracted as i64,
            stats.parse_errors as i64,
            status,
            error,
            ingest_id,
        ],
    )?;
    Ok(())
}
