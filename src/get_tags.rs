use std::{
    io::{stdout, Read},
    sync::mpsc,
    thread,
};

use osmpbf::{BlobDecode, BlobReader, Element};
use rayon::prelude::*;

struct Record {
    id: String,
    wikidata: String,
    wikipedia: String,
}

/// Extract matching tags from an osm pbf file and write to stdout in TSV.
pub fn run(pbf: impl Read + Send) -> anyhow::Result<()> {
    let reader = BlobReader::new(pbf);

    let (send, recv) = mpsc::sync_channel(128);
    let writer_thread = thread::Builder::new()
        .name("writer".to_string())
        .spawn(move || write(recv))?;

    reader
        .par_bridge()
        .try_for_each(move |blob| -> anyhow::Result<()> {
            // Based on `osmpbf` implementation of `ElementReader`.
            let BlobDecode::OsmData(block) = blob?.decode()? else {
                return Ok(());
            };
            for record in block.elements().filter_map(extract_tags) {
                send.send(record)?;
            }
            Ok(())
        })?;

    let record_count = writer_thread.join().unwrap()?;
    info!("Finished processing {record_count} records");

    Ok(())
}

fn write(recv: mpsc::Receiver<Record>) -> anyhow::Result<usize> {
    let mut output = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .from_writer(stdout().lock());
    output.write_record(["@id", "wikidata", "wikipedia"])?;

    let mut count = 0;

    for Record {
        id,
        wikidata,
        wikipedia,
    } in recv
    {
        output.write_record([id, wikidata, wikipedia])?;
        count += 1;
    }

    Ok(count)
}

fn extract_tags(el: Element) -> Option<Record> {
    match el {
        Element::Node(n) => make_record(n.id(), n.tags()),
        Element::DenseNode(n) => make_record(n.id(), n.tags()),
        Element::Way(w) => make_record(w.id(), w.tags()),
        Element::Relation(r) => make_record(r.id(), r.tags()),
    }
}

fn make_record<'i>(id: i64, tags: impl 'i + Iterator<Item = (&'i str, &'i str)>) -> Option<Record> {
    let mut wikipedia = String::new();
    let mut wikidata = String::new();

    for (key, value) in tags {
        match key {
            "wikipedia" => wikipedia = value.trim().to_owned(),
            "wikidata" => wikidata = value.trim().to_owned(),
            _ => {}
        }
    }

    if wikidata.is_empty() && wikipedia.is_empty() {
        return None;
    }

    Some(Record {
        id: id.to_string(),
        wikipedia,
        wikidata,
    })
}
