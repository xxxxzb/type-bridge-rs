use std::net::UdpSocket;

pub fn get_local_ip() -> String {
    match UdpSocket::bind("0.0.0.0:0") {
        Ok(socket) => match socket.connect("8.8.8.8:80") {
            Ok(()) => match socket.local_addr() {
                Ok(addr) => addr.ip().to_string(),
                Err(_) => "127.0.0.1".into(),
            },
            Err(_) => "127.0.0.1".into(),
        },
        Err(_) => "127.0.0.1".into(),
    }
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
        // Should be a valid IPv4 or IPv6 address
        let parts: Vec<&str> = ip.split('.').collect();
        if parts.len() == 4 {
            for part in parts {
                let n: u8 = part.parse().expect("valid u8 octet");
                assert!(n > 0 || part == "0");
            }
        }
        // Fallback is 127.0.0.1
        assert!(ip.parse::<std::net::IpAddr>().is_ok());
    }
}
