use std::{fmt::Display, path::PathBuf, string::FromUtf8Error};

use url::Url;

/// Normalized wikipedia article title that can compare:
/// - titles `Spatial Database`
/// - urls `https://en.wikipedia.org/wiki/Spatial_database#Geodatabase`
/// - osm-style tags `en:Spatial Database`
///
/// ```
/// use om_wikiparser::wm::Title;
///
/// let title = Title::from_title("Article Title", "en").unwrap();
/// let url = Title::from_url("https://en.wikipedia.org/wiki/Article_Title#Section").unwrap();
/// let mobile = Title::from_url("https://en.m.wikipedia.org/wiki/Article_Title#Section").unwrap();
/// let url_tag1 = Title::from_osm_tag("https://en.m.wikipedia.org/wiki/Article_Title#Section").unwrap();
/// let url_tag2 = Title::from_osm_tag("de:https://en.m.wikipedia.org/wiki/Article_Title#Section").unwrap();
/// assert_eq!(url, title);
/// assert_eq!(url, mobile);
/// assert_eq!(url, url_tag1);
/// assert_eq!(url, url_tag2);
///
/// assert!(Title::from_url("https://en.wikipedia.org/not_a_wiki_page").is_err());
/// assert!(Title::from_url("https://wikidata.org/wiki/Q12345").is_err());
///
/// assert!(
///     Title::from_url("https://de.wikipedia.org/wiki/Breil/Brigels").unwrap() !=
///     Title::from_url("https://de.wikipedia.org/wiki/Breil").unwrap()
/// );
/// ```
#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct Title {
    lang: String,
    name: String,
}

impl Display for Title {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.lang, self.name)
    }
}

impl Title {
    fn normalize_title(title: &str) -> String {
        // TODO: Compare with map generator url creation, ensure covers all cases.
        title.trim().replace(' ', "_")
    }

    // https://en.wikipedia.org/wiki/Article_Title/More_Title
    pub fn from_url(url: &str) -> Result<Self, ParseTitleError> {
        let url = Url::parse(url.trim())?;

        let (subdomain, host) = url
            .host_str()
            .ok_or(ParseTitleError::NoHost)?
            .split_once('.')
            .ok_or(ParseTitleError::NoSubdomain)?;
        let host = host.strip_prefix("m.").unwrap_or(host);
        if host != "wikipedia.org" {
            return Err(ParseTitleError::BadDomain);
        }
        let lang = subdomain;

        let path = url.path();

        let (root, title) = path
            .strip_prefix('/')
            .unwrap_or(path)
            .split_once('/')
            .ok_or(ParseTitleError::ShortPath)?;

        if root != "wiki" {
            return Err(ParseTitleError::BadPath);
        }
        let title = urlencoding::decode(title)?;

        Self::from_title(&title, lang)
    }

    // en:Article Title
    pub fn from_osm_tag(tag: &str) -> Result<Self, ParseTitleError> {
        let (lang, title) = tag
            .trim()
            .split_once(':')
            .ok_or(ParseTitleError::MissingColon)?;

        let lang = lang.trim_start();
        let title = title.trim_start();

        if matches!(lang, "http" | "https") {
            return Self::from_url(tag);
        }

        if title.starts_with("http://") || title.starts_with("https://") {
            return Self::from_url(title);
        }

        Self::from_title(title, lang)
    }

    pub fn from_title(title: &str, lang: &str) -> Result<Self, ParseTitleError> {
        let title = title.trim();
        let lang = lang.trim();
        if title.is_empty() {
            return Err(ParseTitleError::NoTitle);
        }
        if lang.is_empty() {
            return Err(ParseTitleError::NoLang);
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

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ParseTitleError {
    #[error("title cannot be empty or whitespace")]
    NoTitle,
    #[error("lang cannot be empty or whitespace")]
    NoLang,
    #[error("no ':' separating lang and title")]
    MissingColon,

    // url-specific
    #[error("cannot parse url")]
    Url(#[from] url::ParseError),
    #[error("cannot decode url")]
    UrlDecode(#[from] FromUtf8Error),
    #[error("no host in url")]
    NoHost,
    #[error("no subdomain in url")]
    NoSubdomain,
    #[error("url base domain is wikipedia.org")]
    BadDomain,
    #[error("url base path is not /wiki/")]
    BadPath,
    #[error("path has less than 2 segments")]
    ShortPath,
}
