use wasm_bindgen::JsCast;

pub mod api;
pub mod app;
pub mod pages;

#[cfg(all(feature = "ui-test", target_arch = "wasm32"))]
pub mod ui_test;

/// Mount the app to the `#app` container element (production entrypoint).
pub fn mount_app() {
    console_error_panic_hook::set_once();
    let document = web_sys::window()
        .expect("window should exist")
        .document()
        .expect("document should exist");
    let container = document
        .get_element_by_id("app")
        .expect("#app element should exist in index.html")
        .unchecked_into::<web_sys::HtmlElement>();
    leptos::mount::mount_to(container, app::App).forget();
}

/// Mount the app to an arbitrary parent element (used by tests).
pub fn mount_app_to(parent: web_sys::HtmlElement) {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to(parent, app::App).forget();
}
