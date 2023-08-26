fn main() {
    tonic_build::configure()
        .compile(&["proto/externalscaler.proto"], &["proto"])
        .unwrap();
}
