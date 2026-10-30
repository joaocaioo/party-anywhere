//! Qual endereço vai no QR. Um PC costuma ter várias placas de rede (Wi-Fi, Ethernet, Hyper-V,
//! WSL, VPN, Docker); só uma é a que o celular alcança. Pontuamos cada endereço e mostramos o melhor.

use std::net::Ipv4Addr;

#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub ip: Ipv4Addr,
    pub interface: String,
    pub score: i32,
}

/// Endereços IPv4 da máquina, do mais provável para o menos provável. Pode vir vazio.
pub fn lan_candidates() -> Vec<Candidate> {
    let mut out: Vec<Candidate> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|i| match i.addr {
            if_addrs::IfAddr::V4(v4) => Some((i.name, v4.ip)),
            _ => None,
        })
        .filter_map(|(name, ip)| score(&name, ip).map(|score| Candidate { ip, interface: name, score }))
        .collect();
    out.sort_by(|a, b| b.score.cmp(&a.score).then(a.ip.cmp(&b.ip)));
    out
}

/// Nota de um endereço; `None` = nunca serve (loopback, link-local, etc.).
pub fn score(interface: &str, ip: Ipv4Addr) -> Option<i32> {
    if ip.is_loopback() || ip.is_link_local() || ip.is_unspecified() || ip.is_multicast() {
        return None;
    }
    let o = ip.octets();
    let mut s = if o[0] == 192 && o[1] == 168 {
        100
    } else if o[0] == 10 {
        70
    } else if o[0] == 172 && (16..=31).contains(&o[1]) {
        // faixa preferida de Hyper-V, WSL e Docker
        40
    } else if o[0] == 100 && (64..=127).contains(&o[1]) {
        // CGNAT: Tailscale e afins
        5
    } else {
        10
    };
    let name = interface.to_ascii_lowercase();
    const VIRTUAL: [&str; 12] = [
        "vethernet", "hyper-v", "wsl", "virtualbox", "vmware", "docker", "loopback", "tailscale", "zerotier",
        "vpn", "tap", "bluetooth",
    ];
    if VIRTUAL.iter().any(|v| name.contains(v)) {
        s -= 60;
    }
    if name.contains("wi-fi") || name.contains("wifi") || name.contains("wlan") || name.contains("wireless") {
        s += 15;
    } else if name.contains("ethernet") {
        s += 10;
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_wifi_beats_hyperv() {
        let wifi = score("Wi-Fi", Ipv4Addr::new(192, 168, 15, 66)).unwrap();
        let hyperv = score("vEthernet (WSL (Hyper-V firewall))", Ipv4Addr::new(172, 24, 160, 1)).unwrap();
        let eth = score("Ethernet", Ipv4Addr::new(10, 0, 0, 5)).unwrap();
        assert!(wifi > eth && eth > hyperv);
    }

    #[test]
    fn useless_addresses_are_dropped() {
        assert_eq!(score("Loopback", Ipv4Addr::LOCALHOST), None);
        assert_eq!(score("Wi-Fi", Ipv4Addr::new(169, 254, 3, 4)), None);
    }
}
