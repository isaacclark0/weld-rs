// Rebuild when migrations change, since they are embedded into the binary
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
