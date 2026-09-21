//! Pipe-delimited FCC ULS `.dat` rows. 1-based fields; extra trailing columns tolerated.

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;

/// Uppercase, strip spaces/dashes/dots, ensure a leading `N`. Empty stays empty.
pub fn canonical_n_number(raw: &str) -> String {
    let s = raw.trim().to_uppercase().replace(['-', ' ', '.'], "");
    if s.is_empty() {
        return s;
    }
    if s.starts_with('N') {
        s
    } else {
        format!("N{s}")
    }
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub file_name: String,
    pub line_number: usize,
    pub raw_line: String,
    pub error: String,
}

#[derive(Debug, Clone)]
pub struct HdRow {
    pub line_number: usize,
    pub raw_line: String,
    pub uls_id: String,
    pub call_sign: String,
    pub license_status: String,
    pub radio_service: String,
    pub grant_date: Option<String>,
    pub expired_date: Option<String>,
    pub cancellation_date: Option<String>,
    pub effective_date: Option<String>,
    pub last_action_date: Option<String>,
    pub certifier_name: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct EnRow {
    pub uls_id: String,
    pub entity_type: String,
    pub licensee_name: Option<String>,
    pub licensee_attention: Option<String>,
    pub licensee_frn: Option<String>,
    pub licensee_street: Option<String>,
    pub licensee_city: Option<String>,
    pub licensee_state: Option<String>,
    pub licensee_zip: Option<String>,
    pub licensee_po_box: Option<String>,
    pub licensee_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AcRow {
    pub uls_id: String,
    pub n_number: Option<String>,
    pub is_fleet: i64,
    pub is_portable: i64,
    pub aircraft_count: Option<i64>,
    pub carrier_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct License {
    pub uls_id: String,
    pub call_sign: String,
    pub n_number: Option<String>,
    pub radio_service: String,
    pub license_status: String,
    pub grant_date: Option<String>,
    pub expired_date: Option<String>,
    pub cancellation_date: Option<String>,
    pub effective_date: Option<String>,
    pub last_action_date: Option<String>,
    pub is_fleet: i64,
    pub is_portable: i64,
    pub aircraft_count: Option<i64>,
    pub carrier_type: Option<String>,
    pub certifier_name: Option<String>,
    pub licensee_name: Option<String>,
    pub licensee_attention: Option<String>,
    pub licensee_frn: Option<String>,
    pub licensee_street: Option<String>,
    pub licensee_city: Option<String>,
    pub licensee_state: Option<String>,
    pub licensee_zip: Option<String>,
    pub licensee_po_box: Option<String>,
    pub licensee_type: Option<String>,
    pub contact_name: Option<String>,
    pub contact_attention: Option<String>,
    pub contact_frn: Option<String>,
    pub contact_street: Option<String>,
    pub contact_city: Option<String>,
    pub contact_state: Option<String>,
    pub contact_zip: Option<String>,
    pub contact_po_box: Option<String>,
    pub contact_type: Option<String>,
}

#[derive(Debug, Default)]
pub struct DatParse<T> {
    pub records: Vec<T>,
    pub errors: Vec<ParseError>,
}

/// Split on `|`. 1-based. Trim. Empty → None.
pub fn field<'a>(cols: &'a [&'a str], n: usize) -> Option<&'a str> {
    let s = cols.get(n.checked_sub(1)?)?.trim();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn owned(cols: &[&str], n: usize) -> Option<String> {
    field(cols, n).map(str::to_string)
}

fn display_name(parts: &[Option<String>]) -> Option<String> {
    let s = parts
        .iter()
        .flatten()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn fcc_date(raw: Option<&str>) -> Result<Option<String>, String> {
    let Some(s) = raw else {
        return Ok(None);
    };
    NaiveDate::parse_from_str(s, "%m/%d/%Y")
        .map(|d| Some(d.format("%Y-%m-%d").to_string()))
        .map_err(|_| format!("invalid FCC date {s:?}"))
}

fn yn(raw: Option<&str>) -> Result<i64, String> {
    match raw {
        None => Ok(0),
        Some("Y") => Ok(1),
        Some("N") => Ok(0),
        Some(other) => Err(format!("invalid Y/N {other:?}")),
    }
}

fn int_field(raw: Option<&str>) -> Result<Option<i64>, String> {
    let Some(s) = raw else {
        return Ok(None);
    };
    s.parse::<i64>()
        .map(Some)
        .map_err(|_| format!("invalid integer {s:?}"))
}

fn decode_lines(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn push_err(
    errors: &mut Vec<ParseError>,
    file_name: &str,
    line_number: usize,
    raw_line: &str,
    error: String,
) {
    errors.push(ParseError {
        file_name: file_name.to_string(),
        line_number,
        raw_line: raw_line.to_string(),
        error,
    });
}

fn parse_hd_line(line: &str, line_number: usize, errors: &mut Vec<ParseError>) -> Option<HdRow> {
    let cols: Vec<&str> = line.split('|').collect();
    if field(&cols, 1) != Some("HD") {
        push_err(
            errors,
            "HD.dat",
            line_number,
            line,
            "expected record type HD".into(),
        );
        return None;
    }
    let uls_id = match owned(&cols, 2) {
        Some(id) => id,
        None => {
            push_err(errors, "HD.dat", line_number, line, "missing uls_id".into());
            return None;
        }
    };
    let call_sign = match owned(&cols, 5) {
        Some(cs) => cs,
        None => {
            push_err(
                errors,
                "HD.dat",
                line_number,
                line,
                "missing call_sign".into(),
            );
            return None;
        }
    };
    let license_status = match owned(&cols, 6) {
        Some(s) => s,
        None => {
            push_err(
                errors,
                "HD.dat",
                line_number,
                line,
                "missing license_status".into(),
            );
            return None;
        }
    };
    let radio_service = match owned(&cols, 7) {
        Some(s) => s,
        None => {
            push_err(
                errors,
                "HD.dat",
                line_number,
                line,
                "missing radio_service".into(),
            );
            return None;
        }
    };

    let mut take_date = |n: usize| match fcc_date(field(&cols, n)) {
        Ok(d) => d,
        Err(err) => {
            push_err(errors, "HD.dat", line_number, line, err);
            None
        }
    };

    Some(HdRow {
        line_number,
        raw_line: line.to_string(),
        uls_id,
        call_sign,
        license_status,
        radio_service,
        grant_date: take_date(8),
        expired_date: take_date(9),
        cancellation_date: take_date(10),
        effective_date: take_date(43),
        last_action_date: take_date(44),
        certifier_name: display_name(&[owned(&cols, 31), owned(&cols, 32), owned(&cols, 33)]),
    })
}

fn parse_en_line(line: &str, line_number: usize, errors: &mut Vec<ParseError>) -> Option<EnRow> {
    let cols: Vec<&str> = line.split('|').collect();
    if field(&cols, 1) != Some("EN") {
        push_err(
            errors,
            "EN.dat",
            line_number,
            line,
            "expected record type EN".into(),
        );
        return None;
    }
    let uls_id = match owned(&cols, 2) {
        Some(id) => id,
        None => {
            push_err(errors, "EN.dat", line_number, line, "missing uls_id".into());
            return None;
        }
    };
    let entity_type = match owned(&cols, 6).as_deref() {
        Some("L") => "L".to_string(),
        Some("CL") => "CL".to_string(),
        _ => return None,
    };

    let entity_name = owned(&cols, 8);
    let first = owned(&cols, 9);
    let last = owned(&cols, 11);
    let licensee_name = entity_name.or_else(|| display_name(&[first, last]));
    let shifted = cols.len() > 30;
    if shifted {
        push_err(
            errors,
            "EN.dat",
            line_number,
            line,
            format!(
                "EN ncols={} (pipe in later text); nulling attention/address/FRN",
                cols.len()
            ),
        );
    }

    Some(EnRow {
        uls_id,
        entity_type,
        licensee_name,
        licensee_attention: if shifted { None } else { owned(&cols, 21) },
        licensee_frn: if shifted { None } else { owned(&cols, 23) },
        licensee_street: if shifted { None } else { owned(&cols, 16) },
        licensee_city: if shifted { None } else { owned(&cols, 17) },
        licensee_state: if shifted { None } else { owned(&cols, 18) },
        licensee_zip: if shifted { None } else { owned(&cols, 19) },
        licensee_po_box: if shifted { None } else { owned(&cols, 20) },
        licensee_type: owned(&cols, 24),
    })
}

fn parse_ac_line(line: &str, line_number: usize, errors: &mut Vec<ParseError>) -> Option<AcRow> {
    let cols: Vec<&str> = line.split('|').collect();
    if field(&cols, 1) != Some("AC") {
        push_err(
            errors,
            "AC.dat",
            line_number,
            line,
            "expected record type AC".into(),
        );
        return None;
    }
    let uls_id = match owned(&cols, 2) {
        Some(id) => id,
        None => {
            push_err(errors, "AC.dat", line_number, line, "missing uls_id".into());
            return None;
        }
    };

    let n_number = field(&cols, 10)
        .map(canonical_n_number)
        .filter(|s| !s.is_empty());

    let is_portable = match yn(field(&cols, 8)) {
        Ok(v) => v,
        Err(err) => {
            push_err(errors, "AC.dat", line_number, line, err);
            0
        }
    };
    let is_fleet = match yn(field(&cols, 9)) {
        Ok(v) => v,
        Err(err) => {
            push_err(errors, "AC.dat", line_number, line, err);
            0
        }
    };
    let aircraft_count = match int_field(field(&cols, 6)) {
        Ok(v) => v,
        Err(err) => {
            push_err(errors, "AC.dat", line_number, line, err);
            None
        }
    };

    Some(AcRow {
        uls_id,
        n_number,
        is_fleet,
        is_portable,
        aircraft_count,
        carrier_type: owned(&cols, 7),
    })
}

pub fn parse_hd(bytes: &[u8]) -> DatParse<HdRow> {
    parse_file(&decode_lines(bytes), parse_hd_line)
}

pub fn parse_en(bytes: &[u8]) -> DatParse<EnRow> {
    parse_file(&decode_lines(bytes), parse_en_line)
}

pub fn parse_ac(bytes: &[u8]) -> DatParse<AcRow> {
    parse_file(&decode_lines(bytes), parse_ac_line)
}

fn parse_file<T>(
    text: &str,
    parse_line: fn(&str, usize, &mut Vec<ParseError>) -> Option<T>,
) -> DatParse<T> {
    let mut records = Vec::new();
    let mut errors = Vec::new();
    for (i, raw) in text.split('\n').enumerate() {
        let line = raw.trim_end_matches('\r').trim_end();
        if line.is_empty() {
            continue;
        }
        if let Some(rec) = parse_line(line, i + 1, &mut errors) {
            records.push(rec);
        }
    }
    DatParse { records, errors }
}

/// Join HD–AC–EN. Capture HD status `A` only.
pub fn join_licenses(
    hd: &[HdRow],
    en: &[EnRow],
    ac: &[AcRow],
) -> (Vec<License>, Vec<String>, Vec<ParseError>) {
    let mut errors = Vec::new();
    let mut licensee_by: HashMap<String, EnRow> = HashMap::new();
    let mut contact_by: HashMap<String, EnRow> = HashMap::new();
    for row in en {
        let dest = if row.entity_type == "CL" {
            &mut contact_by
        } else {
            &mut licensee_by
        };
        if dest.contains_key(&row.uls_id) {
            let kind = if row.entity_type == "CL" { "CL" } else { "L" };
            errors.push(ParseError {
                file_name: "EN.dat".into(),
                line_number: 0,
                raw_line: row.uls_id.clone(),
                error: format!("duplicate EN {kind} for uls_id {}; first wins", row.uls_id),
            });
            continue;
        }
        dest.insert(row.uls_id.clone(), row.clone());
    }
    let mut ac_by: HashMap<String, AcRow> = HashMap::new();
    for row in ac {
        if ac_by.contains_key(&row.uls_id) {
            errors.push(ParseError {
                file_name: "AC.dat".into(),
                line_number: 0,
                raw_line: row.uls_id.clone(),
                error: format!("duplicate AC for uls_id {}; first wins", row.uls_id),
            });
            continue;
        }
        ac_by.insert(row.uls_id.clone(), row.clone());
    }

    let mut seen_hd: HashMap<String, ()> = HashMap::new();
    let mut licenses = Vec::new();
    let mut active_set = HashSet::new();
    for row in hd {
        if row.license_status == "A" {
            active_set.insert(row.uls_id.clone());
        }
        if seen_hd.contains_key(&row.uls_id) {
            errors.push(ParseError {
                file_name: "HD.dat".into(),
                line_number: row.line_number,
                raw_line: row.raw_line.clone(),
                error: format!("duplicate HD for uls_id {}; first wins", row.uls_id),
            });
            continue;
        }
        seen_hd.insert(row.uls_id.clone(), ());
        if row.license_status != "A" {
            continue;
        }
        let Some(ac_row) = ac_by.get(&row.uls_id) else {
            errors.push(ParseError {
                file_name: "HD.dat".into(),
                line_number: row.line_number,
                raw_line: row.raw_line.clone(),
                error: format!("missing AC for Active uls_id {}", row.uls_id),
            });
            continue;
        };
        let en_row = licensee_by.get(&row.uls_id);
        if en_row.is_none() {
            errors.push(ParseError {
                file_name: "HD.dat".into(),
                line_number: row.line_number,
                raw_line: row.raw_line.clone(),
                error: format!("missing EN L for Active uls_id {}", row.uls_id),
            });
        }
        let empty = EnRow::default();
        let licensee = en_row.unwrap_or(&empty);
        let contact = contact_by.get(&row.uls_id);
        licenses.push(License {
            uls_id: row.uls_id.clone(),
            call_sign: row.call_sign.clone(),
            n_number: ac_row.n_number.clone(),
            radio_service: row.radio_service.clone(),
            license_status: row.license_status.clone(),
            grant_date: row.grant_date.clone(),
            expired_date: row.expired_date.clone(),
            cancellation_date: row.cancellation_date.clone(),
            effective_date: row.effective_date.clone(),
            last_action_date: row.last_action_date.clone(),
            is_fleet: ac_row.is_fleet,
            is_portable: ac_row.is_portable,
            aircraft_count: ac_row.aircraft_count,
            carrier_type: ac_row.carrier_type.clone(),
            certifier_name: row.certifier_name.clone(),
            licensee_name: licensee.licensee_name.clone(),
            licensee_attention: licensee.licensee_attention.clone(),
            licensee_frn: licensee.licensee_frn.clone(),
            licensee_street: licensee.licensee_street.clone(),
            licensee_city: licensee.licensee_city.clone(),
            licensee_state: licensee.licensee_state.clone(),
            licensee_zip: licensee.licensee_zip.clone(),
            licensee_po_box: licensee.licensee_po_box.clone(),
            licensee_type: licensee.licensee_type.clone(),
            contact_name: contact.and_then(|c| c.licensee_name.clone()),
            contact_attention: contact.and_then(|c| c.licensee_attention.clone()),
            contact_frn: contact.and_then(|c| c.licensee_frn.clone()),
            contact_street: contact.and_then(|c| c.licensee_street.clone()),
            contact_city: contact.and_then(|c| c.licensee_city.clone()),
            contact_state: contact.and_then(|c| c.licensee_state.clone()),
            contact_zip: contact.and_then(|c| c.licensee_zip.clone()),
            contact_po_box: contact.and_then(|c| c.licensee_po_box.clone()),
            contact_type: contact.and_then(|c| c.licensee_type.clone()),
        });
    }
    let active_ids: Vec<String> = active_set.into_iter().collect();
    (licenses, active_ids, errors)
}

pub const SAMPLE_HD_LLC: &str = "HD|2633746|0001792322||759ZD|A|AC|05/01/2024|07/02/2034||||||||||||||||||||N||Paul|A|Lange||||||||||05/01/2024|05/01/2024|||||||||||||||";
pub const SAMPLE_EN_LLC: &str = "EN|2633746|||759ZD|L|L00641528|N759ZD, LLC c/o Law Offices of Paul A. Lange||||||||80 Ferry Boulevard|Stratford|CT|06615||Paul A. Lange|000|0011173515|L||||||";
pub const SAMPLE_EN_CL: &str = "EN|2633746|||759ZD|CL|C00000001|Law Offices of Paul A. Lange||||||||80 Ferry Boulevard|Stratford|CT|06615||Paul A. Lange|000|0011173515|L||||||";
pub const SAMPLE_AC_LLC: &str = "AC|2633746|||759ZD||P|N|N|759ZD";

pub const SAMPLE_HD_NO_ATTN: &str = "HD|14518|||100AS|A|AC|06/02/2018|07/23/2028||||||||||||||||||||N||||||||||||||06/02/2018|06/02/2018|||||||||||||||";
pub const SAMPLE_EN_NO_ATTN: &str = "EN|14518|||100AS|L|L01398728|CITY AVIATION SERVICES INC||||||||3400 E LAFAYETTE|DETROIT|MI|48207|||000|0017757923|C||||||";
pub const SAMPLE_AC_NO_ATTN: &str = "AC|14518|||100AS||P|N|N|100AS";

pub const SAMPLE_HD_EXPIRED: &str = "HD|14459|||10007|E|AC|09/22/1994|09/18/2004|11/20/2004|||||||||||||||||||N||||||||||||||09/22/1994|11/20/2004|||||||||||||||";

pub const SAMPLE_HD_FLEET: &str = "HD|999001|||FLEET1|A|AC|01/01/2024|01/01/2034||||||||||||||||||||N||||||||||||||01/01/2024|01/01/2024|||||||||||||||";
pub const SAMPLE_EN_FLEET: &str = "EN|999001|||FLEET1|L|L00000001|FLEET LICENSEE LLC||||||||1 MAIN|ANYTOWN|CT|00000|||000|0010000001|C||||||";
pub const SAMPLE_AC_FLEET: &str = "AC|999001|||FLEET1||P|N|Y|";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_n_keeps_leading_n() {
        assert_eq!(canonical_n_number("759ZD"), "N759ZD");
        assert_eq!(canonical_n_number("n100as"), "N100AS");
        assert_eq!(canonical_n_number("  N-66-CL  "), "N66CL");
        assert_eq!(canonical_n_number(""), "");
    }

    #[test]
    fn field_is_one_based_trim_empty_none() {
        let cols: Vec<&str> = "A| b ||".split('|').collect();
        assert_eq!(field(&cols, 1), Some("A"));
        assert_eq!(field(&cols, 2), Some("b"));
        assert_eq!(field(&cols, 3), None);
        assert_eq!(field(&cols, 4), None);
    }

    #[test]
    fn parses_llc_sample() {
        let hd = parse_hd(SAMPLE_HD_LLC.as_bytes());
        let en = parse_en(SAMPLE_EN_LLC.as_bytes());
        let ac = parse_ac(SAMPLE_AC_LLC.as_bytes());
        assert!(hd.errors.is_empty());
        assert!(en.errors.is_empty());
        assert!(ac.errors.is_empty());
        let (lic, active, join_err) = join_licenses(&hd.records, &en.records, &ac.records);
        assert!(join_err.is_empty());
        assert_eq!(active, vec!["2633746".to_string()]);
        assert_eq!(lic.len(), 1);
        let row = &lic[0];
        assert_eq!(row.call_sign, "759ZD");
        assert_eq!(row.n_number.as_deref(), Some("N759ZD"));
        assert_eq!(row.licensee_attention.as_deref(), Some("Paul A. Lange"));
        assert_eq!(row.certifier_name.as_deref(), Some("Paul A Lange"));
        assert_eq!(row.licensee_city.as_deref(), Some("Stratford"));
        assert_eq!(row.grant_date.as_deref(), Some("2024-05-01"));
        assert_eq!(row.is_fleet, 0);
        assert_eq!(row.is_portable, 0);
        assert_eq!(row.contact_name, None);
    }

    #[test]
    fn folds_cl_contact_onto_license() {
        let hd = parse_hd(SAMPLE_HD_LLC.as_bytes());
        let en_text = format!("{SAMPLE_EN_LLC}\n{SAMPLE_EN_CL}");
        let en = parse_en(en_text.as_bytes());
        let ac = parse_ac(SAMPLE_AC_LLC.as_bytes());
        let (lic, _, join_err) = join_licenses(&hd.records, &en.records, &ac.records);
        assert!(join_err.is_empty());
        assert_eq!(lic.len(), 1);
        assert_eq!(
            lic[0].contact_name.as_deref(),
            Some("Law Offices of Paul A. Lange")
        );
        assert_eq!(lic[0].contact_attention.as_deref(), Some("Paul A. Lange"));
        assert_eq!(lic[0].contact_city.as_deref(), Some("Stratford"));
        assert_eq!(
            lic[0].licensee_name.as_deref(),
            Some("N759ZD, LLC c/o Law Offices of Paul A. Lange")
        );
    }

    #[test]
    fn missing_en_l_still_captures_hd_ac() {
        let hd = parse_hd(SAMPLE_HD_LLC.as_bytes());
        let ac = parse_ac(SAMPLE_AC_LLC.as_bytes());
        let (lic, _, join_err) = join_licenses(&hd.records, &[], &ac.records);
        assert_eq!(lic.len(), 1);
        assert_eq!(lic[0].n_number.as_deref(), Some("N759ZD"));
        assert_eq!(lic[0].licensee_name, None);
        assert!(join_err
            .iter()
            .any(|e| e.error.contains("missing EN L for Active uls_id 2633746")));
    }

    #[test]
    fn parses_no_attention_sample() {
        let hd = parse_hd(SAMPLE_HD_NO_ATTN.as_bytes());
        let en = parse_en(SAMPLE_EN_NO_ATTN.as_bytes());
        let ac = parse_ac(SAMPLE_AC_NO_ATTN.as_bytes());
        let (lic, _, _) = join_licenses(&hd.records, &en.records, &ac.records);
        assert_eq!(lic[0].licensee_attention, None);
        assert_eq!(
            lic[0].licensee_name.as_deref(),
            Some("CITY AVIATION SERVICES INC")
        );
        assert_eq!(lic[0].n_number.as_deref(), Some("N100AS"));
    }

    #[test]
    fn expired_hd_not_joined() {
        let hd = parse_hd(SAMPLE_HD_EXPIRED.as_bytes());
        assert_eq!(hd.records[0].license_status, "E");
        let (lic, active, _) = join_licenses(&hd.records, &[], &[]);
        assert!(lic.is_empty());
        assert!(active.is_empty());
    }

    #[test]
    fn fleet_blank_n_number_is_null() {
        let ac = parse_ac(SAMPLE_AC_FLEET.as_bytes());
        assert_eq!(ac.records[0].n_number, None);
        assert_eq!(ac.records[0].is_fleet, 1);
        assert_eq!(ac.records[0].is_portable, 0);
    }

    #[test]
    fn extra_en_columns_null_address() {
        let line = format!("{SAMPLE_EN_LLC}|PIPE");
        let parsed = parse_en(line.as_bytes());
        assert_eq!(parsed.records.len(), 1);
        assert!(!parsed.errors.is_empty());
        let row = &parsed.records[0];
        assert!(row
            .licensee_name
            .as_deref()
            .unwrap()
            .starts_with("N759ZD, LLC"));
        assert_eq!(row.licensee_attention, None);
        assert_eq!(row.licensee_street, None);
        assert_eq!(row.licensee_frn, None);
    }

    #[test]
    fn duplicate_en_l_first_wins() {
        let text = format!("{SAMPLE_EN_LLC}\n{SAMPLE_EN_LLC}");
        let parsed = parse_en(text.as_bytes());
        let (_, _, errors) = join_licenses(&[], &parsed.records, &[]);
        assert_eq!(parsed.records.len(), 2);
        assert!(errors.iter().any(|e| e.error.contains("duplicate EN L")));
    }

    #[test]
    fn bad_date_null_and_error() {
        let line = SAMPLE_HD_LLC.replace("05/01/2024", "99/99/2024");
        let parsed = parse_hd(line.as_bytes());
        assert_eq!(parsed.records[0].grant_date, None);
        assert!(parsed
            .errors
            .iter()
            .any(|e| e.error.contains("invalid FCC date")));
    }
}
