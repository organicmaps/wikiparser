# wikiparser

_Extracts articles from [Wikipedia database dumps](https://en.wikipedia.org/wiki/Wikipedia:Database_download) for embedding into the `mwm` map files created by [the Organic Maps generator](https://github.com/organicmaps/organicmaps/blob/master/tools/python/maps_generator/README.md)._

Extracted articles are identified by Wikipedia article titles in url or text form (language-specific), and [Wikidata QIDs](https://www.wikidata.org/wiki/Wikidata:Glossary#QID) (language-agnostic).
OpenStreetMap (OSM) commonly stores these as [`wikipedia*=`](https://wiki.openstreetmap.org/wiki/Key:wikipedia) and [`wikidata=`](https://wiki.openstreetmap.org/wiki/Key:wikidata) tags on objects.

## Configuring

[`article_processing_config.json`](article_processing_config.json) is _compiled with the program_ and should be updated when adding a new language.
It defines article sections that are not important for users and should be removed from the extracted HTML.
There are some tests for basic validation of the file, run them with `cargo test`.

## Usage

> [!NOTE]
> In production, wikiparser is run with the maps generator, which is somewhat involved to set up. See [Usage with Maps Generator](#usage-with-maps-generator) for more info.

To run the wikiparser for development and testing, see below.

First, install [the rust language tools](https://www.rust-lang.org/)

> [!IMPORTANT]
> For best performance, use `-r`/`--release` with `cargo build`/`run`.

You can run the program from within this directory using `cargo run --release --`.

Alternatively, build it with `cargo build --release`, which places the binary in `./target/release/om-wikiparser`.

Run the program with the `--help` flag to see all supported arguments.

```
$ cargo run -- --help
A set of tools to extract articles from Wikipedia Enterprise HTML dumps selected by OpenStreetMap tags.

Usage: om-wikiparser <COMMAND>

Commands:
  get-tags      Extract wikidata/wikipedia tags from an OpenStreetMap PBF dump
  check-tags    Attempt to parse extracted OSM tags and write errors to stdout in TSV format
  get-articles  Extract, filter, and simplify article HTML from Wikipedia Enterprise HTML dumps
  simplify      Apply html simplification to a single article
  help          Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

> [!NOTE]
> Each subcommand has additional help.

The main work is done in the `get-articles` subcommand.
It takes as inputs:
- A [Wikipedia Enterprise JSON dump](#downloading-wikipedia-dumps), decompressed and connected to `stdin`.
- A directory to write the extracted articles to, as a CLI argument.
- Any number of filters for the articles:
  - Use `--osm-tags` if you have an [OSM .pbf file](#downloading-openstreetmap-osm-files) and can use the `get-tags` subcommand or the `osmconvert` tool.
  - Use `--wikidata-qids` or `--wikipedia-urls` if you have a group of urls or QIDs from another source.

To test a single language in a specific map region, first get the matching tags for the region with `get-tags`:
```sh
cargo run -r -- get-tags $REGION_EXTRACT.pbf > region-tags.tsv
```

Then write the articles to a directory with `get-articles`:
```sh
tar xzOf $dump | cargo run -r -- get-articles --osm-tags region-tags.tsv $OUTPUT_DIR
```

## Downloading OpenStreetMap (OSM) files

To extract Wikipedia tags with the `get-tags` subcommand, you need a file in the [OSM `.pbf` format](https://wiki.openstreetmap.org/wiki/PBF_Format).

The "planet" file is [available directly from OSM](https://wiki.openstreetmap.org/wiki/Planet.osm) but is ~80GB in size; for testing you can [try a smaller region's data (called "Extracts") from one of the many providers](https://wiki.openstreetmap.org/wiki/Planet.osm#Extracts).

## Downloading Wikipedia Dumps

[Enterprise HTML dumps, updated twice a month, are publicly accessible](https://dumps.wikimedia.org/other/enterprise_html/).

> [!WARNING]
> Each language's dump is tens of gigabytes in size, and much larger when decompressed.
> To avoid storing the decompressed data, pipe it directly into the wikiparser as described in [Usage](#usage).

To test a small number of articles, you can also use the [On-Demand API](https://enterprise.wikimedia.com/docs/on-demand/) to download them, which has a free tier.

Wikimedia requests no more than 2 concurrent downloads, which the included [`download.sh`](./download.sh) script respects:
> If you are reading this on Wikimedia servers, please note that we have rate limited downloaders and we are capping the number of per-ip connections to 2.
> This will help to ensure that everyone can access the files with reasonable download times.
> Clients that try to evade these limits may be blocked.
> Our mirror sites do not have this cap.

See [the list of available mirrors](https://dumps.wikimedia.org/mirrors.html) for other options. Note that most of them do not include the enterprise dumps; check to see that the `other/enterprise_html/runs/` path includes subdirectories with files. The following two mirrors are known to include the enterprise html dumps as of August 2023:
- (US) https://dumps.wikimedia.your.org
- (Sweden) https://mirror.accum.se/mirror/wikimedia.org

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

## Usage with Maps Generator

To use with the [maps generator](https://github.com/organicmaps/organicmaps/blob/master/tools/python/maps_generator/README.md), see the [`run.sh` script](run.sh) and its own help documentation.
It handles extracting the tags, using multiple dumps, and re-running to convert titles to QIDs and extract them across languages.

As an example of manual usage with the maps generator:
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
  tar xzOf $dump | om-wikiparser get-articles \
    --wikidata-qids wikidata_qids.txt \
    --wikipedia-urls wikipedia_urls.txt \
    --write-new-qids new_qids.txt \
    descriptions/
done
# Extract discovered QIDs.
for dump in $DUMP_DOWNLOAD_DIR/*-ENTERPRISE-HTML.json.tar.gz
do
  tar xzOf $dump | om-wikiparser get-articles \
    --wikidata-qids new_qids.txt \
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
  tar xzOf $dump | om-wikiparser get-articles \
    --osm-tags osm_tags.tsv \
    --write-new-qids new_qids.txt \
    descriptions/
done
# Extract discovered QIDs.
for dump in $DUMP_DOWNLOAD_DIR/*-ENTERPRISE-HTML.json.tar.gz
do
  tar xzOf $dump | om-wikiparser get-articles \
    --wikidata-qids new_qids.txt \
    descriptions/
done
```
