fn main() -> Result<(), std::io::Error> {
    tonic_build::configure()
        .out_dir("src/lib/proto")
        .compile(&["proto/dataloader.proto"], &["proto"])
}
