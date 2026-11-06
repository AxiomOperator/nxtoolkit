// Re-embed the catalog whenever it changes.
fn main() {
    println!("cargo:rerun-if-changed=../../catalog");
}
