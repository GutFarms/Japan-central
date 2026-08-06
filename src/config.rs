//! Post-boot mining identity: wallet address, pool password, stratum location.

use core::fmt;
use heapless::String;

pub const ADDRESS_MAX: usize = 96;
pub const PASSWORD_MAX: usize = 64;
pub const STRATUM_MAX: usize = 96;

pub type AddressString = String<ADDRESS_MAX>;
pub type PasswordString = String<PASSWORD_MAX>;
pub type StratumString = String<STRATUM_MAX>;

/// Which credential field is being collected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetupField {
    Address,
    Password,
    Stratum,
}

impl SetupField {
    pub const ALL: [SetupField; 3] = [
        SetupField::Address,
        SetupField::Password,
        SetupField::Stratum,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SetupField::Address => "address",
            SetupField::Password => "password",
            SetupField::Stratum => "stratum",
        }
    }

    pub fn prompt(self) -> &'static str {
        match self {
            SetupField::Address => "Wallet address (worker name OK)",
            SetupField::Password => "Pool password (often 'x')",
            SetupField::Stratum => "Stratum location (host:port)",
        }
    }

    pub fn next(self) -> Option<SetupField> {
        match self {
            SetupField::Address => Some(SetupField::Password),
            SetupField::Password => Some(SetupField::Stratum),
            SetupField::Stratum => None,
        }
    }
}

/// Pool / worker credentials entered after boot.
#[derive(Clone, Debug, Default)]
pub struct PoolConfig {
    pub address: AddressString,
    pub password: PasswordString,
    pub stratum: StratumString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigError {
    Empty,
    TooLong,
    InvalidChar,
    UnknownField,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Empty => write!(f, "value cannot be empty"),
            ConfigError::TooLong => write!(f, "value too long"),
            ConfigError::InvalidChar => write!(f, "invalid character"),
            ConfigError::UnknownField => write!(f, "unknown field"),
        }
    }
}

impl PoolConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_complete(&self) -> bool {
        !self.address.is_empty() && !self.password.is_empty() && !self.stratum.is_empty()
    }

    pub fn get(&self, field: SetupField) -> &str {
        match field {
            SetupField::Address => self.address.as_str(),
            SetupField::Password => self.password.as_str(),
            SetupField::Stratum => self.stratum.as_str(),
        }
    }

    pub fn set(&mut self, field: SetupField, raw: &str) -> Result<(), ConfigError> {
        let value = normalize_value(raw)?;
        match field {
            SetupField::Address => {
                self.address.clear();
                self.address
                    .push_str(value)
                    .map_err(|_| ConfigError::TooLong)?;
            }
            SetupField::Password => {
                self.password.clear();
                self.password
                    .push_str(value)
                    .map_err(|_| ConfigError::TooLong)?;
            }
            SetupField::Stratum => {
                validate_stratum(value)?;
                self.stratum.clear();
                self.stratum
                    .push_str(value)
                    .map_err(|_| ConfigError::TooLong)?;
            }
        }
        Ok(())
    }

    /// Parse `address …` / `password …` / `stratum …` assignment lines.
    pub fn parse_assignment(line: &str) -> Result<(SetupField, &str), ConfigError> {
        let line = line.trim();
        for field in SetupField::ALL {
            let prefix = field.label();
            if let Some(rest) = line
                .strip_prefix(prefix)
                .and_then(|r| r.strip_prefix([':', '=', ' ', '\t']))
                .map(str::trim)
                .filter(|r| !r.is_empty())
            {
                return Ok((field, rest));
            }
            // also accept "address <value>" with single space already handled above
        }
        Err(ConfigError::UnknownField)
    }

    /// Display-safe password (never show the real secret on-screen).
    pub fn password_masked(&self) -> String<16> {
        let mut s = String::new();
        if self.password.is_empty() {
            let _ = s.push_str("(unset)");
        } else {
            let _ = s.push_str("********");
        }
        s
    }

    /// Truncate a value for the small LCD.
    pub fn ellipsize(value: &str, max_chars: usize) -> String<96> {
        let mut out = String::new();
        if max_chars == 0 {
            return out;
        }
        if value.chars().count() <= max_chars {
            let _ = out.push_str(value);
            return out;
        }
        if max_chars <= 3 {
            for c in value.chars().take(max_chars) {
                let _ = out.push(c);
            }
            return out;
        }
        for c in value.chars().take(max_chars - 3) {
            let _ = out.push(c);
        }
        let _ = out.push_str("...");
        out
    }
}

fn normalize_value(raw: &str) -> Result<&str, ConfigError> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(ConfigError::Empty);
    }
    if !value
        .chars()
        .all(|c| c.is_ascii_graphic() || c == ' ' || c == '@' || c == ':' || c == '/' || c == '.')
    {
        // allow typical wallet / URL charset (ascii graphic covers most)
        if !value.chars().all(|c| c.is_ascii() && !c.is_ascii_control()) {
            return Err(ConfigError::InvalidChar);
        }
    }
    Ok(value)
}

fn validate_stratum(value: &str) -> Result<(), ConfigError> {
    // Accept host, host:port, stratum+tcp://host:port, etc.
    if value.is_empty() {
        return Err(ConfigError::Empty);
    }
    if value.chars().any(|c| c.is_whitespace()) {
        return Err(ConfigError::InvalidChar);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_each_field_individually() {
        let mut cfg = PoolConfig::new();
        assert!(!cfg.is_complete());
        cfg.set(SetupField::Address, "LWallet123").unwrap();
        cfg.set(SetupField::Password, "x").unwrap();
        cfg.set(SetupField::Stratum, "stratum.example.com:3333")
            .unwrap();
        assert!(cfg.is_complete());
        assert_eq!(cfg.address.as_str(), "LWallet123");
        assert_eq!(cfg.password.as_str(), "x");
        assert_eq!(cfg.stratum.as_str(), "stratum.example.com:3333");
    }

    #[test]
    fn rejects_empty_and_parses_assignments() {
        let mut cfg = PoolConfig::new();
        assert_eq!(
            cfg.set(SetupField::Address, "   "),
            Err(ConfigError::Empty)
        );

        let (field, value) = PoolConfig::parse_assignment("address: LtcAddr99").unwrap();
        assert_eq!(field, SetupField::Address);
        cfg.set(field, value).unwrap();
        assert_eq!(cfg.address.as_str(), "LtcAddr99");

        let (field, value) = PoolConfig::parse_assignment("password=secret").unwrap();
        cfg.set(field, value).unwrap();
        let (field, value) = PoolConfig::parse_assignment("stratum stratum+tcp://pool:4444").unwrap();
        cfg.set(field, value).unwrap();
        assert!(cfg.is_complete());
    }

    #[test]
    fn masks_password_and_ellipsizes() {
        let mut cfg = PoolConfig::new();
        assert_eq!(cfg.password_masked().as_str(), "(unset)");
        cfg.set(SetupField::Password, "hunter2").unwrap();
        assert_eq!(cfg.password_masked().as_str(), "********");
        let short = PoolConfig::ellipsize("ABCDE12345", 8);
        assert_eq!(short.as_str(), "ABCDE...");
    }
}
