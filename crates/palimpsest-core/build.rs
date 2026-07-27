//! Compiles the plain-text frequency wordlists under `wordlists/` into finite
//! state transducers in `OUT_DIR`, which `scoring.rs` embeds with
//! `include_bytes!`. Doing it here keeps the readable, diffable word lists in
//! the repository while shipping the compact lookup structure in the binary.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::{env, fs};

fn main() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR set by cargo");
    for lang in ["en", "de", "ru"] {
        let src = format!("wordlists/{lang}.txt");
        println!("cargo:rerun-if-changed={src}");

        let text = fs::read_to_string(&src).unwrap_or_else(|e| panic!("read {src}: {e}"));
        // `fst` requires keys inserted in byte-lexicographic order, which is
        // exactly Rust's `str` ordering — the source lists stay frequency-sorted.
        let mut words: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
        words.sort_unstable();
        words.dedup();

        let path = Path::new(&out_dir).join(format!("{lang}.fst"));
        let file = File::create(&path).unwrap_or_else(|e| panic!("create {path:?}: {e}"));
        let mut builder = fst::SetBuilder::new(BufWriter::new(file)).expect("fst builder");
        for word in words {
            builder.insert(word).expect("insert word");
        }
        builder.into_inner().expect("finish fst").flush().expect("flush fst");
    }
}
