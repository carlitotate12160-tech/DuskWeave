//! Namespace/veth preparation and exact owned-resource teardown.
use super::{IP, Names, addresses, nft, run};
use duskweave::Res;
use std::time::Instant;

pub(super) fn prepare(n: &Names, pid: u32, resolver: &str, end: Instant) -> Res<()> {
    attach(n, pid, end)?;
    deny_rules(n, resolver, end)?;
    configure(n, resolver, end)
}
fn attach(n: &Names, pid: u32, end: Instant) -> Res<()> {
    run(IP, &["netns", "attach", &n.netns, &pid.to_string()], end)?;
    run(
        IP,
        &[
            "link", "add", &n.host, "type", "veth", "peer", "name", &n.inner,
        ],
        end,
    )?;
    run(IP, &["link", "set", &n.inner, "netns", &n.netns], end)?;
    run(
        IP,
        &[
            "netns",
            "exec",
            &n.netns,
            "/usr/sbin/sysctl",
            "-qw",
            "net.ipv6.conf.all.disable_ipv6=1",
            "net.ipv6.conf.default.disable_ipv6=1",
        ],
        end,
    )
}
fn deny_rules(n: &Names, resolver: &str, end: Instant) -> Res<()> {
    let (_, inner, subnet) = addresses(n);
    let inside = format!(
        "table inet {t} {{ chain o {{ type filter hook output priority 0; policy drop; ip daddr {resolver} udp dport 53 accept; }}; chain i {{ type filter hook input priority 0; policy drop; ip saddr {resolver} udp sport 53 ct state established accept; }}; }}\n",
        t = n.table
    );
    let outside = format!(
        "table inet {t} {{ set live {{ type ipv4_addr; flags timeout; timeout 2000ms; }}; chain p {{ type nat hook postrouting priority 100; ip saddr {subnet} ip daddr {resolver} udp dport 53 masquerade; }}; chain f {{ type filter hook forward priority -50; iifname \"{v}\" ip saddr {inner} ip daddr {resolver} udp dport 53 ip saddr @live accept; iifname \"{v}\" drop; oifname \"{v}\" ip saddr {resolver} udp sport 53 ip daddr @live ct state established accept; oifname \"{v}\" drop; }}; chain i {{ type filter hook input priority -50; iifname \"{v}\" drop; }}; }}\n",
        t = n.table,
        v = n.host
    );
    nft(Some(&n.netns), &inside, end)?;
    nft(None, &outside, end)
}
fn configure(n: &Names, resolver: &str, end: Instant) -> Res<()> {
    let (host, inner, _) = addresses(n);
    run(
        IP,
        &["addr", "add", &format!("{host}/30"), "dev", &n.host],
        end,
    )?;
    run(
        IP,
        &[
            "netns",
            "exec",
            &n.netns,
            IP,
            "addr",
            "add",
            &format!("{inner}/30"),
            "dev",
            &n.inner,
        ],
        end,
    )?;
    // A connected gateway route is unavailable while the interface is down.
    // Both default-deny rule sets are installed before enabling either link.
    run(IP, &["link", "set", &n.host, "up"], end)?;
    run(
        IP,
        &["netns", "exec", &n.netns, IP, "link", "set", &n.inner, "up"],
        end,
    )?;
    run(
        IP,
        &[
            "netns",
            "exec",
            &n.netns,
            IP,
            "route",
            "add",
            &format!("{resolver}/32"),
            "via",
            &host,
        ],
        end,
    )
}

pub(super) fn absent(n: &Names) -> bool {
    !std::path::Path::new(&format!("/run/netns/{}", n.netns)).exists()
        && !std::path::Path::new(&format!("/sys/class/net/{}", n.host)).exists()
}
pub(super) fn destroy(n: &Names, end: Instant) -> bool {
    let link = run(IP, &["link", "del", &n.host], end).is_ok()
        || !std::path::Path::new(&format!("/sys/class/net/{}", n.host)).exists();
    let ns = run(IP, &["netns", "del", &n.netns], end).is_ok()
        || !std::path::Path::new(&format!("/run/netns/{}", n.netns)).exists();
    let table = nft(None, &format!("delete table inet {}\n", n.table), end).is_ok();
    link && ns && table
}
