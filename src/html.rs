use scraper::{ElementRef, Html, Selector};

pub fn simplify(html: &str) -> String {
    // TODO: handle multiple languages
    let bad_sections = [
        "External links",
        "Sources",
        "See also",
        "Bibliography",
        "Further reading",
        "References",
    ];

    let mut document = Html::parse_document(html);

    // TODO: evaluate this only once
    let headers = Selector::parse("h1, h2, h3, h4, h5, h6, h7").unwrap();

    let mut to_remove = Vec::new();

    // remove sections
    for header in document.select(&headers) {
        // TODO: should this join all text nodes?
        let Some(title) = header.text().next() else {
            continue
        };
        if bad_sections.contains(&title) {
            to_remove.push(header.id());
            let header_level = header.value().name();
            // strip trailing nodes
            for sibling in header.next_siblings() {
                if let Some(element) = sibling.value().as_element() {
                    if element.name() == header_level {
                        // TODO: should this check for a higher level?
                        break;
                    }
                }
                to_remove.push(sibling.id());
            }
        }
    }

    for id in to_remove.drain(..) {
        if let Some(mut node) = document.tree.get_mut(id) {
            node.detach();
        }
    }

    // remove elements with no text that isn't whitespace

    for element in document
        .root_element()
        .descendants()
        .filter_map(ElementRef::wrap)
    {
        if element.text().all(|t| t.trim().is_empty()) {
            to_remove.push(element.id());
        }
    }

    for id in to_remove.drain(..) {
        if let Some(mut node) = document.tree.get_mut(id) {
            node.detach();
        }
    }

    document.html()
}
