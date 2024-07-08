//! Tests to check for changes in HTML output.
//!
//! To update the expected output, run the test again with the env variable
//! `UPDATE_EXPECT=1` set.
//! See https://docs.rs/expect-test/ for more information.
use om_wikiparser::html::{detect_lang, process, process_str, HtmlError};

use expect_test::{expect_file, ExpectFile};
use scraper::Html;

fn check(input: &str, expect: ExpectFile) {
    let html = Html::parse_document(input);
    let lang = detect_lang(&html).unwrap();
    let html = process(html, &lang).unwrap();
    let processed = html.html();

    expect.assert_eq(&processed);
}

#[test]
fn simplify_crimean_mountains() {
    check(
        include_str!("./data/Q748282-en/original.html"),
        expect_file!["./data/Q748282-en/output.html"],
    );
}

#[test]
fn simplify_thoor_ballylee() {
    check(
        include_str!("./data/Q4185820-en/original.html"),
        expect_file!["./data/Q4185820-en/output.html"],
    );
}

#[test]
fn not_redirect_crimean_mountains() {
    let article = include_str!("./data/Q748282-en/original.html");
    assert!(process_str(article, "en").is_ok());
}

#[test]
fn not_redirect_thoor_ballylee() {
    let article = include_str!("./data/Q4185820-en/original.html");
    assert!(process_str(article, "en").is_ok());
}

#[test]
fn is_redirect_abdalcık_aşkale() {
    let article = include_str!("./data/redirects/Abdalc%C4%B1k%2C%20A%C5%9Fkale.html");
    assert_eq!(
        Err(HtmlError::Redirect("Aşkale".into())),
        process_str(article, "en")
    );
}

#[test]
fn is_empty_bahnstrecke_bassum_herford() {
    let article = include_str!("./data/redirects/Bahnstrecke%20Bassum%FF%FF%FFHerford.html");
    assert_eq!(Err(HtmlError::NoText), process_str(article, "en"));
}
