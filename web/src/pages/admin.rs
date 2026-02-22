//! Admin dashboard: manage users, providers per user, and server settings.

use leptos::ev;
use leptos::prelude::*;
use leptos::task::spawn_local;
use otvi_core::types::*;

use crate::api;

// ── Entry point ──────────────────────────────────────────────────────────────

#[component]
pub fn AdminPage() -> impl IntoView {
    // ── Shared state ──────────────────────────────────────────────────────
    let users: RwSignal<Vec<UserInfo>> = RwSignal::new(vec![]);
    let providers: RwSignal<Vec<ProviderInfo>> = RwSignal::new(vec![]);
    let settings: RwSignal<ServerSettings> = RwSignal::new(ServerSettings {
        signup_disabled: false,
    });
    let page_error: RwSignal<Option<String>> = RwSignal::new(None);

    // ── Load on mount ─────────────────────────────────────────────────────
    Effect::new(move |_| {
        spawn_local(async move {
            let (u, p, s) = futures_join3(
                api::admin_list_users(),
                api::fetch_providers(),
                api::admin_get_settings(),
            )
            .await;
            match u {
                Ok(list) => users.set(list),
                Err(e) => page_error.set(Some(format!("Failed to load users: {e}"))),
            }
            match p {
                Ok(list) => providers.set(list),
                Err(_) => {}
            }
            match s {
                Ok(cfg) => settings.set(cfg),
                Err(e) => page_error.set(Some(format!("Failed to load settings: {e}"))),
            }
        });
    });

    view! {
        <div class="max-w-4xl mx-auto px-6 py-8 space-y-8">
            <h1 class="text-3xl font-bold">"Admin Dashboard"</h1>

            <Show when=move || page_error.get().is_some()>
                <div class="text-red-400 bg-red-400/10 px-4 py-3 rounded-lg text-sm">
                    {move || page_error.get()}
                </div>
            </Show>

            <SettingsSection settings=settings />
            <AddUserSection users=users providers=providers />
            <UsersSection users=users providers=providers />
        </div>
    }
}

// ── Parallel async helper (no external futures crate needed) ─────────────────

async fn futures_join3<A, B, C>(a: A, b: B, c: C) -> (A::Output, B::Output, C::Output)
where
    A: std::future::Future,
    B: std::future::Future,
    C: std::future::Future,
{
    // Sequential is fine for an admin page that loads once.
    (a.await, b.await, c.await)
}

// ── Settings section ─────────────────────────────────────────────────────────

#[component]
fn SettingsSection(settings: RwSignal<ServerSettings>) -> impl IntoView {
    let loading: RwSignal<bool> = RwSignal::new(false);
    let error: RwSignal<Option<String>> = RwSignal::new(None);
    let success: RwSignal<bool> = RwSignal::new(false);

    let save = move |_: ev::MouseEvent| {
        let cfg = settings.get_untracked();
        loading.set(true);
        error.set(None);
        success.set(false);
        spawn_local(async move {
            match api::admin_update_settings(cfg).await {
                Ok(()) => success.set(true),
                Err(e) => error.set(Some(e)),
            }
            loading.set(false);
        });
    };

    view! {
        <section class="bg-gray-900 border border-white/5 rounded-xl p-6 space-y-4">
            <h2 class="text-lg font-semibold">"Server Settings"</h2>

            <label class="flex items-center gap-3 cursor-pointer">
                <input
                    type="checkbox"
                    class="w-4 h-4 accent-rose-500 cursor-pointer"
                    prop:checked=move || settings.get().signup_disabled
                    on:change=move |ev| {
                        let checked = event_target_checked(&ev);
                        settings.update(|s| s.signup_disabled = checked);
                    }
                />
                <span class="text-sm text-gray-300">"Disable public sign-up"</span>
                <span class="text-xs text-gray-500">
                    "(only admins can create new accounts)"
                </span>
            </label>

            <Show when=move || error.get().is_some()>
                <div class="text-red-400 bg-red-400/10 px-3 py-2 rounded-lg text-sm">
                    {move || error.get()}
                </div>
            </Show>
            <Show when=move || success.get()>
                <div class="text-emerald-400 bg-emerald-400/10 px-3 py-2 rounded-lg text-sm">
                    "Settings saved."
                </div>
            </Show>

            <button
                class="px-4 py-2 rounded-lg bg-rose-500 hover:bg-rose-600 text-white text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer"
                on:click=save
                disabled=move || loading.get()
            >
                {move || if loading.get() { "Saving…" } else { "Save Settings" }}
            </button>
        </section>
    }
}

// ── Add user section ──────────────────────────────────────────────────────────

#[component]
fn AddUserSection(
    users: RwSignal<Vec<UserInfo>>,
    providers: RwSignal<Vec<ProviderInfo>>,
) -> impl IntoView {
    let username: RwSignal<String> = RwSignal::new(String::new());
    let password: RwSignal<String> = RwSignal::new(String::new());
    let is_admin: RwSignal<bool> = RwSignal::new(false);
    // Selected provider IDs; empty vec means access to all.
    let selected_providers: RwSignal<Vec<String>> = RwSignal::new(vec![]);
    let loading: RwSignal<bool> = RwSignal::new(false);
    let error: RwSignal<Option<String>> = RwSignal::new(None);

    let toggle_provider = move |pid: String| {
        selected_providers.update(|list| {
            if list.contains(&pid) {
                list.retain(|x| x != &pid);
            } else {
                list.push(pid);
            }
        });
    };

    let on_submit = move |ev: ev::SubmitEvent| {
        ev.prevent_default();
        let u = username.get_untracked();
        let p = password.get_untracked();
        if u.trim().is_empty() || p.is_empty() {
            error.set(Some("Username and password are required.".into()));
            return;
        }
        let role = if is_admin.get_untracked() {
            UserRole::Admin
        } else {
            UserRole::User
        };
        let req = CreateUserRequest {
            username: u,
            password: p,
            role,
            providers: selected_providers.get_untracked(),
        };
        loading.set(true);
        error.set(None);
        spawn_local(async move {
            match api::admin_create_user(req).await {
                Ok(new_user) => {
                    users.update(|list| list.push(new_user));
                    username.set(String::new());
                    password.set(String::new());
                    is_admin.set(false);
                    selected_providers.set(vec![]);
                }
                Err(e) => error.set(Some(e)),
            }
            loading.set(false);
        });
    };

    view! {
        <section class="bg-gray-900 border border-white/5 rounded-xl p-6 space-y-4">
            <h2 class="text-lg font-semibold">"Create User"</h2>

            <form class="space-y-4" on:submit=on_submit>
                // ── Credentials row ───────────────────────────────────────
                <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
                    <div class="space-y-1">
                        <label class="block text-sm text-gray-400">"Username"</label>
                        <input
                            type="text"
                            class="w-full bg-gray-800 rounded-lg px-3 py-2.5 text-sm text-white placeholder-gray-500 border border-white/10 focus:outline-none focus:border-rose-500"
                            placeholder="e.g. alice"
                            autocomplete="off"
                            prop:value=move || username.get()
                            on:input=move |ev| username.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="space-y-1">
                        <label class="block text-sm text-gray-400">"Password"</label>
                        <input
                            type="password"
                            class="w-full bg-gray-800 rounded-lg px-3 py-2.5 text-sm text-white placeholder-gray-500 border border-white/10 focus:outline-none focus:border-rose-500"
                            placeholder="Min. 8 characters"
                            autocomplete="new-password"
                            prop:value=move || password.get()
                            on:input=move |ev| password.set(event_target_value(&ev))
                        />
                    </div>
                </div>

                // ── Role toggle ───────────────────────────────────────────
                <label class="flex items-center gap-3 cursor-pointer">
                    <input
                        type="checkbox"
                        class="w-4 h-4 accent-rose-500 cursor-pointer"
                        prop:checked=move || is_admin.get()
                        on:change=move |ev| is_admin.set(event_target_checked(&ev))
                    />
                    <span class="text-sm text-gray-300">"Admin role"</span>
                </label>

                // ── Provider access ───────────────────────────────────────
                <div class="space-y-2">
                    <p class="text-sm text-gray-400">
                        "Provider access "
                        <span class="text-gray-500">"(none selected = all providers)"</span>
                    </p>
                    <div class="flex flex-wrap gap-3">
                        <Show
                            when=move || !providers.get().is_empty()
                            fallback=move || view! {
                                <span class="text-xs text-gray-500">"No providers configured."</span>
                            }
                        >
                            <For
                                each=move || providers.get()
                                key=|p| p.id.clone()
                                children=move |provider| {
                                    let pid = provider.id.clone();
                                    let pid2 = pid.clone();
                                    view! {
                                        <label class="flex items-center gap-2 cursor-pointer bg-gray-800 px-3 py-1.5 rounded-lg border border-white/10 hover:border-rose-500 transition-colors">
                                            <input
                                                type="checkbox"
                                                class="w-3.5 h-3.5 accent-rose-500"
                                                prop:checked=move || selected_providers.get().contains(&pid2)
                                                on:change=move |_| toggle_provider(pid.clone())
                                            />
                                            <span class="text-sm">{provider.name}</span>
                                        </label>
                                    }
                                }
                            />
                        </Show>
                    </div>
                </div>

                // ── Error / submit ────────────────────────────────────────
                <Show when=move || error.get().is_some()>
                    <div class="text-red-400 bg-red-400/10 px-3 py-2 rounded-lg text-sm">
                        {move || error.get()}
                    </div>
                </Show>

                <button
                    type="submit"
                    class="px-5 py-2 rounded-lg bg-rose-500 hover:bg-rose-600 text-white text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer"
                    disabled=move || loading.get()
                >
                    {move || if loading.get() { "Creating…" } else { "Create User" }}
                </button>
            </form>
        </section>
    }
}

// ── Users list section ────────────────────────────────────────────────────────

#[component]
fn UsersSection(
    users: RwSignal<Vec<UserInfo>>,
    providers: RwSignal<Vec<ProviderInfo>>,
) -> impl IntoView {
    // (user_id, current provider selection) for the inline edit panel.
    // Only one user's providers can be edited at a time.
    let editing: RwSignal<Option<(String, Vec<String>)>> = RwSignal::new(None);
    let edit_loading: RwSignal<bool> = RwSignal::new(false);
    let edit_error: RwSignal<Option<String>> = RwSignal::new(None);

    view! {
        <section class="bg-gray-900 border border-white/5 rounded-xl p-6 space-y-4">
            <h2 class="text-lg font-semibold">"Users"</h2>

            <Show
                when=move || !users.get().is_empty()
                fallback=move || view! {
                    <p class="text-gray-500 text-sm">"No users yet."</p>
                }
            >
                <div class="space-y-3">
                    <For
                        each=move || users.get()
                        key=|u| u.id.clone()
                        children=move |user| {
                            let uid = user.id.clone();

                            // ── Derived signals scoped to this user ────────
                            let is_editing = {
                                let uid2 = uid.clone();
                                move || {
                                    editing
                                        .get()
                                        .as_ref()
                                        .map(|(id, _)| id == &uid2)
                                        .unwrap_or(false)
                                }
                            };

                            // ── Edit-providers button ──────────────────────
                            let uid_start = uid.clone();
                            let user_providers = user.providers.clone();
                            let on_start_edit = move |_: ev::MouseEvent| {
                                editing.set(Some((uid_start.clone(), user_providers.clone())));
                                edit_error.set(None);
                            };

                            // ── Delete button ──────────────────────────────
                            let uid_del = uid.clone();
                            let on_delete = move |_: ev::MouseEvent| {
                                let uid_d = uid_del.clone();
                                spawn_local(async move {
                                    if api::admin_delete_user(&uid_d).await.is_ok() {
                                        users.update(|list| list.retain(|u| u.id != uid_d));
                                        editing.update(|e| {
                                            if e.as_ref()
                                                .map(|(id, _)| id == &uid_d)
                                                .unwrap_or(false)
                                            {
                                                *e = None;
                                            }
                                        });
                                    }
                                });
                            };


                            view! {
                                <div class="bg-gray-800/60 rounded-lg border border-white/5">
                                    // ── User row ──────────────────────────────
                                    <div class="flex items-center gap-4 px-4 py-3 flex-wrap">
                                        // Username + role badge
                                        <div class="flex items-center gap-2 flex-1 min-w-0">
                                            <span class="font-medium truncate">{user.username.clone()}</span>
                                            {match user.role {
                                                UserRole::Admin => view! {
                                                    <span class="text-xs bg-rose-500/20 text-rose-400 px-2 py-0.5 rounded-full shrink-0">"admin"</span>
                                                }.into_any(),
                                                UserRole::User => view! {
                                                    <span class="text-xs bg-gray-700 text-gray-400 px-2 py-0.5 rounded-full shrink-0">"user"</span>
                                                }.into_any(),
                                            }}
                                        </div>

                                        // Provider chips
                                        <div class="flex flex-wrap gap-1.5 flex-1">
                                            {if user.providers.is_empty() {
                                                view! {
                                                    <span class="text-xs text-gray-500 italic">"all providers"</span>
                                                }.into_any()
                                            } else {
                                                user.providers
                                                    .iter()
                                                    .map(|p| {
                                                        view! {
                                                            <span class="text-xs bg-blue-500/20 text-blue-300 px-2 py-0.5 rounded-full">
                                                                {p.clone()}
                                                            </span>
                                                        }
                                                    })
                                                    .collect_view()
                                                    .into_any()
                                            }}
                                        </div>

                                        // Actions
                                        <div class="flex gap-2 shrink-0">
                                            <button
                                                class="px-3 py-1.5 text-xs rounded-lg bg-gray-700 hover:bg-gray-600 text-gray-200 transition-colors cursor-pointer"
                                                on:click=on_start_edit
                                            >
                                                "Edit Providers"
                                            </button>
                                            <button
                                                class="px-3 py-1.5 text-xs rounded-lg bg-red-500/20 hover:bg-red-500/40 text-red-400 transition-colors cursor-pointer"
                                                on:click=on_delete
                                            >
                                                "Delete"
                                            </button>
                                        </div>
                                    </div>

                                    // ── Inline provider editor ─────────────────
                                    <Show when=is_editing>
                                        <div class="border-t border-white/5 px-4 py-4 space-y-3">
                                            <p class="text-sm text-gray-400">
                                                "Select allowed providers "
                                                <span class="text-gray-500">"(none = all)"</span>
                                            </p>

                                            <div class="flex flex-wrap gap-3">
                                                <Show
                                                    when=move || !providers.get().is_empty()
                                                    fallback=move || view! {
                                                        <span class="text-xs text-gray-500">"No providers configured."</span>
                                                    }
                                                >
                                                    <For
                                                        each=move || providers.get()
                                                        key=|p| p.id.clone()
                                                        children=move |provider| {
                                                            let pid = provider.id.clone();
                                                            let pid_chk = pid.clone();
                                                            let pid_toggle = pid.clone();
                                                            view! {
                                                                <label class="flex items-center gap-2 cursor-pointer bg-gray-700/60 px-3 py-1.5 rounded-lg border border-white/10 hover:border-rose-500 transition-colors">
                                                                    <input
                                                                        type="checkbox"
                                                                        class="w-3.5 h-3.5 accent-rose-500"
                                                                        prop:checked=move || {
                                                                            editing
                                                                                .get()
                                                                                .as_ref()
                                                                                .map(|(_, list)| list.contains(&pid_chk))
                                                                                .unwrap_or(false)
                                                                        }
                                                                        on:change=move |_| {
                                                                            editing.update(|e| {
                                                                                if let Some((_, list)) = e {
                                                                                    if list.contains(&pid_toggle) {
                                                                                        list.retain(|x| x != &pid_toggle);
                                                                                    } else {
                                                                                        list.push(pid_toggle.clone());
                                                                                    }
                                                                                }
                                                                            });
                                                                        }
                                                                    />
                                                                    <span class="text-sm">{provider.name}</span>
                                                                </label>
                                                            }
                                                        }
                                                    />
                                                </Show>
                                            </div>

                                            <Show when=move || edit_error.get().is_some()>
                                                <div class="text-red-400 bg-red-400/10 px-3 py-2 rounded-lg text-sm">
                                                    {move || edit_error.get()}
                                                </div>
                                            </Show>

                                            <div class="flex gap-2">
                                                <button
                                                    class="px-4 py-1.5 rounded-lg bg-rose-500 hover:bg-rose-600 text-white text-sm font-medium transition-colors disabled:opacity-50 cursor-pointer"
                                                    on:click=move |_: ev::MouseEvent| {
                                                        // Derive uid/providers from the `editing` signal
                                                        // so we only capture Copy types here.
                                                        let Some((uid, plist)) =
                                                            editing.get_untracked()
                                                        else {
                                                            return;
                                                        };
                                                        edit_loading.set(true);
                                                        edit_error.set(None);
                                                        spawn_local(async move {
                                                            match api::admin_set_user_providers(
                                                                &uid, plist.clone(),
                                                            )
                                                            .await
                                                            {
                                                                Ok(()) => {
                                                                    users.update(|list| {
                                                                        if let Some(u) = list
                                                                            .iter_mut()
                                                                            .find(|u| u.id == uid)
                                                                        {
                                                                            u.providers = plist;
                                                                        }
                                                                    });
                                                                    editing.set(None);
                                                                }
                                                                Err(e) => {
                                                                    edit_error.set(Some(e))
                                                                }
                                                            }
                                                            edit_loading.set(false);
                                                        });
                                                    }
                                                    disabled=move || edit_loading.get()
                                                >
                                                    {move || if edit_loading.get() { "Saving…" } else { "Save" }}
                                                </button>
                                                <button
                                                    class="px-4 py-1.5 rounded-lg bg-gray-700 hover:bg-gray-600 text-gray-300 text-sm transition-colors cursor-pointer"
                                                    on:click=move |_| {
                                                        editing.set(None);
                                                        edit_error.set(None);
                                                    }
                                                >
                                                    "Cancel"
                                                </button>
                                            </div>
                                        </div>
                                    </Show>
                                </div>
                            }
                        }
                    />
                </div>
            </Show>
        </section>
    }
}
