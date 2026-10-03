//! `cargo run -p kintree-core --release --example synth -- <persons> <seed> > out.ged`
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n: usize = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(30);
    let seed: u64 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    print!("{}", kintree_core::synth::generate_gedcom(n, seed));
}
