//! Host nftables egress lease management. One `inet` table per attempt holds
//! a timeout-flagged endpoint set; elements carry an independent lifetime so
//! a dead launcher cannot leave egress open beyond the lease bound.
use std::io::Write;
use std::process::{Command, Stdio};

pub struct EgressTable {
    pub table: String,
    pub set: String,
}

/// Non-owning handle for the lease set; renewal/revoke only. The owning
/// EgressTable destroys the whole table exactly once in teardown.
#[derive(Clone)]
pub struct LeaseRef {
    pub table: String,
    pub set: String,
}

impl EgressTable {
    pub fn lease(&self) -> LeaseRef {
        LeaseRef {
            table: self.table.clone(),
            set: self.set.clone(),
        }
    }
}

impl LeaseRef {
    /// Add or refresh the admitted endpoint element with the lease timeout.
    pub fn renew(&self, ipv4: &str, port: u16, lease_ms: u64) -> Result<(), &'static str> {
        nft_stdin(&format!(
            "add element inet {} {} {{ {} . {} timeout {}ms }}",
            self.table, self.set, ipv4, port, lease_ms
        ))
        .map(|_| ())
    }

    /// Remove the admitted element: egress closes for new outbound packets.
    pub fn revoke(&self, ipv4: &str, port: u16) {
        let _ = nft_stdin(&format!(
            "delete element inet {} {} {{ {} . {} }}",
            self.table, self.set, ipv4, port
        ));
    }
}

impl EgressTable {
    /// Create the attempt table: the verdict chain default-denies capture
    /// egress; only the live lease element admits (ip, udp port) per packet,
    /// with no conntrack-established exemption on the outbound direction.
    pub fn create(tag: &str, bridge: &str) -> Result<Self, &'static str> {
        let table = format!("dwatt{tag}");
        let script = format!(
            "add table inet {table}\n\
             add set inet {table} lease {{ type ipv4_addr . inet_service; flags timeout; }}\n\
             add chain inet {table} verdict\n\
             add rule inet {table} verdict ip daddr . udp dport @lease accept\n\
             add rule inet {table} verdict counter drop\n\
             add chain inet {table} outbound {{ type filter hook forward priority -10; \
             policy accept; }}\n\
             add chain inet {table} inbound_host {{ type filter hook input priority -10; \
             policy accept; }}\n\
             add chain inet {table} replies {{ type filter hook forward priority -10; \
             policy accept; }}\n\
             add rule inet {table} outbound iifname \"{bridge}\" jump verdict\n\
             add rule inet {table} inbound_host iifname \"{bridge}\" jump verdict\n\
             add rule inet {table} replies oifname \"{bridge}\" ct state established accept\n"
        );
        nft_stdin(&script).map(|_| Self {
            set: "lease".into(),
            table,
        })
    }

    /// Drop counter on the verdict chain, for qualification evidence.
    pub fn drop_counter(&self) -> Option<u64> {
        let out = Command::new("nft")
            .args(["list", "chain", "inet", &self.table, "verdict"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let text = String::from_utf8(out.stdout).ok()?;
        text.split_whitespace()
            .collect::<Vec<_>>()
            .windows(2)
            .find(|w| w[0] == "packets")
            .and_then(|w| w[1].parse().ok())
    }

    /// Destroy the whole attempt table; idempotent by name.
    pub fn destroy(self) {
        let _ = nft_stdin(&format!("delete table inet {}", self.table));
    }
}

fn nft_stdin(script: &str) -> Result<String, &'static str> {
    let mut child = Command::new("nft")
        .args(["-f", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "nft_invoke_failed")?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(script.as_bytes());
    }
    child
        .wait()
        .map_err(|_| "nft_command_failed")
        .and_then(|s| {
            if s.success() {
                Ok(String::new())
            } else {
                Err("nft_command_failed")
            }
        })
}
