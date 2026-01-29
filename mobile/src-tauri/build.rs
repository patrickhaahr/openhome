fn main() {
    // Load .env file to get VAULT_PASSWORD
    if let Err(e) = dotenvy::from_path("../.env") {
        println!("cargo:warning=Could not load .env file: {}", e);
    }

    // Pass VAULT_PASSWORD to the compiler so std::env! can see it
    if let Ok(password) = std::env::var("VAULT_PASSWORD") {
        println!("cargo:rustc-env=VAULT_PASSWORD={}", password);
    } else {
        panic!("VAULT_PASSWORD must be set in .env file for vault compilation");
    }

    tauri_build::build()
}
