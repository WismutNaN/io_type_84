fn main() -> std::io::Result<()> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../src/shared/contracts/generated.ts");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, io_core::application::typescript_contracts())?;
    println!("Контракты обновлены: {}", path.display());
    Ok(())
}
