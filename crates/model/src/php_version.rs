use std::{fmt, str::FromStr};

/// A PHP language version, as the target the project is written against.
///
/// Only major and minor are modelled: no syntax or type rule in PHP has ever turned on a patch
/// version, and carrying one would invite comparisons that look meaningful and are not.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct PhpVersion {
    major: u8,
    minor: u8,
}

impl PhpVersion {
    pub const PHP_8_0: Self = Self::new(8, 0);
    pub const PHP_8_1: Self = Self::new(8, 1);
    pub const PHP_8_2: Self = Self::new(8, 2);
    pub const PHP_8_3: Self = Self::new(8, 3);
    pub const PHP_8_4: Self = Self::new(8, 4);
    pub const PHP_8_5: Self = Self::new(8, 5);

    pub const fn new(major: u8, minor: u8) -> Self {
        Self { major, minor }
    }

    pub const fn major(self) -> u8 {
        self.major
    }

    pub const fn minor(self) -> u8 {
        self.minor
    }

    pub fn at_least(self, other: Self) -> bool {
        self >= other
    }
}

impl Default for PhpVersion {
    /// Newest release. Chosen so that a project without a declared version gets current syntax
    /// accepted rather than flagged: a false syntax error on every line of a modern file is far
    /// worse than silently accepting a construct an older runtime would reject.
    fn default() -> Self {
        Self::PHP_8_5
    }
}

impl fmt::Display for PhpVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Why a version string could not be read.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PhpVersionParseError(String);

impl fmt::Display for PhpVersionParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` is not a `<major>.<minor>` PHP version", self.0)
    }
}

impl std::error::Error for PhpVersionParseError {}

impl FromStr for PhpVersion {
    type Err = PhpVersionParseError;

    /// Accepts `"8.3"` and bare `"8"`. Anything with a patch component is rejected rather than
    /// truncated, so a composer constraint like `"8.3.2"` fails loudly instead of pretending to
    /// have been understood.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let error = || PhpVersionParseError(text.to_owned());
        let (major, minor) = match text.split_once('.') {
            Some((major, minor)) => (major, minor),
            None => (text, "0"),
        };

        Ok(Self::new(
            major.parse().map_err(|_| error())?,
            minor.parse().map_err(|_| error())?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_major_minor_and_bare_major() {
        assert_eq!("8.3".parse::<PhpVersion>().unwrap(), PhpVersion::PHP_8_3);
        assert_eq!("8".parse::<PhpVersion>().unwrap(), PhpVersion::new(8, 0));
    }

    #[test]
    fn rejects_a_patch_component_instead_of_truncating_it() {
        assert!("8.3.2".parse::<PhpVersion>().is_err());
        assert!("".parse::<PhpVersion>().is_err());
        assert!("eight".parse::<PhpVersion>().is_err());
    }

    #[test]
    fn compares_by_major_then_minor() {
        assert!(PhpVersion::PHP_8_5.at_least(PhpVersion::PHP_8_3));
        assert!(!PhpVersion::PHP_8_1.at_least(PhpVersion::PHP_8_2));
        assert!(PhpVersion::new(9, 0).at_least(PhpVersion::PHP_8_5));
    }

    #[test]
    fn defaults_to_the_newest_release() {
        assert_eq!(PhpVersion::default(), PhpVersion::PHP_8_5);
    }
}
