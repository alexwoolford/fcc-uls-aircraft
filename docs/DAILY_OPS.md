# Daily ops (Oracle)

Oneshot + timer. The OS is the scheduler. Do not add an in-process cron.

Operator logs: `tracing` on stderr → journald (`SyslogIdentifier=fcc-uls-aircraft-ingest`). Default `RUST_LOG=info`.

`ingest_runs` is capturable domain telemetry. Query it in mosaic.

## Layout

| | |
| --- | --- |
| Prefix | `/opt/fcc-uls-aircraft` |
| State | `/var/lib/fcc-uls-aircraft/fcc-uls-aircraft.sqlite` |
| User | `fcc` |
| Env | `/opt/fcc-uls-aircraft/etc/fcc-uls-aircraft.env` (`chmod 600`) |
| Timer | `fcc-uls-aircraft-ingest.timer` **Sun 12:00 UTC** + 15m jitter, `Persistent=true` |
| Oneshot timeout | `TimeoutStartSec=1h` |

No published `current/`. Do not copy a laptop sqlite onto the host. Do not send `FAA_USER_AGENT` to FCC.

## Install

```bash
cargo build --release
sudo ./deploy/install.sh
```

`install.sh` enables the timer **without** `--now`. First run: `sudo systemctl start fcc-uls-aircraft-ingest.service`.

## Timer failed

1. `systemctl list-failed --no-pager`
2. `journalctl -u fcc-uls-aircraft-ingest.service -n 80 --no-pager`
3. `fcc-uls-aircraft --db /var/lib/fcc-uls-aircraft/fcc-uls-aircraft.sqlite status`
4. Re-run: `sudo systemctl start fcc-uls-aircraft-ingest.service`

Do not hand-edit sqlite.

`status=skipped` (same `zip_hash` as last ok) is success (exit 0). `status=failed` is a truncated dump, HTTP error, or apply error (exit 1). Floor is `--min-hd-rows` 100000.

Default User-Agent: `fcc-uls-aircraft/0.1 (+https://github.com/alexwoolford/fcc-uls-aircraft)`. Override `FCC_USER_AGENT`. HTTP 503 retries four times with backoff.
