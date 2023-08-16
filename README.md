# wikiparser

_Extracts articles from [Wikipedia database dumps](https://en.wikipedia.org/wiki/Wikipedia:Database_download) for embedding into the `mwm` map files created by [the Organic Maps generator](https://github.com/organicmaps/organicmaps/blob/master/tools/python/maps_generator/README.md)._

Extracted articles are identified by Wikipedia article titles in url or text form (language-specific), and [Wikidata QIDs](https://www.wikidata.org/wiki/Wikidata:Glossary#QID) (language-agnostic).
OpenStreetMap commonly stores these as [`wikipedia*=`](https://wiki.openstreetmap.org/wiki/Key:wikipedia) and [`wikidata=`](https://wiki.openstreetmap.org/wiki/Key:wikidata) tags on objects.

## Configuring

[`article_processing_config.json`](article_processing_config.json) should be updated when adding a new language.
It defines article sections that are not important for users and should be removed from the extracted HTML.

## Downloading Dumps

[Enterprise HTML dumps, updated twice a month, are publicly accessible ](https://dumps.wikimedia.org/other/enterprise_html/).

For the wikiparser you'll want the ["NS0"](https://en.wikipedia.org/wiki/Wikipedia:Namespace) "ENTERPRISE-HTML" `.json.tar.gz` files.

They are gzipped tar files containing a single file of newline-delimited JSON matching the [Wikimedia Enterprise API schema](https://enterprise.wikimedia.com/docs/data-dictionary/).

The included [`download.sh`](./download.sh) script handles downloading the latest set of dumps in specific languages.
It maintains a directory with the following layout:
```
<DUMP_DIR>/
├── latest -> 20230701/
├── 20230701/
│  ├── dewiki-NS0-20230701-ENTERPRISE-HTML.json.tar.gz
│  ├── enwiki-NS0-20230701-ENTERPRISE-HTML.json.tar.gz
│  ├── eswiki-NS0-20230701-ENTERPRISE-HTML.json.tar.gz
│  ...
├── 20230620/
│  ├── dewiki-NS0-20230620-ENTERPRISE-HTML.json.tar.gz
│  ├── enwiki-NS0-20230620-ENTERPRISE-HTML.json.tar.gz
│  ├── eswiki-NS0-20230620-ENTERPRISE-HTML.json.tar.gz
│  ...
...
```

## Usage

To use with the map generator, see the [`run.sh` script](run.sh) and its own help documentation.
It handles extracting the tags, using multiple dumps, and re-running to convert titles to QIDs and extract them across languages.

To run the wikiparser manually or for development, see below.

First, install [the rust language tools](https://www.rust-lang.org/)

For best performance, use `--release` when building or running.

You can run the program from within this directory using `cargo run --release --`.

Alternatively, build it with `cargo build --release`, which places the binary in `./target/release/om-wikiparser`.

Run the program with the `--help` flag to see all supported arguments.

```
$ cargo run --release -- --help
Extract articles from Wikipedia Enterprise HTML dumps

Usage: om-wikiparser <COMMAND>

Commands:
  get-articles  Extract, filter, and simplify article HTML from Wikipedia Enterprise HTML dumps
  get-tags      Extract wikidata/wikipedia tags from an OpenStreetMap PBF dump
  simplify      Apply the same html article simplification used when extracting articles to stdin, and write it to stdout
  help          Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help (see more with '--help')
  -V, --version  Print version
```

Each command has its own additional help:

```
$ cargo run -- get-articles --help
Extract, filter, and simplify article HTML from Wikipedia Enterprise HTML dumps.

Expects an uncompressed dump (newline-delimited JSON) connected to stdin.

Usage: om-wikiparser get-articles [OPTIONS] <OUTPUT_DIR>

Arguments:
  <OUTPUT_DIR>
          Directory to write the extracted articles to

Options:
      --write-new-qids <FILE>
          Append to the provided file path the QIDs of articles matched by title but not QID.

          Use this to save the QIDs of articles you know the url of, but not the QID. The same path can later be passed to the `--wikidata-qids` option to extract them from another language's dump. Writes are atomicly appended to the file, so the same path may be used by multiple concurrent instances.

  -h, --help
          Print help (see a summary with '-h')

FILTERS:
      --osm-tags <FILE.tsv>
          Path to a TSV file that contains one or more of `wikidata`, `wikipedia` columns.

          This can be generated with the `get-tags` command or `osmconvert --csv-headline --csv 'wikidata wikipedia'`.

      --wikidata-qids <FILE>
          Path to file that contains a Wikidata QID to extract on each line (e.g. `Q12345`)

      --wikipedia-urls <FILE>
          Path to file that contains a Wikipedia article url to extract on each line (e.g. `https://lang.wikipedia.org/wiki/Article_Title`)
```

It takes as inputs:
- A wikidata enterprise JSON dump, extracted and connected to `stdin`.
- A directory to write the extracted articles to, as a CLI argument.
- Any number of filters passed:
  - A TSV file of wikidata qids and wikipedia urls, created by the `get-tags` command or `osmconvert`, passed as the CLI flag `--osm-tags`.
  - A file of Wikidata QIDs to extract, one per line (e.g. `Q12345`), passed as the CLI flag `--wikidata-ids`.
  - A file of Wikipedia article titles to extract, one per line (e.g. `https://$LANG.wikipedia.org/wiki/$ARTICLE_TITLE`), passed as a CLI flag `--wikipedia-urls`.

As an example of manual usage with the map generator:
- Assuming this program is installed to `$PATH` as `om-wikiparser`.
- Download [the dumps in the desired languages](https://dumps.wikimedia.org/other/enterprise_html/runs/) (Use the files with the format `${LANG}wiki-NS0-${DATE}-ENTERPRISE-HTML.json.tar.gz`).
  Set `DUMP_DOWNLOAD_DIR` to the location they are downloaded.
- Run a maps build with descriptions enabled to generate the `id_to_wikidata.csv` and `wiki_urls.txt` files.
- Run the following from within the `intermediate_data` subdirectory of the maps build directory:

```shell
# Transform intermediate files from generator.
cut -f 2 id_to_wikidata.csv > wikidata_qids.txt
tail -n +2 wiki_urls.txt | cut -f 3 > wikipedia_urls.txt
# Enable backtraces in errors and panics.
export RUST_BACKTRACE=1
# Set log level to debug
export RUST_LOG=om_wikiparser=debug
# Begin extraction.
for dump in $DUMP_DOWNLOAD_DIR/*-ENTERPRISE-HTML.json.tar.gz
do
  tar xzf $dump | om-wikiparser get-articles \
    --wikidata-ids wikidata_qids.txt \
    --wikipedia-urls wikipedia_urls.txt \
    --write-new-qids new_qids.txt \
    descriptions/
done
# Extract discovered QIDs.
for dump in $DUMP_DOWNLOAD_DIR/*-ENTERPRISE-HTML.json.tar.gz
do
  tar xzf $dump | om-wikiparser get-articles \
    --wikidata-ids new_qids.txt \
    descriptions/
done
```

Alternatively, extract the tags directly from a `.osm.pbf` file (referenced here as `planet-latest.osm.pbf`):
```shell
# Extract tags
om-wikiparser get-tags planet-latest.osm.pbf > osm_tags.tsv
# Begin extraction.
for dump in $DUMP_DOWNLOAD_DIR/*-ENTERPRISE-HTML.json.tar.gz
do
  tar xzf $dump | om-wikiparser get-articles \
    --osm-tags osm_tags.tsv \
    --write-new-qids new_qids.txt \
    descriptions/
done
# Extract discovered QIDs.
for dump in $DUMP_DOWNLOAD_DIR/*-ENTERPRISE-HTML.json.tar.gz
do
  tar xzf $dump | om-wikiparser get-articles \
    --wikidata-ids new_qids.txt \
    descriptions/
done
```
