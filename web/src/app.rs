use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::*;
use leptos_router::path;
use otvi_core::types::{UserInfo, UserRole};

use crate::api;
use crate::pages::{
    admin::AdminPage, app_login::AppLoginPage, channels::ChannelsPage, home::HomePage,
    login::LoginPage, not_found::NotFoundPage, player::PlayerPage, setup::SetupPage,
};

/// Shared auth context available to all child components.
#[derive(Clone, Copy)]
pub struct AuthCtx {
    /// The currently authenticated OTVI user (`None` while loading or logged out).
    pub user: RwSignal<Option<UserInfo>>,
}

impl AuthCtx {
    pub fn is_admin(&self) -> bool {
        self.user
            .get()
            .map(|u| u.role == UserRole::Admin)
            .unwrap_or(false)
    }

    pub fn username(&self) -> String {
        self.user
            .get()
            .map(|u| u.username)
            .unwrap_or_default()
    }
}

/// Possible states for the application bootstrap sequence.
#[derive(Clone, PartialEq)]
enum BootState {
    /// Checking setup / validating token.
    Loading,
    /// No users exist – first-run admin setup required.
    NeedsSetup,
    /// No valid JWT – user must log in.
    NeedsLogin,
    /// Authenticated and ready to use the app.
    Ready,
}

/// Root application component.
#[component]
pub fn App() -> impl IntoView {
    // ── Auth context ──────────────────────────────────────────────────────
    let user: RwSignal<Option<UserInfo>> = RwSignal::new(None);
    let auth_ctx = AuthCtx { user };
    provide_context(auth_ctx.clone());

    // ── Boot sequence ─────────────────────────────────────────────────────
    let (boot_state, set_boot_state) = signal(BootState::Loading);

    Effect::new(move |_| {
        spawn_local(async move {
            match api::boot_check().await {
                api::AppBoot::Ready(info) => {
                    user.set(Some(info));
                    set_boot_state.set(BootState::Ready);
                }
                api::AppBoot::NeedsLogin => {
                    set_boot_state.set(BootState::NeedsLogin);
                }
                api::AppBoot::NeedsSetup => {
                    set_boot_state.set(BootState::NeedsSetup);
                }
            }
        });
    });

    // Callback fired by both SetupPage and AppLoginPage on success.
    let on_auth_done = move |info: UserInfo| {
        user.set(Some(info));
        set_boot_state.set(BootState::Ready);
    };

    // Logout: clear token + reset state.
    let logout = move |_| {
        api::clear_token();
        user.set(None);
        set_boot_state.set(BootState::NeedsLogin);
    };

    view! {
        <Router>
            // ── Auth overlays (full-screen, cover the app until boot is done) ──
            <Show when=move || boot_state.get() == BootState::Loading fallback=|| ()>
                <div class="fixed inset-0 z-50 flex items-center justify-center bg-gray-950 text-gray-400">
                    <div class="animate-pulse text-sm">"Loading…"</div>
                </div>
            </Show>
            <Show when=move || boot_state.get() == BootState::NeedsSetup fallback=|| ()>
                <div class="fixed inset-0 z-50 bg-gray-950 overflow-auto">
                    <SetupPage on_done=Callback::new(on_auth_done) />
                </div>
            </Show>
            <Show when=move || boot_state.get() == BootState::NeedsLogin fallback=|| ()>
                <div class="fixed inset-0 z-50 bg-gray-950 overflow-auto">
                    <AppLoginPage on_done=Callback::new(on_auth_done) />
                </div>
            </Show>

            // ── App shell – always mounted so Router/Routes are never disposed ──
            <nav class="bg-gray-900 px-6 py-3 flex items-center justify-between sticky top-0 z-40 shadow-lg shadow-black/30">
                <a
                    class="text-xl font-bold text-rose-500 hover:text-rose-400 transition-colors"
                    href="/"
                >
                    "OTVI"
                </a>
                <div class="flex gap-3 items-center">
                    <span class="text-sm text-gray-400 hidden sm:inline">
                        {move || auth_ctx.username()}
                    </span>
                    <Show when=move || auth_ctx.is_admin()>
                        <span class="text-xs bg-rose-500/20 text-rose-400 px-2 py-0.5 rounded-full hidden sm:inline">
                            "admin"
                        </span>
                        <a
                            href="/admin"
                            class="px-3 py-1.5 text-sm rounded-lg bg-gray-800 text-gray-300 hover:bg-gray-700 transition-colors no-underline hidden sm:inline-block"
                        >
                            "Dashboard"
                        </a>
                    </Show>
                    <button
                        class="px-3 py-1.5 text-sm rounded-lg bg-gray-800 text-gray-300 hover:bg-gray-700 transition-colors cursor-pointer"
                        on:click=logout
                    >
                        "Sign out"
                    </button>
                </div>
            </nav>
            <main>
                <Routes fallback=NotFoundPage>
                    <Route path=path!("/") view=HomePage />
                    <Route path=path!("/admin") view=AdminPage />
                    <Route path=path!("/login/:provider_id") view=LoginPage />
                    <Route path=path!("/providers/:provider_id/channels") view=ChannelsPage />
                    <Route path=path!("/providers/:provider_id/play/:channel_id") view=PlayerPage />
                </Routes>
            </main>
        </Router>
    }
}

