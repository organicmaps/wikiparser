//! Wikimedia types
use std::{
    collections::HashSet, ffi::OsStr, fmt::Display, fs, num::ParseIntError, path::PathBuf,
    str::FromStr,
};

use anyhow::{anyhow, bail, Context};

use url::Url;

mod page;
pub use page::Page;

/// Read from a file of urls on each line.
pub fn parse_wikidata_file(path: impl AsRef<OsStr>) -> anyhow::Result<HashSet<WikidataQid>> {
    let contents = fs::read_to_string(path.as_ref())?;
    Ok(contents
        .lines()
        .enumerate()
        .map(|(i, line)| {
            WikidataQid::from_str(line).with_context(|| {
                let line_num = i + 1;
                format!("on line {line_num}: {line:?}")
            })
        })
        .filter_map(|r| match r {
            Ok(qid) => Some(qid),
            Err(e) => {
                warn!("Could not parse QID: {:#}", e);
                None
            }
        })
        .collect())
}

/// Read article titles from a file of urls on each line.
pub fn parse_wikipedia_file(
    path: impl AsRef<OsStr>,
) -> anyhow::Result<HashSet<WikipediaTitleNorm>> {
    let contents = fs::read_to_string(path.as_ref())?;
    Ok(contents
        .lines()
        .enumerate()
        .map(|(i, line)| {
            WikipediaTitleNorm::from_url(line).with_context(|| {
                let line_num = i + 1;
                format!("on line {line_num}: {line:?}")
            })
        })
        .filter_map(|r| match r {
            Ok(qid) => Some(qid),
            Err(e) => {
                warn!("Could not parse wikipedia title: {:#}", e);
                None
            }
        })
        .collect())
}

pub fn parse_osm_tag_file(
    path: impl AsRef<OsStr>,
    qids: &mut HashSet<WikidataQid>,
    titles: &mut HashSet<WikipediaTitleNorm>,
) -> anyhow::Result<()> {
    let path = path.as_ref();
    let mut rdr = csv::ReaderBuilder::new().delimiter(b'\t').from_path(path)?;

    let mut qid_col = None;
    let mut title_col = None;
    for (column, title) in rdr.headers()?.iter().enumerate() {
        match title {
            "wikidata" => qid_col = Some(column),
            "wikipedia" => title_col = Some(column),
            _ => (),
        }
    }

    let qid_col = qid_col.ok_or_else(|| anyhow!("Cannot find 'wikidata' column"))?;
    let title_col = title_col.ok_or_else(|| anyhow!("Cannot find 'wikipedia' column"))?;

    let mut row = csv::StringRecord::new();
    loop {
        match rdr.read_record(&mut row) {
            Ok(true) => {}
            // finished
            Ok(false) => break,
            // attempt to recover from parsing errors
            Err(e) => {
                error!("Error parsing tsv file: {}", e);
                continue;
            }
        }

        let qid = &row[qid_col].trim();
        if !qid.is_empty() {
            match WikidataQid::from_str(qid) {
                Ok(qid) => {
                    qids.insert(qid);
                }
                Err(e) => warn!(
                    "Cannot parse qid {:?} on line {} in {:?}: {}",
                    qid,
                    rdr.position().line(),
                    path,
                    e
                ),
            }
        }

        let title = &row[title_col].trim();
        if !title.is_empty() {
            match WikipediaTitleNorm::_from_osm_tag(title) {
                Ok(title) => {
                    titles.insert(title);
                }
                Err(e) => warn!(
                    "Cannot parse title {:?} on line {} in {:?}: {}",
                    title,
                    rdr.position().line(),
                    path,
                    e
                ),
            }
        }
    }

    Ok(())
}

/// Wikidata QID/Q Number
///
/// See https://www.wikidata.org/wiki/Wikidata:Glossary#QID
///
/// ```
/// use std::str::FromStr;
/// use om_wikiparser::wm::WikidataQid;
///
/// let with_q = WikidataQid::from_str("Q12345").unwrap();
/// let without_q = WikidataQid::from_str(" 12345 ").unwrap();
/// assert_eq!(with_q, without_q);
///
/// assert!(WikidataQid::from_str("q12345").is_ok());
/// assert!(WikidataQid::from_str("https://wikidata.org/wiki/Q12345").is_err());
/// assert!(WikidataQid::from_str("Article_Title").is_err());
/// assert!(WikidataQid::from_str("Q").is_err());
/// assert!(WikidataQid::from_str("").is_err());
/// ```
#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct WikidataQid(u32);

impl FromStr for WikidataQid {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let s = s.strip_prefix(['Q', 'q']).unwrap_or(s);
        u32::from_str(s).map(WikidataQid)
    }
}

impl Display for WikidataQid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Q{}", self.0)
    }
}

impl WikidataQid {
    pub fn get_dir(&self, base: PathBuf) -> PathBuf {
        let mut path = base;
        path.push("wikidata");
        // TODO: can use as_mut_os_string with 1.70.0
        path.push(self.to_string());

        path
    }
}

/// Normalized wikipedia article title that can compare:
/// - titles `Spatial Database`
/// - urls `https://en.wikipedia.org/wiki/Spatial_database#Geodatabase`
/// - osm-style tags `en:Spatial Database`
///
/// ```
/// use om_wikiparser::wm::WikipediaTitleNorm;
///
/// let title = WikipediaTitleNorm::from_title("Article Title", "en").unwrap();
/// let url = WikipediaTitleNorm::from_url("https://en.wikipedia.org/wiki/Article_Title#Section").unwrap();
/// assert_eq!(url, title);
///
/// assert!(WikipediaTitleNorm::from_url("https://en.wikipedia.org/not_a_wiki_page").is_err());
/// assert!(WikipediaTitleNorm::from_url("https://wikidata.org/wiki/Q12345").is_err());
///
/// assert!(
///     WikipediaTitleNorm::from_url("https://de.wikipedia.org/wiki/Breil/Brigels").unwrap() !=
///     WikipediaTitleNorm::from_url("https://de.wikipedia.org/wiki/Breil").unwrap()
/// );
/// ```
#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct WikipediaTitleNorm {
    lang: String,
    name: String,
}

impl WikipediaTitleNorm {
    fn normalize_title(title: &str) -> String {
        // TODO: Compare with map generator url creation, ensure covers all cases.
        title.trim().replace(' ', "_")
    }

    // https://en.wikipedia.org/wiki/Article_Title/More_Title
    pub fn from_url(url: &str) -> anyhow::Result<Self> {
        let url = Url::parse(url.trim())?;

        let (subdomain, host) = url
            .host_str()
            .ok_or_else(|| anyhow!("Expected host"))?
            .split_once('.')
            .ok_or_else(|| anyhow!("Expected subdomain"))?;
        if host != "wikipedia.org" {
            bail!("Expected wikipedia.org for domain")
        }
        let lang = subdomain;

        let path = url.path();

        let (root, title) = path
            .strip_prefix('/')
            .unwrap_or(path)
            .split_once('/')
            .ok_or_else(|| anyhow!("Expected at least two segments in path"))?;

        if root != "wiki" {
            bail!("Expected 'wiki' as root path, got: {:?}", root)
        }
        let title = urlencoding::decode(title)?;

        Self::from_title(&title, lang)
    }

    // en:Article Title
    fn _from_osm_tag(tag: &str) -> anyhow::Result<Self> {
        let (lang, title) = tag
            .trim()
            .split_once(':')
            .ok_or_else(|| anyhow!("Expected ':'"))?;

        Self::from_title(title, lang)
    }

    pub fn from_title(title: &str, lang: &str) -> anyhow::Result<Self> {
        let title = title.trim();
        let lang = lang.trim();
        if title.is_empty() {
            bail!("title cannot be empty or whitespace");
        }
        if lang.is_empty() {
            bail!("lang cannot be empty or whitespace");
        }
        let name = Self::normalize_title(title);
        let lang = lang.to_owned();
        Ok(Self { name, lang })
    }

    pub fn get_dir(&self, base: PathBuf) -> PathBuf {
        let mut path = base;
        // TODO: can use as_mut_os_string with 1.70.0
        path.push(format!("{}.wikipedia.org", self.lang));
        path.push("wiki");
        path.push(&self.name);

        path
    }
}
