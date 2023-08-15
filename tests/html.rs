//! Tests to check for changes in HTML output.
//!
//! To update the expected output, run the test again with the env variable
//! `UPDATE_EXPECT=1` set.
//! See https://docs.rs/expect-test/ for more information.
use om_wikiparser::html::{pretty_print, simplify_html};

use expect_test::{expect_file, ExpectFile};
use scraper::Html;

fn check(input: &str, expect: ExpectFile) {
    let mut html = Html::parse_document(input);
    simplify_html(&mut html, "en");
    let processed = pretty_print(&html);

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
