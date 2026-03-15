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

async fn settle() {
    sleep_ms(20).await;
}

fn install_player_stubs() {
    js_sys::eval(
        "window.otviInitHls = window.otviInitHls || function(){};
         window.otviInitDash = window.otviInitDash || function(){};
         window.otviDestroyPlayer = window.otviDestroyPlayer || function(){};",
    )
    .expect("player bridge stubs should install");
}

fn reset_dom_and_state(path: &str, state: UiTestMockState) {
    api::clear_ui_test_mock_state();
    api::clear_token();
    document()
        .body()
        .expect("body must exist")
        .set_inner_html("");
    set_path(path);
    api::set_ui_test_mock_state(state);
}

fn mount() {
    crate::mount_app();
}

#[wasm_bindgen_test(async)]
async fn renders_setup_gate_when_boot_needs_setup() {
    reset_dom_and_state(
        "/",
        UiTestMockState {
            boot: Some(AppBoot::NeedsSetup),
            ..Default::default()
        },
    );

    mount();
    settle().await;

    assert!(has_testid("setup-overlay"));
    assert!(has_testid("setup-page"));
}

#[wasm_bindgen_test(async)]
async fn renders_login_gate_when_boot_needs_login() {
    reset_dom_and_state(
        "/",
        UiTestMockState {
            boot: Some(AppBoot::NeedsLogin),
            ..Default::default()
        },
    );

    mount();
    settle().await;

    assert!(has_testid("login-overlay"));
    assert!(has_testid("app-login-page"));
}

#[wasm_bindgen_test(async)]
async fn renders_forced_password_gate_when_required() {
    reset_dom_and_state(
        "/",
        UiTestMockState {
            boot: Some(AppBoot::Ready(fixtures::user(UserRole::User, true))),
            ..Default::default()
        },
    );

    mount();
    settle().await;

    assert!(has_testid("forced-password-overlay"));
    assert!(has_testid("forced-change-password-page"));
}

#[wasm_bindgen_test(async)]
async fn renders_authenticated_home_shell() {
    reset_dom_and_state(
        "/",
        UiTestMockState {
            boot: Some(AppBoot::Ready(fixtures::user(UserRole::User, false))),
            providers: Some(Ok(fixtures::providers())),
            ..Default::default()
        },
    );

    mount();
    settle().await;

    assert!(has_testid("app-shell-nav"));
    assert!(has_testid("home-page"));
}

#[wasm_bindgen_test(async)]
async fn renders_admin_route_for_admin_user() {
    reset_dom_and_state(
        "/admin",
        UiTestMockState {
            boot: Some(AppBoot::Ready(fixtures::user(UserRole::Admin, false))),
            providers: Some(Ok(fixtures::providers())),
            admin_users: Some(Ok(vec![fixtures::user(UserRole::Admin, false)])),
            settings: Some(Ok(fixtures::settings())),
            ..Default::default()
        },
    );

    mount();
    settle().await;
    assert!(has_testid("admin-page"));
}

#[wasm_bindgen_test(async)]
async fn renders_provider_login_route() {
    reset_dom_and_state(
        "/login/provider-a",
        UiTestMockState {
            boot: Some(AppBoot::Ready(fixtures::user(UserRole::User, false))),
            provider: Some(Ok(fixtures::providers()[0].clone())),
            provider_session_valid: Some(false),
            ..Default::default()
        },
    );
    mount();
    settle().await;
    assert!(has_testid("provider-login-page"));
}

#[wasm_bindgen_test(async)]
async fn renders_not_found_route() {
    reset_dom_and_state(
        "/404-missing",
        UiTestMockState {
            boot: Some(AppBoot::Ready(fixtures::user(UserRole::User, false))),
            providers: Some(Ok(fixtures::providers())),
            ..Default::default()
        },
    );
    mount();
    settle().await;
    assert!(has_testid("not-found-page"));
}

#[wasm_bindgen_test(async)]
async fn admin_dashboard_link_visible_for_admin() {
    reset_dom_and_state(
        "/",
        UiTestMockState {
            boot: Some(AppBoot::Ready(fixtures::user(UserRole::Admin, false))),
            providers: Some(Ok(fixtures::providers())),
            ..Default::default()
        },
    );

    mount();
    settle().await;
    assert!(has_testid("admin-dashboard-link"));
}

#[wasm_bindgen_test(async)]
async fn admin_dashboard_link_hidden_for_non_admin() {
    reset_dom_and_state(
        "/",
        UiTestMockState {
            boot: Some(AppBoot::Ready(fixtures::user(UserRole::User, false))),
            providers: Some(Ok(fixtures::providers())),
            ..Default::default()
        },
    );

    mount();
    settle().await;
    assert!(!has_testid("admin-dashboard-link"));
}

#[wasm_bindgen_test(async)]
async fn voluntary_password_action_opens_overlay() {
    reset_dom_and_state(
        "/",
        UiTestMockState {
            boot: Some(AppBoot::Ready(fixtures::user(UserRole::User, false))),
            providers: Some(Ok(fixtures::providers())),
            ..Default::default()
        },
    );

    mount();
    settle().await;
    click_testid("open-change-password-button");
    settle().await;

    assert!(has_testid("voluntary-password-overlay"));
    assert!(has_testid("voluntary-change-password-page"));
}

#[wasm_bindgen_test(async)]
async fn sign_out_returns_to_login_gate() {
    reset_dom_and_state(
        "/",
        UiTestMockState {
            boot: Some(AppBoot::Ready(fixtures::user(UserRole::User, false))),
            providers: Some(Ok(fixtures::providers())),
            ..Default::default()
        },
    );

    mount();
    settle().await;
    assert!(has_testid("sign-out-button"));

    click_testid("sign-out-button");
    settle().await;

    assert!(has_testid("login-overlay"));
}

#[wasm_bindgen_test(async)]
async fn in_app_navigation_uses_spa_route_transition() {
    reset_dom_and_state(
        "/admin",
        UiTestMockState {
            boot: Some(AppBoot::Ready(fixtures::user(UserRole::Admin, false))),
            providers: Some(Ok(fixtures::providers())),
            admin_users: Some(Ok(vec![fixtures::user(UserRole::Admin, false)])),
            settings: Some(Ok(fixtures::settings())),
            ..Default::default()
        },
    );

    mount();
    settle().await;
    assert_eq!(pathname(), "/admin");

    click_testid("app-logo-link");
    settle().await;

    assert_eq!(pathname(), "/");
    assert!(has_testid("app-shell-nav"));
    assert!(has_testid("home-page"));
}

#[wasm_bindgen_test(async)]
async fn channel_to_player_navigation_keeps_playback_context() {
    install_player_stubs();
    reset_dom_and_state(
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

    mount();
    sleep_ms(60).await;
    assert!(has_testid("channels-page"));

    click_selector("[title='News One']");
    sleep_ms(180).await;

    assert_eq!(pathname(), "/providers/provider-a/play/channel-1");
    assert_eq!(text_for_testid("player-channel-name"), "News One");
}
