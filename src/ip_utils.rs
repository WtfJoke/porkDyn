use std::net::IpAddr;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub enum IpType {
    V4,
    V6,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
pub enum RecordType {
    A,
    AAAA,
}

impl RecordType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecordType::A => "A",
            RecordType::AAAA => "AAAA",
        }
    }
}

impl From<IpType> for RecordType {
    fn from(ip_type: IpType) -> Self {
        match ip_type {
            IpType::V4 => RecordType::A,
            IpType::V6 => RecordType::AAAA,
        }
    }
}

/// Validates and determines the type of an IP address
pub fn validate_and_classify_ip(ip_str: &str) -> Result<IpType, String> {
    match IpAddr::from_str(ip_str) {
        Ok(IpAddr::V4(_)) => Ok(IpType::V4),
        Ok(IpAddr::V6(_)) => Ok(IpType::V6),
        Err(_) => Err(format!("Invalid IP address: {}", ip_str)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_ipv4() {
        assert_eq!(validate_and_classify_ip("192.168.1.1"), Ok(IpType::V4));
        assert_eq!(validate_and_classify_ip("127.0.0.1"), Ok(IpType::V4));
        assert_eq!(validate_and_classify_ip("0.0.0.0"), Ok(IpType::V4));
        assert_eq!(validate_and_classify_ip("255.255.255.255"), Ok(IpType::V4));
    }

    #[test]
    fn test_valid_ipv6() {
        assert_eq!(validate_and_classify_ip("::1"), Ok(IpType::V6));
        assert_eq!(validate_and_classify_ip("2001:db8::1"), Ok(IpType::V6));
        assert_eq!(validate_and_classify_ip("fe80::1"), Ok(IpType::V6));
        assert_eq!(
            validate_and_classify_ip("2001:0db8:85a3:0000:0000:8a2e:0370:7334"),
            Ok(IpType::V6)
        );
        assert_eq!(
            validate_and_classify_ip("2001:db8:85a3::8a2e:370:7334"),
            Ok(IpType::V6)
        );
    }

    #[test]
    fn test_invalid_ip() {
        assert!(validate_and_classify_ip("256.1.1.1").is_err());
        assert!(validate_and_classify_ip("192.168.1").is_err());
        assert!(validate_and_classify_ip("not_an_ip").is_err());
        assert!(validate_and_classify_ip("").is_err());
        assert!(validate_and_classify_ip("2001:db8::g1").is_err());
        // Zone identifiers are not supported in std::net parsing
        assert!(validate_and_classify_ip("fe80::1%lo0").is_err());
    }

    #[test]
    fn test_record_type_conversion() {
        assert_eq!(RecordType::from(IpType::V4), RecordType::A);
        assert_eq!(RecordType::from(IpType::V6), RecordType::AAAA);
    }

    #[test]
    fn test_record_type_as_str() {
        assert_eq!(RecordType::A.as_str(), "A");
        assert_eq!(RecordType::AAAA.as_str(), "AAAA");
    }
}
