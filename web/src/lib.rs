pub mod api;
pub mod app;
pub mod pages;

#[cfg(all(feature = "ui-test", target_arch = "wasm32"))]
pub mod ui_test;

pub fn mount_app() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
