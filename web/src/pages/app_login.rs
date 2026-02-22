use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::ev;
use otvi_core::types::UserInfo;

use crate::api;

/// OTVI application-level login / signup page.
/// Shown whenever the user has no valid JWT token.
#[component]
pub fn AppLoginPage(on_done: Callback<UserInfo>) -> impl IntoView {
    let (is_signup, set_is_signup) = signal(false);
    let (username, set_username) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (confirm, set_confirm) = signal(String::new());
    let (error, set_error) = signal(Option::<String>::None);
    let (loading, set_loading) = signal(false);

    let on_submit = move |ev: ev::SubmitEvent| {
        ev.prevent_default();
        let u = username.get_untracked();
        let p = password.get_untracked();
        let signup = is_signup.get_untracked();

        if u.trim().is_empty() || p.is_empty() {
            set_error.set(Some("Username and password are required.".into()));
            return;
        }

        if signup {
            let c = confirm.get_untracked();
            if p.len() < 8 {
                set_error.set(Some("Password must be at least 8 characters.".into()));
                return;
            }
            if p != c {
                set_error.set(Some("Passwords do not match.".into()));
                return;
            }
        }

        set_loading.set(true);
        set_error.set(None);

        spawn_local(async move {
            let result = if signup {
                api::app_register(&u, &p).await
            } else {
                api::app_login(&u, &p).await
            };
            match result {
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
                    <p class="text-gray-400 text-sm">"Sign in to continue"</p>
                </div>

                <div class="bg-gray-900 rounded-xl p-8 border border-white/5">
                    // Mode toggle
                    <div class="flex bg-gray-800 rounded-lg p-1 mb-6">
                        <button
                            type="button"
                            class="flex-1 py-2 text-sm rounded-md transition-colors cursor-pointer"
                            class=("bg-gray-700 text-white font-medium", move || !is_signup.get())
                            class=("text-gray-400 hover:text-gray-200", move || is_signup.get())
                            on:click=move |_| {
                                set_is_signup.set(false);
                                set_error.set(None);
                            }
                        >
                            "Sign in"
                        </button>
                        <button
                            type="button"
                            class="flex-1 py-2 text-sm rounded-md transition-colors cursor-pointer"
                            class=("bg-gray-700 text-white font-medium", move || is_signup.get())
                            class=("text-gray-400 hover:text-gray-200", move || !is_signup.get())
                            on:click=move |_| {
                                set_is_signup.set(true);
                                set_error.set(None);
                            }
                        >
                            "Create account"
                        </button>
                    </div>

                    <form class="space-y-4" on:submit=on_submit>
                        <div class="space-y-1">
                            <label class="block text-sm text-gray-400">"Username"</label>
                            <input
                                type="text"
                                class="w-full bg-gray-800 rounded-lg px-3 py-2.5 text-sm text-white placeholder-gray-500 border border-white/10 focus:outline-none focus:border-rose-500"
                                placeholder="your-username"
                                autocomplete="username"
                                prop:value=username
                                on:input=move |ev| set_username.set(event_target_value(&ev))
                            />
                        </div>

                        <div class="space-y-1">
                            <label class="block text-sm text-gray-400">"Password"</label>
                            <input
                                type="password"
                                class="w-full bg-gray-800 rounded-lg px-3 py-2.5 text-sm text-white placeholder-gray-500 border border-white/10 focus:outline-none focus:border-rose-500"
                                placeholder="••••••••"
                                autocomplete=move || if is_signup.get() { "new-password" } else { "current-password" }
                                prop:value=password
                                on:input=move |ev| set_password.set(event_target_value(&ev))
                            />
                        </div>

                        <Show when=move || is_signup.get()>
                            <div class="space-y-1">
                                <label class="block text-sm text-gray-400">"Confirm password"</label>
                                <input
                                    type="password"
                                    class="w-full bg-gray-800 rounded-lg px-3 py-2.5 text-sm text-white placeholder-gray-500 border border-white/10 focus:outline-none focus:border-rose-500"
                                    placeholder="Repeat password"
                                    autocomplete="new-password"
                                    prop:value=confirm
                                    on:input=move |ev| set_confirm.set(event_target_value(&ev))
                                />
                            </div>
                        </Show>

                        <Show when=move || error.get().is_some()>
                            <div class="text-red-400 bg-red-400/10 px-3 py-2.5 rounded-lg text-sm">
                                {move || error.get()}
                            </div>
                        </Show>

                        <button
                            type="submit"
                            class="w-full py-3 rounded-lg bg-rose-500 text-white font-medium hover:bg-rose-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer"
                            disabled=move || loading.get()
                        >
                            {move || {
                                if loading.get() {
                                    "Please wait…"
                                } else if is_signup.get() {
                                    "Create account"
                                } else {
                                    "Sign in"
                                }
                            }}
                        </button>
                    </form>
                </div>
            </div>
        </div>
    }
}
