use std::net::Ipv4Addr;

/// Enumerate non-loopback IPv4 interfaces and return the first suitable address.
/// Falls back to 127.0.0.1 when no LAN interface is found.
pub fn get_local_ip() -> String {
    if let Ok(ifaces) = if_addrs::get_if_addrs() {
        for iface in ifaces {
            if iface.is_loopback() {
                continue;
            }
            if let std::net::IpAddr::V4(v4) = iface.ip() {
                if !v4.is_loopback() && v4 != Ipv4Addr::UNSPECIFIED {
                    return v4.to_string();
                }
            }
        }
    }
    "127.0.0.1".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_local_ip_returns_nonempty() {
        let ip = get_local_ip();
        assert!(!ip.is_empty());
    }

    #[test]
    fn test_get_local_ip_valid_format() {
        let ip = get_local_ip();
        // Fallback is 127.0.0.1 — always a valid IP
        assert!(ip.parse::<std::net::IpAddr>().is_ok());
    }

    #[test]
    fn test_filter_loopback_rejected() {
        // Verify that pure filter rejects loopback
        let ip = get_local_ip();
        if ip == "127.0.0.1" {
            // Accept fallback — means no non-loopback interface was found
        } else {
            let addr: std::net::Ipv4Addr = ip.parse().unwrap();
            assert!(!addr.is_loopback());
        }
    }
}
