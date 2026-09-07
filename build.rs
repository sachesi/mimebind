fn main() {
    // option_env! bakes this in, so a prefix change has to force a rebuild.
    println!("cargo:rerun-if-env-changed=MIMEBIND_LOCALEDIR");
}
