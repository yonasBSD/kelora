use ipnet::IpNet;
use rhai::{Engine, EvalAltResult, ImmutableString};
use std::net::IpAddr;
use std::str::FromStr;

/// Check if a string is a valid IPv4 address
/// Returns false for invalid input
/// Usage: is_ipv4(e.client_ip)
pub fn is_ipv4(ip: ImmutableString) -> bool {
    if let Ok(addr) = IpAddr::from_str(ip.as_str()) {
        matches!(addr, IpAddr::V4(_))
    } else {
        false
    }
}

/// Check if a string is a valid IPv6 address
/// Returns false for invalid input
/// Usage: is_ipv6(e.client_ip)
pub fn is_ipv6(ip: ImmutableString) -> bool {
    if let Ok(addr) = IpAddr::from_str(ip.as_str()) {
        matches!(addr, IpAddr::V6(_))
    } else {
        false
    }
}

/// Check if an IP address is in a CIDR network
/// Returns error for invalid CIDR format
/// Usage: is_in_cidr(e.client_ip, "10.0.0.0/8")
pub fn is_in_cidr(ip: ImmutableString, cidr: ImmutableString) -> Result<bool, Box<EvalAltResult>> {
    // Parse the IP address
    let addr = IpAddr::from_str(ip.as_str()).map_err(|_| {
        EvalAltResult::ErrorRuntime(
            format!("Invalid IP address: '{}'", ip).into(),
            rhai::Position::NONE,
        )
    })?;

    // Parse the CIDR network
    let network = IpNet::from_str(cidr.as_str()).map_err(|_| {
        EvalAltResult::ErrorRuntime(
            format!("Invalid CIDR format: '{}'", cidr).into(),
            rhai::Position::NONE,
        )
    })?;

    // Check if the address is in the network
    Ok(network.contains(&addr))
}

/// Register network functions with the Rhai engine
pub fn register_functions(engine: &mut Engine) {
    // IPv4 validation
    engine.register_fn("is_ipv4", |ip: ImmutableString| -> bool { is_ipv4(ip) });

    // IPv6 validation
    engine.register_fn("is_ipv6", |ip: ImmutableString| -> bool { is_ipv6(ip) });

    // CIDR matching
    engine.register_fn(
        "is_in_cidr",
        |ip: ImmutableString, cidr: ImmutableString| -> Result<bool, Box<EvalAltResult>> {
            is_in_cidr(ip, cidr)
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_ipv4() {
        assert!(is_ipv4("192.168.1.1".into()));
        assert!(is_ipv4("10.0.0.1".into()));
        assert!(is_ipv4("0.0.0.0".into()));
        assert!(is_ipv4("255.255.255.255".into()));

        // Invalid cases
        assert!(!is_ipv4("not-an-ip".into()));
        assert!(!is_ipv4("256.1.1.1".into()));
        assert!(!is_ipv4("".into()));
        assert!(!is_ipv4("2001:db8::1".into())); // IPv6
    }

    #[test]
    fn test_is_ipv6() {
        assert!(is_ipv6("2001:db8::1".into()));
        assert!(is_ipv6("::1".into()));
        assert!(is_ipv6("fe80::1".into()));
        assert!(is_ipv6("2001:0db8:0000:0000:0000:0000:0000:0001".into()));

        // Invalid cases
        assert!(!is_ipv6("not-an-ip".into()));
        assert!(!is_ipv6("".into()));
        assert!(!is_ipv6("192.168.1.1".into())); // IPv4
    }

    #[test]
    fn test_is_in_cidr_ipv4() {
        // Matching cases
        assert!(is_in_cidr("192.168.1.1".into(), "192.168.1.0/24".into()).unwrap());
        assert!(is_in_cidr("10.0.0.1".into(), "10.0.0.0/8".into()).unwrap());
        assert!(is_in_cidr("172.16.5.4".into(), "172.16.0.0/16".into()).unwrap());

        // Non-matching cases
        assert!(!is_in_cidr("192.168.2.1".into(), "192.168.1.0/24".into()).unwrap());
        assert!(!is_in_cidr("11.0.0.1".into(), "10.0.0.0/8".into()).unwrap());

        // Edge cases
        assert!(is_in_cidr("192.168.1.1".into(), "192.168.1.1/32".into()).unwrap());
        assert!(is_in_cidr("0.0.0.0".into(), "0.0.0.0/0".into()).unwrap()); // Match all
    }

    #[test]
    fn test_is_in_cidr_ipv6() {
        // Matching cases
        assert!(is_in_cidr("2001:db8::1".into(), "2001:db8::/32".into()).unwrap());
        assert!(is_in_cidr("fe80::1".into(), "fe80::/10".into()).unwrap());

        // Non-matching cases
        assert!(!is_in_cidr("2001:db9::1".into(), "2001:db8::/32".into()).unwrap());
    }

    #[test]
    fn test_is_in_cidr_errors() {
        // Invalid IP
        assert!(is_in_cidr("not-an-ip".into(), "192.168.1.0/24".into()).is_err());

        // Invalid CIDR
        assert!(is_in_cidr("192.168.1.1".into(), "not-a-cidr".into()).is_err());
        assert!(is_in_cidr("192.168.1.1".into(), "192.168.1.0/33".into()).is_err());
    }

    #[test]
    fn test_is_in_cidr_mixed() {
        // IPv4 address against IPv6 network (no match)
        assert!(!is_in_cidr("192.168.1.1".into(), "2001:db8::/32".into()).unwrap());

        // IPv6 address against IPv4 network (no match)
        assert!(!is_in_cidr("2001:db8::1".into(), "192.168.1.0/24".into()).unwrap());
    }
}
