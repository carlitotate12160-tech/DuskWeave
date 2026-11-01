//! Fixed root boundary. No command arguments, environment overrides or ordinary raw output.
#![forbid(unsafe_code)]
#[cfg(target_os = "linux")]
mod admission;
#[cfg(target_os = "linux")]
mod attempt;
#[cfg(target_os = "linux")]
mod control;
#[cfg(target_os = "linux")]
mod docker;
#[cfg(target_os = "linux")]
mod network;
#[cfg(target_os = "linux")]
mod qualification;
#[cfg(target_os = "linux")]
mod relay;

fn main() {
    #[cfg(target_os = "linux")]
    let accepted = admission::serve().is_ok();
    #[cfg(not(target_os = "linux"))]
    let accepted = false;
    if !accepted {
        std::process::exit(1)
    }
}
