#![cfg(all(feature = "ui-test", target_arch = "wasm32"))]

use crate::api::{self, AppBoot, UiTestMockState};
use otvi_core::types::{
    AuthFlowInfo, Category, CategoryListResponse, Channel, ChannelListResponse, FieldInfo,
    ProviderInfo, ServerSettings, StreamInfo, StreamType, UserInfo, UserRole,
};
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

pub mod fixtures {
    use super::*;

    pub fn user(role: UserRole, must_change_password: bool) -> UserInfo {
        UserInfo {
            id: "u-1".to_string(),
            username: "tester".to_string(),
            role,
            providers: vec!["provider-a".to_string()],
            must_change_password,
        }
    }

    pub fn providers() -> Vec<ProviderInfo> {
        vec![ProviderInfo {
            id: "provider-a".to_string(),
            name: "Provider A".to_string(),
            logo: None,
            auth_flows: vec![AuthFlowInfo {
                id: "password".to_string(),
                name: "Password".to_string(),
                fields: vec![
                    FieldInfo {
                        key: "username".to_string(),
                        label: "Username".to_string(),
                        field_type: "text".to_string(),
                        required: true,
                    },
                    FieldInfo {
                        key: "password".to_string(),
                        label: "Password".to_string(),
                        field_type: "password".to_string(),
                        required: true,
                    },
                ],
            }],
        }]
    }

    pub fn channels() -> ChannelListResponse {
        ChannelListResponse {
            channels: vec![Channel {
                id: "channel-1".to_string(),
                name: "News One".to_string(),
                logo: None,
                category: Some("news".to_string()),
                number: Some("101".to_string()),
                description: None,
            }],
            total: Some(1),
        }
    }

    pub fn categories() -> CategoryListResponse {
        CategoryListResponse {
            categories: vec![Category {
                id: "news".to_string(),
                name: "News".to_string(),
            }],
        }
    }

    pub fn stream() -> StreamInfo {
        StreamInfo {
            url: "https://example.test/live.m3u8".to_string(),
            stream_type: StreamType::Hls,
            drm: None,
            channel_name: Some("News One".to_string()),
            channel_logo: None,
        }
    }

    pub fn settings() -> ServerSettings {
        ServerSettings {
            signup_disabled: false,
        }
    }
}

// ---------------------------------------------------------------------------
// DOM helpers
// ---------------------------------------------------------------------------

fn window() -> web_sys::Window {
    web_sys::window().expect("window must exist in browser test")
}

fn document() -> web_sys::Document {
    window()
        .document()
        .expect("document must exist in browser test")
}

fn set_path(path: &str) {
    let history = window().history().expect("history should be available");
    history
        .push_state_with_url(&JsValue::NULL, "", Some(path))
        .expect("pushState should work in tests");
}

fn pathname() -> String {
    window()
        .location()
        .pathname()
        .expect("pathname should be readable")
}

fn has_testid(test_id: &str) -> bool {
    document()
        .query_selector(&format!("[data-testid='{test_id}']"))
        .expect("query selector should succeed")
        .is_some()
}

fn text_for_testid(test_id: &str) -> String {
    document()
        .query_selector(&format!("[data-testid='{test_id}']"))
        .expect("query selector should succeed")
        .and_then(|el| el.text_content())
        .unwrap_or_default()
}

fn click_testid(test_id: &str) {
    let el = document()
        .query_selector(&format!("[data-testid='{test_id}']"))
        .expect("query selector should succeed")
        .expect("element should exist");
    let target: web_sys::HtmlElement = el
        .dyn_into()
        .expect("data-testid element should be clickable html element");
    target.click();
}

fn click_selector(selector: &str) {
    let el = document()
        .query_selector(selector)
        .expect("query selector should succeed")
        .expect("element should exist");
    let target: web_sys::HtmlElement = el
        .dyn_into()
        .expect("selected element should be clickable html element");
    target.click();
}

async fn sleep_ms(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

#[allow(dead_code)]
async fn settle() {
    sleep_ms(100).await;
}

/// Returns the innerHTML of the `#app` container (useful for diagnostic messages).
fn app_html() -> String {
    document()
        .get_element_by_id("app")
        .map(|el| el.inner_html())
        .unwrap_or_else(|| "(#app not found)".into())
}

/// Poll the DOM for up to `timeout_ms` until `[data-testid='id']` appears.
/// Returns `true` if found, `false` on timeout.
async fn wait_for_testid(test_id: &str, timeout_ms: i32) -> bool {
    let step = 25;
    let mut elapsed = 0;
    while elapsed < timeout_ms {
        if has_testid(test_id) {
            return true;
        }
        sleep_ms(step).await;
        elapsed += step;
    }
    has_testid(test_id)
}

/// Poll the DOM for up to `timeout_ms` until `[data-testid='id']` is gone.
/// Returns `true` if absent, `false` on timeout.
#[allow(dead_code)]
async fn wait_for_no_testid(test_id: &str, timeout_ms: i32) -> bool {
    let step = 25;
    let mut elapsed = 0;
    while elapsed < timeout_ms {
        if !has_testid(test_id) {
            return true;
        }
        sleep_ms(step).await;
        elapsed += step;
    }
    !has_testid(test_id)
}

fn install_player_stubs() {
    js_sys::eval(
        "window.otviInitHls = window.otviInitHls || function(){};
         window.otviInitDash = window.otviInitDash || function(){};
         window.otviDestroyPlayer = window.otviDestroyPlayer || function(){};",
    )
    .expect("player bridge stubs should install");
}

// ---------------------------------------------------------------------------
// Test lifecycle helpers
// ---------------------------------------------------------------------------

/// Prepare a fresh DOM and mock state for a sub-test scenario. Returns the
/// `#app` container element that should be passed to [`mount`].
fn setup(path: &str, state: UiTestMockState) {
    api::clear_ui_test_mock_state();
    api::clear_token();

    // Remove old #app container if it exists, but preserve the
    // wasm-bindgen-test-runner harness elements (e.g. #logs).
    let doc = document();
    if let Some(old) = doc.get_element_by_id("app") {
        old.remove();
    }

    // Create a fresh #app container for the Leptos mount.
    let container = doc
        .create_element("div")
        .expect("create_element should work");
    container.set_id("app");
    doc.body()
        .expect("body must exist")
        .append_child(&container)
        .expect("append_child should work");

    set_path(path);
    api::set_ui_test_mock_state(state);
}

/// Mount the Leptos app into `#app`. Returns an opaque handle; dropping it
/// tears down the reactive system so the next sub-test starts clean.
fn mount() -> Box<dyn std::any::Any> {
    console_error_panic_hook::set_once();
    let container = document()
        .get_element_by_id("app")
        .expect("#app must exist after setup()")
        .unchecked_into::<web_sys::HtmlElement>();
    let handle = leptos::mount::mount_to(container, crate::app::App);
    Box::new(handle)
}

/// Tear down the current sub-test: drop the reactive system, remove the app
/// container, and clear mock state.
fn teardown(handle: Box<dyn std::any::Any>) {
    drop(handle);
    set_path("/");
    if let Some(old) = document().get_element_by_id("app") {
        old.remove();
    }
    api::clear_ui_test_mock_state();
    api::clear_token();
}

// ---------------------------------------------------------------------------
// All UI scenarios in a single sequential test
// ---------------------------------------------------------------------------
//
// wasm_bindgen_test runs async tests *concurrently* in the same browser page.
// Because every scenario needs exclusive access to the DOM and the History API,
// we run them sequentially inside one top-level test function.
// ---------------------------------------------------------------------------

#[wasm_bindgen_test(async)]
async fn ui_scenarios() {
    install_player_stubs();

    // Maximum time (ms) to wait for a testid to appear.
    const WAIT: i32 = 2000;

    // ── 1. Setup gate ────────────────────────────────────────────────
    {
        setup(
            "/",
            UiTestMockState {
                boot: Some(AppBoot::NeedsSetup),
                ..Default::default()
            },
        );
        let h = mount();
        assert!(
            wait_for_testid("setup-overlay", WAIT).await,
            "setup-overlay should render. #app HTML: {}",
            app_html()
        );
        assert!(has_testid("setup-page"), "setup-page should render");
        teardown(h);
    }

    // ── 2. Login gate ────────────────────────────────────────────────
    {
        setup(
            "/",
            UiTestMockState {
                boot: Some(AppBoot::NeedsLogin),
                ..Default::default()
            },
        );
        let h = mount();
        assert!(
            wait_for_testid("login-overlay", WAIT).await,
            "login-overlay should render. #app HTML: {}",
            app_html()
        );
        assert!(has_testid("app-login-page"), "app-login-page should render");
        teardown(h);
    }

    // ── 3. Forced password change gate ───────────────────────────────
    {
        setup(
            "/",
            UiTestMockState {
                boot: Some(AppBoot::Ready(fixtures::user(UserRole::User, true))),
                ..Default::default()
            },
        );
        let h = mount();
        assert!(
            wait_for_testid("forced-password-overlay", WAIT).await,
            "forced-password-overlay should render. #app HTML: {}",
            app_html()
        );
        assert!(
            has_testid("forced-change-password-page"),
            "forced-change-password-page should render"
        );
        teardown(h);
    }

    // ── 4. Authenticated home shell ──────────────────────────────────
    {
        setup(
            "/",
            UiTestMockState {
                boot: Some(AppBoot::Ready(fixtures::user(UserRole::User, false))),
                providers: Some(Ok(fixtures::providers())),
                ..Default::default()
            },
        );
        let h = mount();
        assert!(
            wait_for_testid("app-shell-nav", WAIT).await,
            "app-shell-nav should render. #app HTML: {}",
            app_html()
        );
        assert!(has_testid("home-page"), "home-page should render");
        teardown(h);
    }

    // ── 5. Admin route for admin user ────────────────────────────────
    {
        setup(
            "/admin",
            UiTestMockState {
                boot: Some(AppBoot::Ready(fixtures::user(UserRole::Admin, false))),
                providers: Some(Ok(fixtures::providers())),
                admin_users: Some(Ok(vec![fixtures::user(UserRole::Admin, false)])),
                settings: Some(Ok(fixtures::settings())),
                ..Default::default()
            },
        );
        let h = mount();
        assert!(
            wait_for_testid("admin-page", WAIT).await,
            "admin-page should render. #app HTML: {}",
            app_html()
        );
        teardown(h);
    }

    // ── 6. Provider login route ──────────────────────────────────────
    {
        setup(
            "/login/provider-a",
            UiTestMockState {
                boot: Some(AppBoot::Ready(fixtures::user(UserRole::User, false))),
                provider: Some(Ok(fixtures::providers()[0].clone())),
                provider_session_valid: Some(false),
                ..Default::default()
            },
        );
        let h = mount();
        assert!(
            wait_for_testid("provider-login-page", WAIT).await,
            "provider-login-page should render. #app HTML: {}",
            app_html()
        );
        teardown(h);
    }

    // ── 7. Not-found route ───────────────────────────────────────────
    {
        setup(
            "/404-missing",
            UiTestMockState {
                boot: Some(AppBoot::Ready(fixtures::user(UserRole::User, false))),
                providers: Some(Ok(fixtures::providers())),
                ..Default::default()
            },
        );
        let h = mount();
        assert!(
            wait_for_testid("not-found-page", WAIT).await,
            "not-found-page should render. #app HTML: {}",
            app_html()
        );
        teardown(h);
    }

    // ── 8. Admin dashboard link visible for admin ────────────────────
    {
        setup(
            "/",
            UiTestMockState {
                boot: Some(AppBoot::Ready(fixtures::user(UserRole::Admin, false))),
                providers: Some(Ok(fixtures::providers())),
                ..Default::default()
            },
        );
        let h = mount();
        assert!(
            wait_for_testid("admin-dashboard-link", WAIT).await,
            "admin-dashboard-link should be visible for admin. #app HTML: {}",
            app_html()
        );
        teardown(h);
    }

    // ── 9. Admin dashboard link hidden for non-admin ─────────────────
    {
        setup(
            "/",
            UiTestMockState {
                boot: Some(AppBoot::Ready(fixtures::user(UserRole::User, false))),
                providers: Some(Ok(fixtures::providers())),
                ..Default::default()
            },
        );
        let h = mount();
        // Wait for the shell to render first, then verify admin link is absent.
        assert!(
            wait_for_testid("app-shell-nav", WAIT).await,
            "app-shell-nav should render for non-admin. #app HTML: {}",
            app_html()
        );
        assert!(
            !has_testid("admin-dashboard-link"),
            "admin-dashboard-link should NOT be visible for non-admin"
        );
        teardown(h);
    }

    // ── 10. Voluntary password change overlay ────────────────────────
    {
        setup(
            "/",
            UiTestMockState {
                boot: Some(AppBoot::Ready(fixtures::user(UserRole::User, false))),
                providers: Some(Ok(fixtures::providers())),
                ..Default::default()
            },
        );
        let h = mount();
        assert!(
            wait_for_testid("open-change-password-button", WAIT).await,
            "open-change-password-button should render. #app HTML: {}",
            app_html()
        );
        click_testid("open-change-password-button");
        assert!(
            wait_for_testid("voluntary-password-overlay", WAIT).await,
            "voluntary-password-overlay should render. #app HTML: {}",
            app_html()
        );
        assert!(
            has_testid("voluntary-change-password-page"),
            "voluntary-change-password-page should render"
        );
        teardown(h);
    }

    // ── 11. Sign-out returns to login gate ───────────────────────────
    {
        setup(
            "/",
            UiTestMockState {
                boot: Some(AppBoot::Ready(fixtures::user(UserRole::User, false))),
                providers: Some(Ok(fixtures::providers())),
                ..Default::default()
            },
        );
        let h = mount();
        assert!(
            wait_for_testid("sign-out-button", WAIT).await,
            "sign-out-button should exist. #app HTML: {}",
            app_html()
        );
        click_testid("sign-out-button");
        assert!(
            wait_for_testid("login-overlay", WAIT).await,
            "login-overlay should appear after sign-out. #app HTML: {}",
            app_html()
        );
        teardown(h);
    }

    // ── 12. In-app SPA navigation ────────────────────────────────────
    {
        setup(
            "/admin",
            UiTestMockState {
                boot: Some(AppBoot::Ready(fixtures::user(UserRole::Admin, false))),
                providers: Some(Ok(fixtures::providers())),
                admin_users: Some(Ok(vec![fixtures::user(UserRole::Admin, false)])),
                settings: Some(Ok(fixtures::settings())),
                ..Default::default()
            },
        );
        let h = mount();
        assert!(
            wait_for_testid("admin-page", WAIT).await,
            "admin-page should render before navigation. #app HTML: {}",
            app_html()
        );
        assert_eq!(pathname(), "/admin");
        click_testid("app-logo-link");
        assert!(
            wait_for_testid("home-page", WAIT).await,
            "home-page should render after navigation. #app HTML: {}",
            app_html()
        );
        assert_eq!(pathname(), "/");
        assert!(
            has_testid("app-shell-nav"),
            "app-shell-nav should persist after navigation"
        );
        teardown(h);
    }

    // ── 13. Channel → Player navigation ──────────────────────────────
    {
        setup(
            "/providers/provider-a/channels",
            UiTestMockState {
                boot: Some(AppBoot::Ready(fixtures::user(UserRole::User, false))),
                provider_session_valid: Some(true),
                channels: Some(Ok(fixtures::channels())),
                categories: Some(Ok(fixtures::categories())),
                stream: Some(Ok(fixtures::stream())),
                ..Default::default()
            },
        );
        let h = mount();
        assert!(
            wait_for_testid("channels-page", WAIT).await,
            "channels-page should render. #app HTML: {}",
            app_html()
        );
        click_selector("[title='News One']");
        assert!(
            wait_for_testid("player-channel-name", WAIT).await,
            "player-channel-name should render. #app HTML: {}",
            app_html()
        );
        assert_eq!(pathname(), "/providers/provider-a/play/channel-1");
        assert_eq!(text_for_testid("player-channel-name"), "News One");
        teardown(h);
    }
}
