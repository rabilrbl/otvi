//! Change-password page.
//!
//! Used in two contexts:
//!
//! 1. **Forced (overlay)** – `forced = true`.  Shown when an admin-created
//!    account first logs in.  The user cannot dismiss it; they must set a new
//!    password before proceeding.
//!
//! 2. **Voluntary** – `forced = false` (default).  Regular page that any
//!    authenticated user can visit to update their password.

use leptos::ev;
use leptos::prelude::*;
use leptos::task::spawn_local;
use otvi_core::types::UserInfo;

use crate::api;

/// Change-password form component.
///
/// `on_done` is called with the updated `UserInfo` after a successful change.
/// `forced` – when `true` the heading explains this is a required step.
#[component]
pub fn ChangePasswordPage(
    on_done: Callback<UserInfo>,
    /// When `true` the UI explains the temporary-password situation.
    #[prop(default = false)]
    forced: bool,
) -> impl IntoView {
    let current_pw: RwSignal<String> = RwSignal::new(String::new());
    let new_pw: RwSignal<String> = RwSignal::new(String::new());
    let confirm_pw: RwSignal<String> = RwSignal::new(String::new());
    let error: RwSignal<Option<String>> = RwSignal::new(None);
    let loading: RwSignal<bool> = RwSignal::new(false);

    let on_submit = move |ev: ev::SubmitEvent| {
        ev.prevent_default();
        let cur = current_pw.get_untracked();
        let new = new_pw.get_untracked();
        let conf = confirm_pw.get_untracked();

        if cur.is_empty() || new.is_empty() {
            error.set(Some("All fields are required.".into()));
            return;
        }
        if new.len() < 8 {
            error.set(Some("New password must be at least 8 characters.".into()));
            return;
        }
        if new != conf {
            error.set(Some("New passwords do not match.".into()));
            return;
        }

        loading.set(true);
        error.set(None);

        spawn_local(async move {
            match api::change_password(&cur, &new).await {
                Ok(resp) => {
                    api::store_token(&resp.token);
                    on_done.run(resp.user);
                }
                Err(e) => {
                    error.set(Some(e));
                    loading.set(false);
                }
            }
        });
    };

    view! {
        <div class="min-h-screen flex items-center justify-center bg-gray-950 px-4">
            <div class="w-full max-w-sm">
                // ── Header ─────────────────────────────────────────────────
                <div class="text-center mb-8">
                    <div class="text-4xl font-bold text-rose-500 mb-3">"OTVI"</div>
                    <h1 class="text-xl font-semibold text-white mb-2">
                        {if forced { "Set your password" } else { "Change password" }}
                    </h1>
                    <p class="text-gray-400 text-sm">
                        {if forced {
                            "Your account was created with a temporary password. \
                             Please choose a new password before continuing."
                        } else {
                            "Enter your current password and choose a new one."
                        }}
                    </p>
                </div>

                // ── Card ───────────────────────────────────────────────────
                <div class="bg-gray-900 rounded-xl p-8 border border-white/5">
                    <form class="space-y-4" on:submit=on_submit>
                        // Current / temporary password
                        <div class="space-y-1">
                            <label class="block text-sm text-gray-400">
                                {if forced { "Temporary password" } else { "Current password" }}
                            </label>
                            <input
                                type="password"
                                class="w-full bg-gray-800 rounded-lg px-3 py-2.5 text-sm text-white \
                                       placeholder-gray-500 border border-white/10 \
                                       focus:outline-none focus:border-rose-500"
                                placeholder="••••••••"
                                autocomplete="current-password"
                                prop:value=move || current_pw.get()
                                on:input=move |ev| current_pw.set(event_target_value(&ev))
                            />
                        </div>

                        // New password
                        <div class="space-y-1">
                            <label class="block text-sm text-gray-400">"New password"</label>
                            <input
                                type="password"
                                class="w-full bg-gray-800 rounded-lg px-3 py-2.5 text-sm text-white \
                                       placeholder-gray-500 border border-white/10 \
                                       focus:outline-none focus:border-rose-500"
                                placeholder="Min. 8 characters"
                                autocomplete="new-password"
                                prop:value=move || new_pw.get()
                                on:input=move |ev| new_pw.set(event_target_value(&ev))
                            />
                        </div>

                        // Confirm new password
                        <div class="space-y-1">
                            <label class="block text-sm text-gray-400">"Confirm new password"</label>
                            <input
                                type="password"
                                class="w-full bg-gray-800 rounded-lg px-3 py-2.5 text-sm text-white \
                                       placeholder-gray-500 border border-white/10 \
                                       focus:outline-none focus:border-rose-500"
                                placeholder="••••••••"
                                autocomplete="new-password"
                                prop:value=move || confirm_pw.get()
                                on:input=move |ev| confirm_pw.set(event_target_value(&ev))
                            />
                        </div>

                        // Error message
                        <Show when=move || error.get().is_some()>
                            <div class="text-red-400 bg-red-400/10 px-3 py-2 rounded-lg text-sm">
                                {move || error.get()}
                            </div>
                        </Show>

                        // Submit
                        <button
                            type="submit"
                            class="w-full py-2.5 rounded-lg bg-rose-500 hover:bg-rose-600 text-white \
                                   text-sm font-medium transition-colors \
                                   disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer"
                            disabled=move || loading.get()
                        >
                            {move || if loading.get() { "Saving…" } else { "Set new password" }}
                        </button>
                    </form>
                </div>
            </div>
        </div>
    }
}
