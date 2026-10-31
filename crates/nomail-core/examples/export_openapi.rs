fn main() -> Result<(), Box<dyn std::error::Error>> {
    let document = nomail_core::api::openapi::document();
    let text = serde_json::to_string_pretty(&document)? + "\n";
    if let Some(path) = std::env::args().nth(1) {
        std::fs::write(path, text)?;
    } else {
        print!("{text}");
    }
    Ok(())
}
