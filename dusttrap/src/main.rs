//! CLI demo DustTrap.
//!
//! Contoh:
//!   dusttrap scan <dir>            — daftar kandidat debu (readonly)
//!   dusttrap quarantine <dir>      — jebak debu ke ./.dusttrap-quarantine
//!   dusttrap report                — ringkasan (demo in-memory)
//!   dusttrap purge <ticket>        — SELALU ditolak (fitur dibekukan)

use dusttrap::{DustPolicy, DustTrap, PurgeTicket, ScanConfig};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");

    match cmd {
        "scan" | "quarantine" => {
            let root = PathBuf::from(args.get(2).cloned().unwrap_or_else(|| ".".into()));
            let mut trap = DustTrap::new(DustPolicy::standard(
                root.join(".dusttrap-quarantine"),
            ));
            let cfg = ScanConfig::new(root);
            let entries = match trap.scan(&cfg) {
                Ok(e) => e,
                Err(err) => {
                    eprintln!("GAGAL: {err}");
                    return ExitCode::FAILURE;
                }
            };
            println!("Ditemukan {} kandidat debu:", entries.len());
            for e in &entries {
                let kind = trap.classify(e);
                println!("  [{}] {} ({} B, umur {} s)", kind.label(), e.path.display(), e.size_bytes, e.age_secs);
            }
            if cmd == "quarantine" {
                for e in &entries {
                    match trap.quarantine(e) {
                        Ok(rec) => println!("  DIJEBAK -> tiket #{} @ {}", rec.ticket_id, rec.trapped_at.display()),
                        Err(err) => println!("  DILEWAT : {err}"),
                    }
                }
            }
            print!("{}", trap.report());
            ExitCode::SUCCESS
        }
        "purge" => {
            let id: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            let trap = DustTrap::new(DustPolicy::standard(PathBuf::from(".dusttrap-quarantine")));
            let ticket = PurgeTicket::for_audit(id, PathBuf::from(format!(".dusttrap-quarantine/t{id:06}-*")));
            // purge butuh &mut self untuk audit; pada CLI kita cukup tunjukkan penolakannya:
            let err = ticket.refuse();
            let _ = trap; // instance hanya untuk ilustrasi
            eprintln!("TOLAK: {err}");
            ExitCode::FAILURE
        }
        _ => {
            println!("DustTrap — perangkap debu (penghapusan file DINONAKTIFKAN)\n");
            println!("Penggunaan:");
            println!("  dusttrap scan <dir>          : daftar kandidat debu");
            println!("  dusttrap quarantine <dir>    : karantina debu (non-destruktif)");
            println!("  dusttrap purge <tiket>       : selalu ditolak oleh desain");
            ExitCode::SUCCESS
        }
    }
}
