fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(&["proto/txn.proto"], &["proto"])?;
    println!("cargo:rerun-if-changed=proto/txn.proto");
    Ok(())
}

