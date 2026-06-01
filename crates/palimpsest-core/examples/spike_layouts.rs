//! Phase 5/6 helper: print the installed keyboard layouts the settings UI would
//! offer, plus the derived default pair and a sanitize dry-run. Headless — safe
//! for Claude to run.

fn main() {
    let installed = palimpsest_core::layouts::installed();
    println!("installed layouts ({}):", installed.len());
    for l in &installed {
        println!("  {} — {}", l.klid, l.name);
    }
    println!("default_pair = {:?}", palimpsest_core::layouts::default_pair(&installed));

    let mut config = palimpsest_core::Config::default();
    println!("config pairs before = {:?}", config.layout_pairs);
    let changed = palimpsest_core::layouts::sanitize_pairs(&mut config, &installed);
    println!("changed = {changed}");
    println!("config pairs after  = {:?}", config.layout_pairs);
}
