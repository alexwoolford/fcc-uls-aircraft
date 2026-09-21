use fcc_uls_aircraft::db::{lookup_licenses, open};
use fcc_uls_aircraft::download::write_test_zip;
use fcc_uls_aircraft::ingest::{ingest, IngestOptions};
use fcc_uls_aircraft::parse::{
    SAMPLE_AC_FLEET, SAMPLE_AC_LLC, SAMPLE_AC_NO_ATTN, SAMPLE_EN_FLEET, SAMPLE_EN_LLC,
    SAMPLE_EN_NO_ATTN, SAMPLE_HD_EXPIRED, SAMPLE_HD_FLEET, SAMPLE_HD_LLC, SAMPLE_HD_NO_ATTN,
};

fn join_lines(lines: &[&str]) -> Vec<u8> {
    let mut out = String::new();
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
    out.into_bytes()
}

fn zip_from(hd: &[&str], en: &[&str], ac: &[&str]) -> Vec<u8> {
    write_test_zip(&[
        ("HD.dat", &join_lines(hd)),
        ("EN.dat", &join_lines(en)),
        ("AC.dat", &join_lines(ac)),
    ])
    .expect("zip")
}

fn opts(db: &std::path::Path, zip: &std::path::Path, force: bool) -> IngestOptions {
    IngestOptions {
        db_path: db.to_path_buf(),
        zip_path: Some(zip.to_path_buf()),
        cache_dir: None,
        zip_url: "file://fixture".into(),
        min_hd_rows: 1,
        force,
        user_agent: "fcc-uls-aircraft-test/0".into(),
    }
}

fn write_zip(dir: &std::path::Path, bytes: &[u8]) -> std::path::PathBuf {
    let path = dir.join("l_aircr.zip");
    std::fs::write(&path, bytes).unwrap();
    path
}

fn full_fixture() -> Vec<u8> {
    zip_from(
        &[
            SAMPLE_HD_LLC,
            SAMPLE_HD_NO_ATTN,
            SAMPLE_HD_EXPIRED,
            SAMPLE_HD_FLEET,
        ],
        &[SAMPLE_EN_LLC, SAMPLE_EN_NO_ATTN, SAMPLE_EN_FLEET],
        &[SAMPLE_AC_LLC, SAMPLE_AC_NO_ATTN, SAMPLE_AC_FLEET],
    )
}

#[test]
fn ingest_samples_skip_expired_and_lookup() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("fcc-uls-aircraft.sqlite");
    let zip = write_zip(tmp.path(), &full_fixture());

    let stats = ingest(&opts(&db, &zip, false)).unwrap();
    assert_eq!(stats.status, "ok");
    assert!(!stats.skipped_same_zip);
    assert_eq!(stats.hd_rows, 4);
    assert_eq!(stats.active_upserted, 3);
    assert_eq!(stats.retracted, 0);

    let conn = open(&db).unwrap();
    let expired: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM licenses WHERE uls_id = '14459'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(expired, 0);

    let by_n = lookup_licenses(&conn, "N100AS").unwrap();
    let by_bare = lookup_licenses(&conn, "100AS").unwrap();
    assert_eq!(by_n.len(), 1);
    assert_eq!(by_bare.len(), 1);
    assert_eq!(by_n[0].n_number.as_deref(), Some("N100AS"));
    assert_eq!(by_bare[0].uls_id, "14518");

    let llc = lookup_licenses(&conn, "N759ZD").unwrap();
    assert_eq!(llc[0].n_number.as_deref(), Some("N759ZD"));
    assert_eq!(llc[0].call_sign, "759ZD");

    let fleet: Option<String> = conn
        .query_row(
            "SELECT n_number FROM licenses WHERE uls_id = '999001'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(fleet, None);
    let fleet_flag: i64 = conn
        .query_row(
            "SELECT is_fleet FROM licenses WHERE uls_id = '999001'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(fleet_flag, 1);

    let skipped = ingest(&opts(&db, &zip, false)).unwrap();
    assert!(skipped.skipped_same_zip);
    assert_eq!(skipped.status, "skipped");
    let skip_status: String = conn
        .query_row(
            "SELECT status FROM ingest_runs ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(skip_status, "skipped");

    let forced = ingest(&opts(&db, &zip, true)).unwrap();
    assert!(!forced.skipped_same_zip);
    assert_eq!(forced.unchanged_rows, 3);
    assert_eq!(forced.active_upserted, 0);
}

#[test]
fn extra_en_columns_null_address_and_parse_error() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("fcc-uls-aircraft.sqlite");
    let en = format!("{SAMPLE_EN_LLC}|PIPE");
    let bytes = zip_from(&[SAMPLE_HD_LLC], &[&en], &[SAMPLE_AC_LLC]);
    let zip = write_zip(tmp.path(), &bytes);
    let stats = ingest(&opts(&db, &zip, false)).unwrap();
    assert!(stats.parse_errors >= 1);
    let conn = open(&db).unwrap();
    let (attn, street, frn): (Option<String>, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT licensee_attention, licensee_street, licensee_frn FROM licenses WHERE uls_id = '2633746'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(attn, None);
    assert_eq!(street, None);
    assert_eq!(frn, None);
    let name: String = conn
        .query_row(
            "SELECT licensee_name FROM licenses WHERE uls_id = '2633746'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(name.starts_with("N759ZD, LLC"));
}

#[test]
fn retract_missing_uls_id_sets_deleted_at() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("fcc-uls-aircraft.sqlite");
    let both = zip_from(
        &[SAMPLE_HD_LLC, SAMPLE_HD_NO_ATTN],
        &[SAMPLE_EN_LLC, SAMPLE_EN_NO_ATTN],
        &[SAMPLE_AC_LLC, SAMPLE_AC_NO_ATTN],
    );
    let zip = write_zip(tmp.path(), &both);
    ingest(&opts(&db, &zip, false)).unwrap();

    let only_llc = zip_from(&[SAMPLE_HD_LLC], &[SAMPLE_EN_LLC], &[SAMPLE_AC_LLC]);
    std::fs::write(&zip, only_llc).unwrap();
    let stats = ingest(&opts(&db, &zip, false)).unwrap();
    assert_eq!(stats.retracted, 1);

    let conn = open(&db).unwrap();
    let deleted: Option<i64> = conn
        .query_row(
            "SELECT deleted_at FROM licenses WHERE uls_id = '14518'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(deleted.is_some() && deleted.unwrap() > 0);
    let live: Option<i64> = conn
        .query_row(
            "SELECT deleted_at FROM licenses WHERE uls_id = '2633746'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(live, None);
}

#[test]
fn refuse_truncated_hd() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("fcc-uls-aircraft.sqlite");
    let bytes = zip_from(&[SAMPLE_HD_LLC], &[SAMPLE_EN_LLC], &[SAMPLE_AC_LLC]);
    let zip = write_zip(tmp.path(), &bytes);
    let mut o = opts(&db, &zip, false);
    o.min_hd_rows = 100_000;
    let err = ingest(&o).unwrap_err();
    assert!(err.to_string().contains("refusing ingest"));
    let conn = open(&db).unwrap();
    let status: String = conn
        .query_row(
            "SELECT status FROM ingest_runs ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "failed");
}
