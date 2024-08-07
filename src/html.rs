//! Simplification of Wikipedia Enterprise HTML.
//!
//! The goal is to process the [Enterprise API] HTML output similar to the [TextExtracts API](https://www.mediawiki.org/wiki/Extension:TextExtracts).
//! In particular:
//! - All images, media, tables, and wrapper elements like divs and sections are removed.
//! - Doctype, comments, `html`, `body`, `head`, etc. are removed.
//! - Only top-level headers, paragraphs, links and basic text formatting (`b`, `i`, etc.)
//!
//! The HTML that the TextExtracts API starts with seems to be different from the Enterprise API, which follows [this spec](https://www.mediawiki.org/wiki/Specs/HTML/) and seems to have more content and data encoded in attributes.

use std::{
    any::Any,
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    ops::Deref,
    panic,
};

use ego_tree::NodeId;
use markup5ever::{LocalName, Namespace, QualName};
use once_cell::sync::Lazy;
use scraper::{ElementRef, Html, Node, Selector};
use serde::Deserialize;

use url::Url;

#[derive(Debug, Deserialize)]
struct Config<'a> {
    #[serde(borrow)]
    sections_to_remove: BTreeMap<&'a str, BTreeSet<&'a str>>,
}

/// Path to the processing config file.
///
/// Other compile-time macros expect a string literal, so this must be a macro instead of a const str.
macro_rules! config_path {
    () => {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/article_processing_config.json"
        )
    };
}

static CONFIG: Lazy<Config<'static>> = Lazy::new(|| {
    serde_json::from_str(include_str!(config_path!())).expect(concat!(
        config_path!(),
        " is either invalid json or the wrong structure"
    ))
});

static HEADERS: Lazy<Selector> =
    Lazy::new(|| Selector::parse("h1, h2, h3, h4, h5, h6, h7").unwrap());

/// Elements that should always be kept, regardless of other metrics.
static ELEMENT_ALLOW_LIST: Lazy<Selector> = Lazy::new(|| {
    Selector::parse(
        &[
            // Content from other articles (expanded later)
            // TODO: See if these are used in other ways.
            "div.excerpt-block",
            "div.excerpt",
        ]
        .join(", "),
    )
    .unwrap()
});

/// Elements that should be removed.
static ELEMENT_DENY_LIST: Lazy<Selector> = Lazy::new(|| {
    Selector::parse(
        &[
            // From the Extracts API config `extension.json`: https://phabricator.wikimedia.org/diffusion/ETEX/browse/master/extension.json
            "table",
            "div",
            "figure",
            "script",
            "input",
            "style",
            "ul.gallery",
            ".mw-editsection",
            "sup.reference",
            "ol.references",
            ".error",
            ".nomobile",
            ".noprint",
            ".noexcerpt",
            ".sortkey",
            // Media elements.
            "img",
            "audio",
            "video",
            "figure",
            "embed",
            // Pronunciation "listen" link/button.
            r#"span[typeof="mw:Transclusion"][data-mw*="\"audio\":"]"#,
            // Coordinates transclusion.
            "span#coordinates",
            // Remove head altogether.
            "head",
        ]
        .join(", "),
    )
    .unwrap()
});

/// Convenience wrapper around [[process]].
pub fn process_str(html: &str, lang: &str) -> Result<String, HtmlError> {
    let document = Html::parse_document(html);
    let document = process(document, lang)?;
    Ok(document.html())
}

/// Simplify an article, checking for bad pages and failures.
pub fn process(mut document: Html, lang: &str) -> Result<Html, HtmlError> {
    panic::catch_unwind(|| {
        if let Some(redirect) = detect_redirect(&document) {
            return Err(HtmlError::Redirect(redirect.to_owned()));
        }
        simplify(&mut document, lang);
        if !has_text(&document) {
            return Err(HtmlError::NoText);
        }
        Ok(document)
    })
    .map_err(PanicMsg::new)?
}

/// Attempt to find target title of the article if it is a redirect.
pub fn detect_redirect(document: &Html) -> Option<&str> {
    static REDIRECT: Lazy<Selector> =
        Lazy::new(|| Selector::parse(r#"link[rel="mw:PageProp/redirect"]"#).unwrap());

    document.select(&REDIRECT).next().map(|el| {
        let href = el.value().attr("href").unwrap_or_default().trim();
        let redirect = href.strip_prefix("./").unwrap_or(href);
        redirect
    })
}

/// Attempt to find the wikipedia language of the article.
pub fn detect_lang(document: &Html) -> Option<String> {
    static BASE: Lazy<Selector> = Lazy::new(|| Selector::parse("head > base[href]").unwrap());

    document
        .select(&BASE)
        .next()
        .and_then(|el| el.value().attr("href"))
        .and_then(|url| {
            let mut url = url.to_owned();
            if url.starts_with("//") {
                url.insert_str(0, "http:");
            }

            match Url::parse(&url) {
                Err(e) => {
                    trace!("Error parsing base lang url: {}", e);
                    None
                }
                Ok(url) => {
                    let domain = url.domain()?;
                    let (lang, domain) = domain.split_once('.')?;
                    if domain != "wikipedia.org" {
                        trace!("Domain of base lang url is not wikipedia.org: {}", domain);
                    }
                    Some(lang.to_owned())
                }
            }
        })
}

/// Check if the html contains any non-whitespace text nodes.
pub fn has_text(document: &Html) -> bool {
    if let Some(root) = ElementRef::wrap(document.tree.root()) {
        !is_empty_or_whitespace(&root)
    } else {
        !document
            .tree
            .root()
            .children()
            .filter_map(ElementRef::wrap)
            .all(|el| is_empty_or_whitespace(&el))
    }
}

/// Simplify an article to only basic text.
///
/// # Panics
///
/// This modifies the HTML tree in a way that violates some assumptions of the underlying
/// `scraper` and `ego-tree` crates and cause panics.
///
/// If this is undesirable, see [[process]] for a higher-level wrapper that
/// handles panics and other errors.
pub fn simplify(document: &mut Html, lang: &str) {
    if let Some(titles) = CONFIG.sections_to_remove.get(lang) {
        remove_named_header_siblings(document, titles);
    }

    remove_denylist_elements(document);

    remove_empty_sections(document);

    expand_empty(document);

    remove_non_element_nodes(document);

    remove_attrs(document);

    final_expansions(document);

    remove_toplevel_whitespace(document);
}

fn remove_ids(document: &mut Html, ids: impl IntoIterator<Item = NodeId>) {
    for id in ids {
        if let Some(mut node) = document.tree.get_mut(id) {
            node.detach();
        }
    }
}

/// Remove headers with the specified `titles` and all following siblings until the next header greater or equal level.
///
/// `titles` are matched by case-sensitive simple byte comparison.
/// `titles` should be normalized to Unicode NFC to match Wikipedia's internal normalization: <https://mediawiki.org/wiki/Unicode_normalization_considerations>.
fn remove_named_header_siblings(document: &mut Html, titles: &BTreeSet<&str>) {
    let mut to_remove = Vec::new();

    for header in document.select(&HEADERS) {
        // TODO: Should this join all text nodes?
        let Some(title) = header.text().next() else {
            continue;
        };
        if !titles.contains(title) {
            continue;
        }

        to_remove.push(header.id());
        let header_level = header.value().name();
        trace!("Removing trailing siblings for header {header_level} {title:?}");
        for sibling in header.next_siblings() {
            if let Some(element) = ElementRef::wrap(sibling) {
                if HEADERS.matches(&element) && element.value().name() <= header_level {
                    trace!("Stopping removal early at {}", element.value().name(),);
                    break;
                }
            }
            to_remove.push(sibling.id());
        }
    }

    remove_ids(document, to_remove.drain(..));
}

fn remove_denylist_elements(document: &mut Html) {
    let mut to_remove = Vec::new();
    for el in document
        .root_element()
        .descendants()
        .filter_map(ElementRef::wrap)
    {
        if ELEMENT_DENY_LIST.matches(&el) && !ELEMENT_ALLOW_LIST.matches(&el) {
            to_remove.push(el.id());
        }
    }
    remove_ids(document, to_remove.drain(..));
}

fn remove_non_element_nodes(document: &mut Html) {
    let mut to_remove = Vec::new();
    // `.root_element()` returns the first `Element` in the children of the
    // root, which comments/doctypes are not.
    // Use `root()` instead and `skip()` because `descendants` includes the
    // node it is called on.
    for el in document.tree.root().descendants().skip(1) {
        if el.value().is_comment() || el.value().is_doctype() {
            to_remove.push(el.id());
        }
    }
    remove_ids(document, to_remove.drain(..));
}

fn remove_toplevel_whitespace(document: &mut Html) {
    let mut to_remove = Vec::new();

    let parent = document.tree.root();

    for el in parent.children() {
        let Some(t) = el.value().as_text() else {
            continue;
        };

        if t.chars().all(char::is_whitespace) {
            to_remove.push(el.id());
        }
    }

    trace!(
        "Removing {} whitespace text nodes children from {:?}",
        to_remove.len(),
        parent.value(),
    );
    remove_ids(document, to_remove.drain(..));
}

/// Expand elements that contain no text or only whitespace, leaving only their contents.
fn expand_empty(document: &mut Html) {
    let mut to_remove = Vec::new();

    for el in document
        .root_element()
        .descendants()
        .filter_map(ElementRef::wrap)
    {
        if is_empty_or_whitespace(&el) {
            to_remove.push(el.id());
        }
    }

    for id in to_remove.drain(..) {
        expand_id(document, id);
    }
}

fn remove_empty_sections(document: &mut Html) {
    let mut to_remove = Vec::new();
    for el in document.select(&HEADERS) {
        let Some(parent) = el.parent() else {
            continue;
        };

        if !parent
            .value()
            .as_element()
            .map(|p| p.name() == "section")
            .unwrap_or_default()
        {
            trace!("Skipping header without section name: {:?}", parent);
            continue;
        }

        if el
            .prev_siblings()
            .chain(el.next_siblings())
            .filter_map(ElementRef::wrap)
            .all(|e| is_empty_or_whitespace(&e) || HEADERS.matches(&e))
        {
            trace!(
                "Removing empty section {} {:?}",
                el.value().name(),
                el.text().collect::<String>()
            );
            to_remove.push(parent.id());
        }
    }

    remove_ids(document, to_remove);
}

fn remove_attrs(document: &mut Html) {
    let mut to_remove = Vec::new();

    let all_elements: Vec<_> = document
        .tree
        .root()
        .descendants()
        .filter_map(ElementRef::wrap)
        .map(|el| el.id())
        .collect();

    trace!("Removing attributes on {} elements", all_elements.len());

    for id in all_elements {
        let Some(mut node) = document.tree.get_mut(id) else {
            trace!("Invalid id: {:?}", id);
            continue;
        };
        let Node::Element(el) = node.value() else {
            continue;
        };

        if el.name() == "span" {
            for attr in ["style", "class"]
                .iter()
                .map(|attr| QualName::new(None, Namespace::from(""), LocalName::from(*attr)))
            {
                el.attrs.remove(&attr);
            }
        }

        for (k, _v) in el.attrs.iter() {
            if k.local.starts_with("data-mw")
                // TODO: To keep ids for linking to headers, only remove ones that start with "mw".
                || ["id", "prefix", "typeof", "about", "rel"]
                    .iter()
                    .any(|attr| *attr == &k.local)
            {
                to_remove.push(k.to_owned());
            }
        }

        for k in to_remove.drain(..) {
            el.attrs.remove(&k);
        }
    }
}

fn final_expansions(document: &mut Html) {
    let mut to_expand = Vec::new();
    for el in document
        .tree
        .root()
        .descendants()
        .filter_map(ElementRef::wrap)
    {
        if (el.value().name() == "span" && el.value().attrs().next().is_none())
            || ["a", "section", "div", "body", "html"].contains(&el.value().name())
        {
            to_expand.push(el.id());
        }
    }

    trace!("Expanding {} elements", to_expand.len());

    for id in to_expand {
        expand_id(document, id);
    }
}

fn is_empty_or_whitespace(el: &ElementRef) -> bool {
    el.text().flat_map(str::chars).all(char::is_whitespace)
}

/// Remove an element, leaving any children in its place.
fn expand_id(document: &mut Html, id: NodeId) {
    let Some(mut node) = document.tree.get_mut(id) else {
        return;
    };
    if node.parent().is_none() {
        // Already detached.
        return;
    }

    // reparent to same location as node
    while let Some(mut child) = node.first_child() {
        let child_id = child.id();
        child.detach();
        node.insert_id_before(child_id);
    }

    node.detach();
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum HtmlError {
    /// Processing this HTML caused a panic in an underlying library
    #[error("panicked while processing html")]
    Panic(#[from] PanicMsg),
    #[error("page is redirect stub for {0:?}")]
    Redirect(String),
    #[error("page has no text after processing")]
    NoText,
}

/// Error wrapper around panic payloads that handles static and formatted messages.
#[derive(Debug, PartialEq)]
pub struct PanicMsg(Cow<'static, str>);

impl PanicMsg {
    pub fn new(payload: Box<dyn Any + Send + 'static>) -> Self {
        let msg = if let Some(s) = payload.downcast_ref::<&str>() {
            Some(Cow::Borrowed(*s))
        } else {
            payload.downcast::<String>().ok().map(|s| Cow::Owned(*s))
        };

        Self(msg.unwrap_or_default())
    }
}

impl Display for PanicMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for PanicMsg {}

impl Deref for PanicMsg {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn static_config_parses() {
        assert!(!CONFIG.sections_to_remove.is_empty());
    }

    /// Ensure config sections match Wikipedia's Unicode normalization (NFC) so
    /// that they can be correctly compared bytewise.
    ///
    /// As the discussion below mentions, there is an edge-case where section
    /// names in the article contain templates that expand to non-normalized
    /// text, which this does not handle.
    ///
    /// See also:
    /// - [super::remove_named_header_siblings]
    /// - Mediawiki discussion of normalization: https://mediawiki.org/wiki/Unicode_normalization_considerations
    /// - Online conversion tool: https://util.unicode.org/UnicodeJsps/transform.jsp?a=Any-NFC
    #[test]
    fn static_config_sections_are_normalized() {
        use unicode_normalization::{is_nfc, UnicodeNormalization};

        let mut all_sections_are_normalized = true;
        for section in CONFIG.sections_to_remove.values().flatten() {
            if !is_nfc(section) {
                all_sections_are_normalized = false;
                let normalized = String::from_iter(section.nfc());
                eprintln!("Section to remove {section:?} should be normalized to {normalized:?}");
            }
        }

        assert!(
            all_sections_are_normalized,
            "Not all sections in {} are in Unicode NFC. Please replace the reported sections.",
            config_path!()
        );
    }

    fn expand_links(document: &mut Html) {
        let links: Vec<_> = document
            .select(&Selector::parse("a").unwrap())
            .map(|el| el.id())
            .collect();

        for id in links {
            expand_id(document, id)
        }
    }

    #[test]
    fn remove_links() {
        let html = r#"
        <p> Some text that includes
            <a href="Some_Page"><span id="inner-content">several</span></a>
            <a id="second-link" href="./Another_Page">relative links</a>
        and
            <a href="https://example.com/page">an absolute link</a>
        .
        </p>
        "#;

        let anchors = Selector::parse("a").unwrap();
        let inner_element = Selector::parse("#inner-content").unwrap();
        let second_link = Selector::parse("#second-link").unwrap();

        let mut document = Html::parse_fragment(html);
        let links: Vec<_> = document
            .select(&anchors)
            .filter_map(|el| el.value().attr("href"))
            .collect();

        eprintln!("{}", document.html());

        assert_eq!(
            vec!["Some_Page", "./Another_Page", "https://example.com/page"],
            links,
            "Links in original html are not expected."
        );

        // Detach one of the links from the root tree (as if previously deleted) to ensure it handles orphan nodes nicely.
        let link = document.select(&second_link).next().unwrap().id();
        document.tree.get_mut(link).unwrap().detach();

        expand_links(&mut document);

        let links: Vec<_> = document.select(&anchors).collect();

        assert!(links.is_empty(), "All links should be removed.");

        assert!(
            document.select(&inner_element).next().is_some(),
            "Link inner elements should be preserved."
        );
    }

    #[test]
    /// Verify trailing siblings are removed up to superheaders.
    fn remove_headers() {
        let html = r#"
            <h1>Title</h1>
                <p id="p1">Foo bar</p>
            <h2>Section 1</h2>
                <p id="p2">Foo bar</p>
            <h3>Subsection</h3>
                <p id="p3">Foo bar</p>
            <h1>Next Title</h1>
                <p id="p4">Foo bar</p>
            <h2>Section 2</h2>
                <p id="p5">Foo bar</p>
        "#;

        let paragraphs = Selector::parse("p").unwrap();

        let mut document = Html::parse_fragment(html);

        assert_eq!(
            vec!["p1", "p2", "p3", "p4", "p5"],
            document
                .select(&paragraphs)
                .map(|el| el.value().id().unwrap_or_default())
                .collect::<Vec<_>>(),
            "paragraphs in original html are not expected"
        );

        remove_named_header_siblings(&mut document, &BTreeSet::from_iter(Some("Section 1")));

        eprintln!("{}", document.html());

        assert_eq!(
            vec!["p1", "p4", "p5"],
            document
                .select(&paragraphs)
                .map(|el| el.value().id().unwrap_or_default())
                .collect::<Vec<_>>(),
            "only p2 and p3 should be removed"
        );
    }
}
