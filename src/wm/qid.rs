use std::{error::Error, fmt::Display, num::ParseIntError, path::PathBuf, str::FromStr};

/// Wikidata QID/Q Number
///
/// See https://www.wikidata.org/wiki/Wikidata:Glossary#QID
///
/// ```
/// use std::str::FromStr;
/// use om_wikiparser::wm::Qid;
///
/// let with_q = Qid::from_str("Q12345").unwrap();
/// let without_q = Qid::from_str(" 12345 ").unwrap();
/// assert_eq!(with_q, without_q);
///
/// assert!(Qid::from_str("q12345").is_ok());
/// assert!(Qid::from_str("https://wikidata.org/wiki/Q12345").is_err());
/// assert!(Qid::from_str("Article_Title").is_err());
/// assert!(Qid::from_str("Q").is_err());
/// assert!(Qid::from_str("").is_err());
/// ```
#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct Qid(u32);

impl FromStr for Qid {
    type Err = ParseQidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let s = s.strip_prefix(['Q', 'q']).unwrap_or(s);
        u32::from_str(s).map(Qid).map_err(ParseQidError)
    }
}

impl Display for Qid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Q{}", self.0)
    }
}

impl Qid {
    pub fn get_dir(&self, base: PathBuf) -> PathBuf {
        let mut path = base;
        path.push("wikidata");
        // TODO: can use as_mut_os_string with 1.70.0
        path.push(self.to_string());

        path
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseQidError(ParseIntError);

impl Display for ParseQidError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Error for ParseQidError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.0.source()
    }
}
