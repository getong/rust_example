fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        // .build_client(false)
        .compile(&["proto/todo.proto"], &["proto"])
        .unwrap();
    Ok(())
}