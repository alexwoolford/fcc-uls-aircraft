use std::ops::{Deref, DerefMut};
use std::path::Path;

use anyhow::{Context, Result};
use capturable_state::{
    apply_runtime_pragmas, install, CaptureConfig, CaptureMode, Nudge, TableSpec,
};
use rusqlite::{params, Connection, OptionalExtension};

use crate::parse::{canonical_n_number, License};

pub const DB_NAME: &str = "fcc-uls-aircraft";

pub struct WorkDb {
    conn: Connection,
    pub nudge: Nudge,
}

impl Deref for WorkDb {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        &self.conn
    }
}

impl DerefMut for WorkDb {
    fn deref_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}

fn open_conn(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
    }
    let conn = Connection::open(path).with_context(|| format!("open {}", path.display()))?;
    apply_runtime_pragmas(&conn)?;
    apply_schema(&conn)?;
    Ok(conn)
}

pub fn open(path: &Path) -> Result<Connection> {
    open_conn(path)
}

pub fn open_work(path: &Path) -> Result<WorkDb> {
    let conn = open_conn(path)?;
    let nudge = install_capture(&conn, path)?;
    Ok(WorkDb { conn, nudge })
}

fn apply_schema(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    conn.execute_batch(include_str!("schema.sql"))
        .context("apply schema")?;
    Ok(())
}

fn install_capture(conn: &Connection, path: &Path) -> Result<Nudge> {
    let tables = [
        TableSpec::new("licenses", CaptureMode::After),
        TableSpec::new("ingest_runs", CaptureMode::After),
    ];
    install(conn, &CaptureConfig::new(DB_NAME, path, &tables))
}

/// Change-aware upsert. Identical live rows emit no extra `_outbox` row.
/// A previously retracted row is restored (`deleted_at` cleared) even if facts match.
pub fn upsert_license(conn: &Connection, lic: &License) -> Result<bool> {
    let n = conn.execute(
        "INSERT INTO licenses (
            uls_id, call_sign, n_number, radio_service, license_status,
            grant_date, expired_date, cancellation_date, effective_date, last_action_date,
            is_fleet, is_portable, aircraft_count, carrier_type, certifier_name,
            licensee_name, licensee_attention, licensee_frn, licensee_street,
            licensee_city, licensee_state, licensee_zip, licensee_po_box, licensee_type,
            contact_name, contact_attention, contact_frn, contact_street,
            contact_city, contact_state, contact_zip, contact_po_box, contact_type,
            deleted_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
            ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
            ?21, ?22, ?23, ?24, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
            NULL
         )
         ON CONFLICT(uls_id) DO UPDATE SET
            call_sign = excluded.call_sign,
            n_number = excluded.n_number,
            radio_service = excluded.radio_service,
            license_status = excluded.license_status,
            grant_date = excluded.grant_date,
            expired_date = excluded.expired_date,
            cancellation_date = excluded.cancellation_date,
            effective_date = excluded.effective_date,
            last_action_date = excluded.last_action_date,
            is_fleet = excluded.is_fleet,
            is_portable = excluded.is_portable,
            aircraft_count = excluded.aircraft_count,
            carrier_type = excluded.carrier_type,
            certifier_name = excluded.certifier_name,
            licensee_name = excluded.licensee_name,
            licensee_attention = excluded.licensee_attention,
            licensee_frn = excluded.licensee_frn,
            licensee_street = excluded.licensee_street,
            licensee_city = excluded.licensee_city,
            licensee_state = excluded.licensee_state,
            licensee_zip = excluded.licensee_zip,
            licensee_po_box = excluded.licensee_po_box,
            licensee_type = excluded.licensee_type,
            contact_name = excluded.contact_name,
            contact_attention = excluded.contact_attention,
            contact_frn = excluded.contact_frn,
            contact_street = excluded.contact_street,
            contact_city = excluded.contact_city,
            contact_state = excluded.contact_state,
            contact_zip = excluded.contact_zip,
            contact_po_box = excluded.contact_po_box,
            contact_type = excluded.contact_type,
            deleted_at = NULL
         WHERE licenses.call_sign IS NOT excluded.call_sign
            OR licenses.n_number IS NOT excluded.n_number
            OR licenses.radio_service IS NOT excluded.radio_service
            OR licenses.license_status IS NOT excluded.license_status
            OR licenses.grant_date IS NOT excluded.grant_date
            OR licenses.expired_date IS NOT excluded.expired_date
            OR licenses.cancellation_date IS NOT excluded.cancellation_date
            OR licenses.effective_date IS NOT excluded.effective_date
            OR licenses.last_action_date IS NOT excluded.last_action_date
            OR licenses.is_fleet IS NOT excluded.is_fleet
            OR licenses.is_portable IS NOT excluded.is_portable
            OR licenses.aircraft_count IS NOT excluded.aircraft_count
            OR licenses.carrier_type IS NOT excluded.carrier_type
            OR licenses.certifier_name IS NOT excluded.certifier_name
            OR licenses.licensee_name IS NOT excluded.licensee_name
            OR licenses.licensee_attention IS NOT excluded.licensee_attention
            OR licenses.licensee_frn IS NOT excluded.licensee_frn
            OR licenses.licensee_street IS NOT excluded.licensee_street
            OR licenses.licensee_city IS NOT excluded.licensee_city
            OR licenses.licensee_state IS NOT excluded.licensee_state
            OR licenses.licensee_zip IS NOT excluded.licensee_zip
            OR licenses.licensee_po_box IS NOT excluded.licensee_po_box
            OR licenses.licensee_type IS NOT excluded.licensee_type
            OR licenses.contact_name IS NOT excluded.contact_name
            OR licenses.contact_attention IS NOT excluded.contact_attention
            OR licenses.contact_frn IS NOT excluded.contact_frn
            OR licenses.contact_street IS NOT excluded.contact_street
            OR licenses.contact_city IS NOT excluded.contact_city
            OR licenses.contact_state IS NOT excluded.contact_state
            OR licenses.contact_zip IS NOT excluded.contact_zip
            OR licenses.contact_po_box IS NOT excluded.contact_po_box
            OR licenses.contact_type IS NOT excluded.contact_type
            OR licenses.deleted_at IS NOT NULL",
        params![
            lic.uls_id,
            lic.call_sign,
            lic.n_number.as_deref(),
            lic.radio_service,
            lic.license_status,
            lic.grant_date.as_deref(),
            lic.expired_date.as_deref(),
            lic.cancellation_date.as_deref(),
            lic.effective_date.as_deref(),
            lic.last_action_date.as_deref(),
            lic.is_fleet,
            lic.is_portable,
            lic.aircraft_count,
            lic.carrier_type.as_deref(),
            lic.certifier_name.as_deref(),
            lic.licensee_name.as_deref(),
            lic.licensee_attention.as_deref(),
            lic.licensee_frn.as_deref(),
            lic.licensee_street.as_deref(),
            lic.licensee_city.as_deref(),
            lic.licensee_state.as_deref(),
            lic.licensee_zip.as_deref(),
            lic.licensee_po_box.as_deref(),
            lic.licensee_type.as_deref(),
        ],
    )?;
    Ok(n > 0)
}

#[derive(Debug)]
pub struct StatusRow {
    pub as_of_date: String,
    pub started_at: String,
    pub finished_at: String,
    pub status: String,
    pub zip_hash: Option<String>,
    pub hd_rows: Option<i64>,
    pub active_upserted: Option<i64>,
    pub unchanged_rows: Option<i64>,
    pub retracted: Option<i64>,
    pub parse_errors: Option<i64>,
}

pub fn last_run(conn: &Connection) -> Result<Option<StatusRow>> {
    conn.query_row(
        "SELECT as_of_date, started_at, COALESCE(finished_at, ''), status,
                zip_hash, hd_rows, active_upserted, unchanged_rows, retracted, parse_errors
         FROM ingest_runs
         ORDER BY id DESC
         LIMIT 1",
        [],
        |r| {
            Ok(StatusRow {
                as_of_date: r.get(0)?,
                started_at: r.get(1)?,
                finished_at: r.get(2)?,
                status: r.get(3)?,
                zip_hash: r.get(4)?,
                hd_rows: r.get(5)?,
                active_upserted: r.get(6)?,
                unchanged_rows: r.get(7)?,
                retracted: r.get(8)?,
                parse_errors: r.get(9)?,
            })
        },
    )
    .optional()
    .context("last ingest_runs")
}

#[derive(Debug, Clone)]
pub struct LicenseHit {
    pub uls_id: String,
    pub call_sign: String,
    pub n_number: Option<String>,
    pub licensee_name: Option<String>,
    pub licensee_frn: Option<String>,
    pub license_status: String,
    pub deleted_at: Option<i64>,
}

pub fn lookup_licenses(conn: &Connection, q: &str) -> Result<Vec<LicenseHit>> {
    let q = q.trim();
    let canon = canonical_n_number(q);
    let upper = q.to_ascii_uppercase();
    let call_alt = canon.strip_prefix('N').unwrap_or(canon.as_str());
    let mut stmt = conn.prepare(
        "SELECT uls_id, call_sign, n_number, licensee_name, licensee_frn,
                license_status, deleted_at
         FROM licenses
         WHERE n_number = ?1
            OR call_sign = ?2
            OR call_sign = ?3
            OR licensee_frn = ?4
            OR licensee_name LIKE ?5 ESCAPE '\\'
         ORDER BY uls_id",
    )?;
    let like = like_substring(q);
    let rows = stmt.query_map(params![canon, upper, call_alt, q, like], |r| {
        Ok(LicenseHit {
            uls_id: r.get(0)?,
            call_sign: r.get(1)?,
            n_number: r.get(2)?,
            licensee_name: r.get(3)?,
            licensee_frn: r.get(4)?,
            license_status: r.get(5)?,
            deleted_at: r.get(6)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn outbox_count(conn: &Connection) -> Result<i64> {
    conn.query_row("SELECT COUNT(*) FROM _outbox", [], |r| r.get(0))
        .context("outbox count")
}

fn like_substring(q: &str) -> String {
    let mut out = String::from("%");
    for c in q.chars() {
        match c {
            '%' | '_' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out.push('%');
    out
}
