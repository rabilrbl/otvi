use leptos::ev;
use leptos::prelude::*;
use leptos::task::spawn_local;
use otvi_core::types::UserInfo;

use crate::api;

/// First-run wizard shown when no users exist yet.
/// Creates the initial admin account.
#[component]
pub fn SetupPage(on_done: Callback<UserInfo>) -> impl IntoView {
    let (username, set_username) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (confirm, set_confirm) = signal(String::new());
    let (error, set_error) = signal(Option::<String>::None);
    let (loading, set_loading) = signal(false);

    let on_submit = move |ev: ev::SubmitEvent| {
        ev.prevent_default();
        let u = username.get_untracked();
        let p = password.get_untracked();
        let c = confirm.get_untracked();

        if u.trim().is_empty() {
            set_error.set(Some("Username is required.".into()));
            return;
        }
        if p.len() < 8 {
            set_error.set(Some("Password must be at least 8 characters.".into()));
            return;
        }
        if p != c {
            set_error.set(Some("Passwords do not match.".into()));
            return;
        }

        set_loading.set(true);
        set_error.set(None);

        spawn_local(async move {
            match api::app_register(&u, &p).await {
                Ok(resp) => {
                    api::store_token(&resp.token);
                    on_done.run(resp.user);
                }
                Err(e) => {
                    set_error.set(Some(e));
                    set_loading.set(false);
                }
            }
        });
    };

    view! {
        <div class="min-h-screen flex items-center justify-center bg-gray-950 px-4">
            <div class="w-full max-w-sm">
                <div class="text-center mb-8">
                    <div class="text-4xl font-bold text-rose-500 mb-3">"OTVI"</div>
                    <h1 class="text-2xl font-semibold text-white mb-1">"Welcome!"</h1>
                    <p class="text-gray-400 text-sm">
                        "This is your first time running OTVI. "
                        "Create an admin account to continue."
                    </p>
                </div>

                <form
                    class="bg-gray-900 rounded-xl p-8 space-y-4 border border-white/5"
                    on:submit=on_submit
                >
                    <h2 class="text-base font-semibold text-gray-200 mb-1">"Create admin account"</h2>

                    <div class="space-y-1">
                        <label class="block text-sm text-gray-400">"Username"</label>
                        <input
                            type="text"
                            class="w-full bg-gray-800 rounded-lg px-3 py-2.5 text-sm text-white placeholder-gray-500 border border-white/10 focus:outline-none focus:border-rose-500"
                            placeholder="admin"
                            autocomplete="username"
                            on:input=move |ev| set_username.set(event_target_value(&ev))
                        />
                    </div>

                    <div class="space-y-1">
                        <label class="block text-sm text-gray-400">"Password"</label>
                        <input
                            type="password"
                            class="w-full bg-gray-800 rounded-lg px-3 py-2.5 text-sm text-white placeholder-gray-500 border border-white/10 focus:outline-none focus:border-rose-500"
                            placeholder="Min. 8 characters"
                            autocomplete="new-password"
                            on:input=move |ev| set_password.set(event_target_value(&ev))
                        />
                    </div>

                    <div class="space-y-1">
                        <label class="block text-sm text-gray-400">"Confirm password"</label>
                        <input
                            type="password"
                            class="w-full bg-gray-800 rounded-lg px-3 py-2.5 text-sm text-white placeholder-gray-500 border border-white/10 focus:outline-none focus:border-rose-500"
                            placeholder="Repeat password"
                            autocomplete="new-password"
                            on:input=move |ev| set_confirm.set(event_target_value(&ev))
                        />
                    </div>

                    <Show when=move || error.get().is_some()>
                        <div class="text-red-400 bg-red-400/10 px-3 py-2.5 rounded-lg text-sm">
                            {move || error.get()}
                        </div>
                    </Show>

                    <button
                        type="submit"
                        class="w-full py-3 rounded-lg bg-rose-500 text-white font-medium hover:bg-rose-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer mt-2"
                        disabled=move || loading.get()
                    >
                        {move || if loading.get() { "Creating account…" } else { "Create admin account" }}
                    </button>
                </form>
            </div>
        </div>
    }
}
