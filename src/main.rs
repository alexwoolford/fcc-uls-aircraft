use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};
use fcc_uls_aircraft::db::{last_run, lookup_licenses, open};
use fcc_uls_aircraft::download::{resolve_user_agent, DEFAULT_ZIP_URL};
use fcc_uls_aircraft::ingest::{ingest, IngestOptions, DEFAULT_MIN_HD_ROWS};

#[derive(Parser)]
#[command(
    name = "fcc-uls-aircraft",
    about = "Weekly FCC ULS Part 87 Active aircraft radio licenses. Labels, not leads.",
    version
)]
struct Cli {
    #[arg(
        long,
        global = true,
        default_value = "data/fcc-uls-aircraft.sqlite",
        env = "FCC_ULS_SQLITE"
    )]
    db: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Download (or read) l_aircr.zip and upsert Active licenses
    Ingest {
        /// Local zip instead of downloading
        #[arg(long)]
        zip: Option<PathBuf>,
        /// After an origin GET, write l_aircr.zip here for later `--zip` reruns
        #[arg(long)]
        cache_dir: Option<PathBuf>,
        /// FCC zip URL used when --zip is omitted
        #[arg(long, default_value = DEFAULT_ZIP_URL)]
        url: String,
        /// Abort if HD.dat has fewer data rows than this (truncated dump guard)
        #[arg(long, default_value_t = DEFAULT_MIN_HD_ROWS)]
        min_hd_rows: usize,
        /// Re-run even if zip_hash matches the last ok ingest
        #[arg(long)]
        force: bool,
        /// Override FCC User-Agent (else FCC_USER_AGENT, else crate default). Never FAA_USER_AGENT.
        #[arg(long, env = "FCC_USER_AGENT")]
        fcc_user_agent: Option<String>,
    },
    /// Last ingest_runs row
    Status,
    /// Licenses by N-number, call sign, FRN, or licensee name (accepts N2860C or 2860C)
    Lookup { query: String },
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    match run() {
        Ok(code) => code,
        Err(err) => {
            tracing::error!("{err:#}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    match cli.command {
        Command::Ingest {
            zip,
            cache_dir,
            url,
            min_hd_rows,
            force,
            fcc_user_agent,
        } => {
            let stats = ingest(&IngestOptions {
                db_path: cli.db,
                zip_path: zip,
                cache_dir,
                zip_url: url,
                min_hd_rows,
                force,
                user_agent: resolve_user_agent(fcc_user_agent.as_deref()),
            })?;
            if stats.status == "failed" {
                Ok(ExitCode::from(1))
            } else {
                Ok(ExitCode::SUCCESS)
            }
        }
        Command::Status => {
            let conn = open(&cli.db)?;
            match last_run(&conn)? {
                None => println!("no ingest_runs"),
                Some(r) => {
                    println!(
                        "as_of_date={} status={} hd_rows={} upserted={} unchanged={} retracted={} parse_errors={} started_at={} finished_at={} zip_hash={}",
                        r.as_of_date,
                        r.status,
                        r.hd_rows.map(|n| n.to_string()).as_deref().unwrap_or("-"),
                        r.active_upserted.map(|n| n.to_string()).as_deref().unwrap_or("-"),
                        r.unchanged_rows.map(|n| n.to_string()).as_deref().unwrap_or("-"),
                        r.retracted.map(|n| n.to_string()).as_deref().unwrap_or("-"),
                        r.parse_errors.map(|n| n.to_string()).as_deref().unwrap_or("-"),
                        r.started_at,
                        r.finished_at,
                        r.zip_hash.as_deref().unwrap_or("-"),
                    );
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Lookup { query } => {
            let conn = open(&cli.db)?;
            let rows = lookup_licenses(&conn, &query)?;
            if rows.is_empty() {
                println!("no licenses for {query}");
            } else {
                for p in rows {
                    let deleted = p
                        .deleted_at
                        .map(|t| format!(" deleted_at={t}"))
                        .unwrap_or_default();
                    println!(
                        "{} {} {} {} {} {}{deleted}",
                        p.uls_id,
                        p.call_sign,
                        p.n_number.as_deref().unwrap_or("-"),
                        p.license_status,
                        p.licensee_frn.as_deref().unwrap_or("-"),
                        p.licensee_name.as_deref().unwrap_or("-"),
                    );
                }
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}
