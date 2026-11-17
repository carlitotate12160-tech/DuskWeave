//! Opt-in operator checks, separate from campaign processes. Standard library only.
//! The link-check CI consumer can compile this entrypoint directly with rustc,
//! without compiling production dependencies or installing a Python environment.
#![forbid(unsafe_code)]
mod links;
mod markdown;

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let parts: Vec<&str> = args.iter().map(String::as_str).collect();
    let ["check-links", "--root", root] = parts.as_slice() else {
        eprintln!("usage: dw-qualify check-links --root DIRECTORY");
        return ExitCode::from(2);
    };
    match links::check(Path::new(root)) {
        Ok(report) => {
            println!("OK relative links: {}", report.ok);
            println!("Broken relative links: {}", report.broken.len());
            println!("Broken anchors: {}", report.bad_anchor.len());
            println!("External links (not fetched): {}", report.external);
            for issue in report.broken.iter().chain(&report.bad_anchor) {
                println!("  {issue}");
            }
            ExitCode::from(u8::from(
                !report.broken.is_empty() || !report.bad_anchor.is_empty(),
            ))
        }
        Err(_) => {
            eprintln!("link_check_io_failed");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/dw_qualify/links.rs"]
mod tests;
