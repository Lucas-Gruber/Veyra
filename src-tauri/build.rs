fn main() {
    let manifest = tauri_build::AppManifest::new().commands(&[
        "app_info",
        "check_for_updates",
        "open_release_page",
        "session_status",
        "login_vault",
        "list_metadata",
        "sync_vault",
        "unlock_demo",
        "lock_vault",
        "show_quick_access",
        "hide_quick_access",
        "copy_field",
        "reveal_field",
        "open_website",
        "open_main_item",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("Tauri build configuration failed");
}
