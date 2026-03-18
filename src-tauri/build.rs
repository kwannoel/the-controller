fn main() {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    println!("cargo:rustc-env=BUILD_DATE={}", today);

    tauri_build::build()
}
