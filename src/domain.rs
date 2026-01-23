use crate::error::DomainError;

#[derive(Debug, Clone)]
pub struct Domain {
    domain_name: String,    // e.g., "example.org"
    subdomain: String,      // e.g., "api"
    qualified_name: String, // e.g., "api.example.org"
}

impl Domain {
    pub fn new(qualified_name: &str) -> Result<Self, DomainError> {
        let parts: Vec<&str> = qualified_name.split('.').collect();

        if parts.len() < 3 {
            return Err(DomainError::DomainValidationError(
                "Domain must have at least 3 parts (e.g., sub.example.com)".to_string(),
            ));
        }

        if parts.iter().any(|part| part.is_empty()) {
            return Err(DomainError::DomainValidationError(
                "Domain contains empty parts".to_string(),
            ));
        }

        let domain_name = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
        let subdomain = parts[..parts.len() - 2].join(".");

        Ok(Self {
            domain_name,
            subdomain,
            qualified_name: qualified_name.to_string(),
        })
    }

    pub fn domain_name(&self) -> &str {
        &self.domain_name
    }

    pub fn subdomain(&self) -> &str {
        &self.subdomain
    }

    pub fn qualified_name(&self) -> &str {
        &self.qualified_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_valid_domain() {
        let qualified_name_input = "api.example.com";
        let domain = Domain::new(qualified_name_input);
        assert!(
            domain.is_ok(),
            "Domain: {} should be valid",
            qualified_name_input
        );
        let domain = domain.unwrap();
        assert_eq!(domain.domain_name(), "example.com");
        assert_eq!(domain.subdomain(), "api");
        assert_eq!(domain.qualified_name(), "api.example.com");
    }

    #[test]
    fn test_valid_domains() {
        let valid_domains = [
            "api.example.com",
            "very-very-very-very-very-very-very-long-subdomain.example.com",
        ];

        for domain in valid_domains {
            let result = Domain::new(domain);
            assert!(result.is_ok(), "Domain: {}", domain);
            let domain = result.unwrap();
            assert_eq!(domain.domain_name(), "example.com");
        }
    }

    #[test]
    fn test_invalid_domains() {
        let invalid_domains = [
            "example.com",
            "a..example.com",
            "api.example",
            "api@invalid.com",
        ];

        for domain in invalid_domains {
            let result = Domain::new(domain);
            assert!(result.is_err(), "Domain: {}", domain);
        }
    }
}
