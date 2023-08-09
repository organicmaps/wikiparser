#![feature(test)]
use std::{collections::HashSet, str::FromStr};

extern crate om_wikiparser;
extern crate test;

use om_wikiparser::wm::{Qid, Title};

const TITLE: &str = "https://en.wikipedia.org/wiki/Article_Title";
const QID: &str = "Q123456789";

#[bench]
fn parse_wikipedia(b: &mut test::Bencher) {
    b.iter(|| {
        Title::from_url(TITLE).unwrap();
    });
}

#[bench]
fn hash_wikipedia(b: &mut test::Bencher) {
    let title = Title::from_url(TITLE).unwrap();
    let mut set = HashSet::new();
    b.iter(|| {
        set.insert(&title);
    });
}

#[bench]
fn parse_wikidata(b: &mut test::Bencher) {
    b.iter(|| {
        Qid::from_str(QID).unwrap();
    });
}

#[bench]
fn hash_wikidata(b: &mut test::Bencher) {
    let qid = Qid::from_str(QID).unwrap();
    let mut set = HashSet::new();
    b.iter(|| {
        set.insert(&qid);
    });
}
