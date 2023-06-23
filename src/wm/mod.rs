//! Wikimedia types
use std::{collections::HashSet, ffi::OsStr, fs, num::ParseIntError, str::FromStr};

use anyhow::{anyhow, bail, Context};

use url::Url;

mod page;
pub use page::Page;

/// Read from a file of urls on each line.
pub fn parse_wikidata_file(path: impl AsRef<OsStr>) -> anyhow::Result<HashSet<WikidataQid>> {
    let contents = fs::read_to_string(path.as_ref())?;
    contents
        .lines()
        .enumerate()
        .map(|(i, line)| {
            WikidataQid::from_str(line).with_context(|| {
                let line_num = i + 1;
                format!("bad QID value on line {line_num}: {line:?}")
            })
        })
        .collect()
}

/// Read article titles from a file of urls on each line.
pub fn parse_wikipedia_file(
    path: impl AsRef<OsStr>,
) -> anyhow::Result<HashSet<WikipediaTitleNorm>> {
    let contents = fs::read_to_string(path.as_ref())?;
    contents
        .lines()
        .enumerate()
        .map(|(i, line)| {
            WikipediaTitleNorm::from_url(line).with_context(|| {
                let line_num = i + 1;
                format!("bad wikipedia url on line {line_num}: {line:?}")
            })
        })
        .collect()
}

pub fn is_wikidata_match(ids: &HashSet<WikidataQid>, page: &Page) -> Option<WikidataQid> {
    let Some(wikidata) = &page.main_entity else { return None;};
    let wikidata_id = &wikidata.identifier;
    let wikidata_id = match WikidataQid::from_str(wikidata_id) {
        Ok(qid) => qid,
        Err(e) => {
            warn!(
                "Could not parse QID for {:?}: {:?}: {:#}",
                page.name, wikidata_id, e
            );
            return None;
        }
    };

    ids.get(&wikidata_id).map(|_| wikidata_id)
}

pub fn is_wikipedia_match(
    titles: &HashSet<WikipediaTitleNorm>,
    page: &Page,
) -> Option<WikipediaTitleNorm> {
    match WikipediaTitleNorm::from_title(&page.name, &page.in_language.identifier) {
        Err(e) => warn!("Could not parse title for {:?}: {:#}", page.name, e),
        Ok(title) => {
            if titles.get(&title).is_some() {
                return Some(title);
            }
        }
    }

    for redirect in &page.redirects {
        match WikipediaTitleNorm::from_title(&redirect.name, &page.in_language.identifier) {
            Err(e) => warn!(
                "Could not parse redirect title for {:?}: {:?}: {:#}",
                page.name, redirect.name, e
            ),
            Ok(title) => {
                if titles.get(&title).is_some() {
                    return Some(title);
                }
            }
        }
    }

    None
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

    // https://en.wikipedia.org/wiki/Article_Title
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

        let mut paths = url
            .path_segments()
            .ok_or_else(|| anyhow!("Expected path"))?;

        let root = paths
            .next()
            .ok_or_else(|| anyhow!("Expected first segment in path"))?;

        if root != "wiki" {
            bail!("Expected 'wiki' in path")
        }

        let title = paths
            .next()
            .ok_or_else(|| anyhow!("Expected second segment in path"))?;
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
}
