CREATE TABLE IF NOT EXISTS licenses (
    uls_id TEXT PRIMARY KEY,
    call_sign TEXT NOT NULL,
    n_number TEXT,
    radio_service TEXT NOT NULL,
    license_status TEXT NOT NULL,
    grant_date TEXT,
    expired_date TEXT,
    cancellation_date TEXT,
    effective_date TEXT,
    last_action_date TEXT,
    is_fleet INTEGER NOT NULL CHECK (is_fleet IN (0, 1)),
    is_portable INTEGER NOT NULL CHECK (is_portable IN (0, 1)),
    aircraft_count INTEGER,
    carrier_type TEXT,
    certifier_name TEXT,
    licensee_name TEXT,
    licensee_attention TEXT,
    licensee_frn TEXT,
    licensee_street TEXT,
    licensee_city TEXT,
    licensee_state TEXT,
    licensee_zip TEXT,
    licensee_po_box TEXT,
    licensee_type TEXT,
    contact_name TEXT,
    contact_attention TEXT,
    contact_frn TEXT,
    contact_street TEXT,
    contact_city TEXT,
    contact_state TEXT,
    contact_zip TEXT,
    contact_po_box TEXT,
    contact_type TEXT,
    deleted_at INTEGER
) STRICT;

CREATE INDEX IF NOT EXISTS idx_licenses_n_number ON licenses(n_number);
CREATE INDEX IF NOT EXISTS idx_licenses_call_sign ON licenses(call_sign);
CREATE INDEX IF NOT EXISTS idx_licenses_licensee_frn ON licenses(licensee_frn);
CREATE INDEX IF NOT EXISTS idx_licenses_licensee_name ON licenses(licensee_name);

CREATE TABLE IF NOT EXISTS ingest_runs (
    id INTEGER PRIMARY KEY,
    as_of_date TEXT NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    source TEXT NOT NULL,
    zip_hash TEXT,
    hd_rows INTEGER,
    en_rows INTEGER,
    ac_rows INTEGER,
    active_upserted INTEGER,
    unchanged_rows INTEGER,
    retracted INTEGER,
    parse_errors INTEGER,
    status TEXT NOT NULL CHECK (status IN ('running', 'ok', 'failed', 'skipped')),
    error TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS parse_errors (
    id INTEGER PRIMARY KEY,
    ingest_id INTEGER NOT NULL,
    file_name TEXT NOT NULL,
    line_number INTEGER NOT NULL,
    raw_line TEXT NOT NULL,
    error TEXT NOT NULL,
    FOREIGN KEY (ingest_id) REFERENCES ingest_runs(id)
) STRICT;
