//! Two network interfaces on one subnet (a bridge plus Wi-Fi, say) break the broadcast DJ
//! protocols: StageLinQ and Pro DJ Link can announce on one address and route TCP through the
//! other. Ported from `network-check.ts`; the Bridge warns about it at start.

use std::net::IpAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    V4,
    V6,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfaceAddr {
    pub family: Family,
    pub address: String,
    pub netmask: String,
    /// Loopback, as Node's `internal`.
    pub internal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubnetConflict {
    /// "network/prefix", e.g. "192.168.1.0/24".
    pub subnet: String,
    /// (interface name, address), in the order the system lists them.
    pub interfaces: Vec<(String, String)>,
}

/// "192.168.1.0/24" from an address and netmask, as the TypeScript computes it.
pub fn calculate_subnet(address: &str, netmask: &str) -> String {
    let octets = |s: &str| {
        s.split('.')
            .map(|p| p.parse::<u32>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    let ip = octets(address);
    let mask = octets(netmask);
    let network: Vec<String> = ip
        .iter()
        .enumerate()
        .map(|(i, o)| (o & mask.get(i).copied().unwrap_or(0)).to_string())
        .collect();
    let prefix: u32 = mask.iter().map(|o| o.count_ones()).sum();
    format!("{}/{prefix}", network.join("."))
}

/// The pure check: every IPv4, non-loopback address grouped by subnet; groups of two or more.
pub fn detect_subnet_conflicts_from(
    interfaces: &[(String, Vec<IfaceAddr>)],
) -> Vec<SubnetConflict> {
    let mut groups: Vec<SubnetConflict> = Vec::new();
    for (name, addrs) in interfaces {
        for addr in addrs
            .iter()
            .filter(|a| a.family == Family::V4 && !a.internal)
        {
            let subnet = calculate_subnet(&addr.address, &addr.netmask);
            let entry = (name.clone(), addr.address.clone());
            match groups.iter_mut().find(|g| g.subnet == subnet) {
                Some(g) => g.interfaces.push(entry),
                None => groups.push(SubnetConflict {
                    subnet,
                    interfaces: vec![entry],
                }),
            }
        }
    }
    groups.retain(|g| g.interfaces.len() > 1);
    groups
}

/// The live check, from this machine's interfaces. An unreadable list means no warning.
pub fn detect_subnet_conflicts() -> Vec<SubnetConflict> {
    let Ok(list) = if_addrs::get_if_addrs() else {
        return Vec::new();
    };
    let mut by_name: Vec<(String, Vec<IfaceAddr>)> = Vec::new();
    for iface in list {
        let internal = iface.is_loopback();
        let addr = match iface.addr {
            if_addrs::IfAddr::V4(v4) => IfaceAddr {
                family: Family::V4,
                address: v4.ip.to_string(),
                netmask: v4.netmask.to_string(),
                internal,
            },
            if_addrs::IfAddr::V6(v6) => IfaceAddr {
                family: Family::V6,
                address: IpAddr::V6(v6.ip).to_string(),
                netmask: v6.netmask.to_string(),
                internal,
            },
        };
        match by_name.iter_mut().find(|(n, _)| *n == iface.name) {
            Some((_, addrs)) => addrs.push(addr),
            None => by_name.push((iface.name, vec![addr])),
        }
    }
    detect_subnet_conflicts_from(&by_name)
}

pub fn format_conflict_warnings(conflicts: &[SubnetConflict]) -> Vec<String> {
    conflicts
        .iter()
        .map(|c| {
            let list: Vec<String> = c.interfaces.iter().map(|(n, a)| format!("{n} ({a})")).collect();
            format!(
                "Multiple interfaces on subnet {}: {}. This may cause DJ equipment connection failures — consider disabling one interface.",
                c.subnet,
                list.join(", ")
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v4(address: &str, netmask: &str) -> IfaceAddr {
        IfaceAddr {
            family: Family::V4,
            address: address.into(),
            netmask: netmask.into(),
            internal: false,
        }
    }
    fn iface(name: &str, addrs: Vec<IfaceAddr>) -> (String, Vec<IfaceAddr>) {
        (name.into(), addrs)
    }
    const C: &str = "255.255.255.0";

    #[test]
    fn no_conflicts() {
        let got = detect_subnet_conflicts_from(&[
            iface("eth0", vec![v4("192.168.1.10", C)]),
            iface("wlan0", vec![v4("10.0.0.5", C)]),
        ]);
        assert!(got.is_empty());
    }

    #[test]
    fn two_interfaces_on_one_slash_24() {
        let got = detect_subnet_conflicts_from(&[
            iface("Ethernet", vec![v4("192.168.1.10", C)]),
            iface("Wi-Fi", vec![v4("192.168.1.20", C)]),
        ]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].subnet, "192.168.1.0/24");
        assert_eq!(
            got[0].interfaces,
            vec![
                ("Ethernet".into(), "192.168.1.10".into()),
                ("Wi-Fi".into(), "192.168.1.20".into())
            ]
        );
    }

    #[test]
    fn slash_16_and_three_interfaces() {
        let m = "255.255.0.0";
        let got = detect_subnet_conflicts_from(&[
            iface("a", vec![v4("172.16.1.1", m)]),
            iface("b", vec![v4("172.16.200.9", m)]),
            iface("c", vec![v4("172.16.3.3", m)]),
        ]);
        assert_eq!(got[0].subnet, "172.16.0.0/16");
        assert_eq!(got[0].interfaces.len(), 3);
    }

    #[test]
    fn ignores_loopback_ipv6_and_empty_interfaces() {
        let mut lo = v4("127.0.0.1", "255.0.0.0");
        lo.internal = true;
        let mut lo2 = lo.clone();
        lo2.address = "127.0.0.2".into();
        let six = |a: &str| IfaceAddr {
            family: Family::V6,
            address: a.into(),
            netmask: "ffff:ffff:ffff:ffff::".into(),
            internal: false,
        };
        let got = detect_subnet_conflicts_from(&[
            iface("lo", vec![lo]),
            iface("lo2", vec![lo2]),
            iface("e1", vec![six("fe80::1")]),
            iface("e2", vec![six("fe80::2")]),
            iface("empty", vec![]),
        ]);
        assert!(got.is_empty());
        assert!(detect_subnet_conflicts_from(&[]).is_empty());
    }

    #[test]
    fn separate_subnets_are_reported_independently() {
        let got = detect_subnet_conflicts_from(&[
            iface("a", vec![v4("192.168.1.1", C), v4("10.0.0.1", C)]),
            iface("b", vec![v4("192.168.1.2", C)]),
            iface("c", vec![v4("10.0.0.2", C)]),
            iface("d", vec![v4("172.20.0.1", C)]),
        ]);
        let subnets: Vec<&str> = got.iter().map(|g| g.subnet.as_str()).collect();
        assert_eq!(subnets, vec!["192.168.1.0/24", "10.0.0.0/24"]);
    }

    #[test]
    fn subnet_math_matches_the_typescript() {
        assert_eq!(calculate_subnet("192.168.1.77", C), "192.168.1.0/24");
        assert_eq!(
            calculate_subnet("10.1.2.3", "255.255.255.252"),
            "10.1.2.0/30"
        );
        assert_eq!(calculate_subnet("bad", C), "0/24");
    }

    #[test]
    fn live_check_runs() {
        let _ = detect_subnet_conflicts();
    }

    #[test]
    fn formats_warnings() {
        let got = format_conflict_warnings(&detect_subnet_conflicts_from(&[
            iface("eth0", vec![v4("192.168.1.10", C)]),
            iface("wlan0", vec![v4("192.168.1.20", C)]),
        ]));
        assert_eq!(
            got,
            vec![
                "Multiple interfaces on subnet 192.168.1.0/24: eth0 (192.168.1.10), wlan0 (192.168.1.20). This may cause DJ equipment connection failures — consider disabling one interface."
            ]
        );
        assert!(format_conflict_warnings(&[]).is_empty());
    }
}
