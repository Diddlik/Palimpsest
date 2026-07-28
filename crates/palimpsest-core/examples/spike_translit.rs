//! Throwaway diagnostic: does the "convert selection" transliteration produce a
//! different string for a given layout pair on THIS machine's installed layouts?
//!
//! It exercises exactly the seam the selection hotkey uses (`suggest_text` over
//! the `Transliterator`), so a `None`/unchanged result here is the same silent
//! no-op a user sees as "the shortcut did nothing".
//!
//! Run: `cargo run -p palimpsest-core --example spike_translit -- 00000407 a0000419 privet`

use palimpsest_core::analyze::Transliterator; // brings `.transliterate` into scope
use palimpsest_core::reinterpret::WinReinterpreter;
use palimpsest_core::{suggest_text, LayoutPair, Models};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let from = args.first().cloned().unwrap_or_else(|| "00000407".into());
    let to = args.get(1).cloned().unwrap_or_else(|| "a0000419".into());
    let samples: Vec<String> = if args.len() > 2 {
        args[2..].to_vec()
    } else {
        vec!["privet".into(), "ghbdtn".into(), "привет".into()]
    };

    let reinterp = WinReinterpreter::new();
    let models = Models::builtin();
    let pairs = vec![LayoutPair { from: from.clone(), to: to.clone(), phonetic: Some(true) }];

    println!("pair: {from}  <->  {to}\n");
    for s in &samples {
        // Raw transliteration in both directions, then the actual selection verdict.
        let fwd = reinterp.transliterate(s, &from, &to);
        let rev = reinterp.transliterate(s, &to, &from);
        let verdict = suggest_text(s, &pairs, &reinterp, &models);
        println!("  input   : {s:?}");
        println!("  {from}->{to} : {fwd:?}");
        println!("  {to}->{from} : {rev:?}");
        match verdict {
            Some(c) => println!("  SELECTION -> {:?}  (switch to {})\n", c.text, c.target_layout),
            None => println!("  SELECTION -> None  (hotkey would do nothing)\n"),
        }
    }
}
