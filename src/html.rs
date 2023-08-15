use std::collections::{BTreeMap, BTreeSet};

use ego_tree::NodeId;
use once_cell::sync::Lazy;
use scraper::{ElementRef, Html, Selector};
use serde::Deserialize;

mod pretty;
pub use pretty::pretty_print;

#[derive(Debug, Deserialize)]
struct Config<'a> {
    #[serde(borrow)]
    sections_to_remove: BTreeMap<&'a str, BTreeSet<&'a str>>,
}

static CONFIG: Lazy<Config<'static>> = Lazy::new(|| {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/article_processing_config.json"
    )))
    .expect("\"article_processing_config.json\" is either invalid json or the wrong structure")
});

static HEADERS: Lazy<Selector> =
    Lazy::new(|| Selector::parse("h1, h2, h3, h4, h5, h6, h7").unwrap());

/// Elements that should always be kept, regardless of other metrics.
static ELEMENT_ALLOW_LIST: Lazy<Selector> = Lazy::new(|| {
    Selector::parse(
        &[
            // Meta tags that affect rendering.
            "head > meta[charset]",
            "head > meta[http-equiv]",
        ]
        .join(", "),
    )
    .unwrap()
});

pub fn simplify(html: &str, lang: &str) -> String {
    let mut document = Html::parse_document(html);
    simplify_html(&mut document, lang);
    document.html()
}

pub fn simplify_html(document: &mut Html, lang: &str) {
    let mut to_remove = Vec::new();

    // Remove configured sections and all trailing elements until next section.

    if let Some(bad_sections) = CONFIG.sections_to_remove.get(lang) {
        for header in document.select(&HEADERS) {
            // TODO: Should this join all text nodes?
            let Some(title) = header.text().next() else {
                continue
            };

            if bad_sections.contains(&title.trim()) {
                to_remove.push(header.id());
                let header_level = header.value().name();
                trace!("Removing section for header {header_level} {title:?}");
                // Strip trailing nodes.
                for sibling in header.next_siblings() {
                    if let Some(element) = sibling.value().as_element() {
                        if element.name() == header_level {
                            trace!("Stopping removal at {}", element.name(),);
                            // TODO: Should this check for a higher level?
                            break;
                        }
                    }
                    to_remove.push(sibling.id());
                }
            }
        }

        remove_ids(document, to_remove.drain(..));
    }

    for el in document
        .root_element()
        .descendants()
        .filter_map(ElementRef::wrap)
    {
        if (is_image(&el) || is_empty_or_whitespace(&el)) && !ELEMENT_ALLOW_LIST.matches(&el) {
            to_remove.push(el.id());
        }
    }
    remove_ids(document, to_remove.drain(..));

    remove_links(document);
}

fn remove_ids(document: &mut Html, ids: impl IntoIterator<Item = NodeId>) {
    for id in ids {
        if let Some(mut node) = document.tree.get_mut(id) {
            node.detach();
        }
    }
}

fn is_empty_or_whitespace(el: &ElementRef) -> bool {
    el.text().flat_map(str::chars).all(char::is_whitespace)
}

fn is_image(el: &ElementRef) -> bool {
    ["img", "picture"].contains(&el.value().name())
}

/// Remove all links, preserving any inner elements/text.
fn remove_links(document: &mut Html) {
    let links: Vec<_> = document
        .select(&Selector::parse("a").unwrap())
        .map(|el| el.id())
        .collect();

    for id in links {
        let Some(mut node) = document.tree.get_mut(id) else { continue };
        if node.parent().is_none() {
            continue;
        }

        // reparent to same location as node
        while let Some(mut child) = node.first_child() {
            let child_id = child.id();
            child.detach();
            node.insert_id_before(child_id);
        }

        node.detach();
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn static_config_parses() {
        assert!(!CONFIG.sections_to_remove.is_empty());
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

        super::remove_links(&mut document);

        let links: Vec<_> = document.select(&anchors).collect();

        assert!(links.is_empty(), "All links should be removed.");

        assert!(
            document.select(&inner_element).next().is_some(),
            "Link inner elements should be preserved."
        );
    }
}
