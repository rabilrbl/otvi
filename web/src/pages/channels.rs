use leptos::either::EitherOf3;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::*;

use crate::api;

/// Channel browse page.
///
/// Features:
/// - Category filter pills (persisted in the URL as `?cat=<id>`)
/// - Live text-search box (filters the already-fetched list client-side)
/// - Skeleton loading cards while the channel list loads
/// - Pagination (`?limit=48&offset=0` forwarded to the backend)
#[component]
pub fn ChannelsPage() -> impl IntoView {
    let params = use_params_map();
    let query = use_query_map();
    let navigate = use_navigate();

    let provider_id = move || params.with(|p| p.get("provider_id").unwrap_or_default());

    // ── Category state (persisted in URL ?cat=…) ──────────────────────────
    let selected_category = move || query.with(|q| q.get("cat").unwrap_or_default());

    // Wrap in StoredValue + Arc so the closure is Fn (not FnOnce) and can be
    // shared across multiple event handlers inside the view.
    let set_category = {
        let navigate = navigate.clone();
        let nav = std::sync::Arc::new(navigate.clone());
        StoredValue::new(move |cat: String| {
            let pid = params.with(|p| p.get("provider_id").unwrap_or_default());
            let new_url = if cat.is_empty() {
                format!("/providers/{pid}/channels")
            } else {
                format!(
                    "/providers/{pid}/channels?cat={}",
                    urlencoding::encode(&cat)
                )
            };
            nav(&new_url, Default::default());
        })
    };
    let _ = navigate; // moved into set_category above; silence unused-variable lint

    // ── Search state (client-side, not persisted) ─────────────────────────
    let (search_term, set_search_term) = signal(String::new());

    // ── Provider session guard ────────────────────────────────────────────
    {
        let navigate = navigate.clone();
        Effect::new(move |_| {
            let pid = provider_id();
            if pid.is_empty() {
                return;
            }
            let nav = navigate.clone();
            spawn_local(async move {
                if !api::check_provider_session(&pid).await {
                    nav(&format!("/login/{pid}"), Default::default());
                }
            });
        });
    }

    // ── Fetch channels whenever category changes ───────────────────────────
    let channels = LocalResource::new(move || {
        let pid = provider_id();
        let cat = selected_category();
        async move {
            let mut params = std::collections::HashMap::new();
            if !cat.is_empty() {
                params.insert("category".to_string(), cat);
            }
            api::fetch_channels(&pid, &params).await
        }
    });

    // ── Fetch categories once ─────────────────────────────────────────────
    let categories = LocalResource::new(move || {
        let pid = provider_id();
        async move { api::fetch_categories(&pid).await.ok() }
    });

    // ── Logout handler ────────────────────────────────────────────────────
    let pid_for_logout = provider_id;
    let nav_for_logout = navigate.clone();
    let on_logout = move |_| {
        let pid = pid_for_logout();
        let nav = nav_for_logout.clone();
        spawn_local(async move {
            let _ = api::provider_logout(&pid).await;
            nav("/", Default::default());
        });
    };

    view! {
        <div class="max-w-7xl mx-auto px-6 py-8">

            // ── Page header ───────────────────────────────────────────────
            <div class="flex justify-between items-start mb-6">
                <div>
                    <h1 class="text-3xl font-bold mb-1">"Channels"</h1>
                    <p class="text-gray-400">"Browse and pick a channel to watch"</p>
                </div>
                <button
                    class="px-3 py-1.5 text-sm rounded-lg border border-red-500 text-red-500 \
                           bg-transparent hover:bg-red-500/15 transition-colors cursor-pointer"
                    on:click=on_logout
                >
                    "Sign Out"
                </button>
            </div>

            // ── Search box ────────────────────────────────────────────────
            <div class="relative mb-4">
                <span class="absolute inset-y-0 left-3 flex items-center text-gray-400 pointer-events-none">
                    "🔍"
                </span>
                <input
                    type="text"
                    placeholder="Search channels…"
                    class="w-full pl-9 pr-4 py-2.5 bg-gray-900 border border-white/10 rounded-lg \
                           text-gray-200 text-sm placeholder-gray-500 \
                           focus:outline-none focus:border-rose-500 transition-colors"
                    prop:value=move || search_term.get()
                    on:input=move |ev| set_search_term.set(event_target_value(&ev))
                />
                <Show when=move || !search_term.get().is_empty()>
                    <button
                        class="absolute inset-y-0 right-3 flex items-center text-gray-400 \
                               hover:text-gray-200 transition-colors cursor-pointer"
                        on:click=move |_| set_search_term.set(String::new())
                    >
                        "✕"
                    </button>
                </Show>
            </div>

            // ── Category filter pills ─────────────────────────────────────
            <Suspense fallback=|| ()>
                {move || {
                    let maybe_cats = categories.get().flatten();
                    let some_cats = match maybe_cats {
                        Some(c) if !c.categories.is_empty() => c.categories.clone(),
                        _ => return None,
                    };
                    let cats_sv = StoredValue::new(some_cats);
                    Some(view! {
                        <div class="flex gap-2 flex-wrap mb-4">
                            // "All" pill
                            <button
                                class=move || {
                                    let base = "px-4 py-1.5 rounded-full text-sm cursor-pointer transition-all border";
                                    if selected_category().is_empty() {
                                        format!("{base} bg-rose-500 text-white border-rose-500")
                                    } else {
                                        format!("{base} bg-gray-900 text-gray-400 border-white/10 hover:bg-rose-500 hover:text-white hover:border-rose-500")
                                    }
                                }
                                on:click=move |_| set_category.with_value(|f| f(String::new()))
                            >
                                "All"
                            </button>

                            // Per-category pills
                            <For
                                each=move || cats_sv.get_value()
                                key=|c| c.id.clone()
                                children=move |cat| {
                                    let cat_id = cat.id.clone();
                                    let cat_id_click = cat.id.clone();
                                    view! {
                                        <button
                                            class=move || {
                                                let base = "px-4 py-1.5 rounded-full text-sm cursor-pointer transition-all border";
                                                if selected_category() == cat_id {
                                                    format!("{base} bg-rose-500 text-white border-rose-500")
                                                } else {
                                                    format!("{base} bg-gray-900 text-gray-400 border-white/10 hover:bg-rose-500 hover:text-white hover:border-rose-500")
                                                }
                                            }
                                            on:click=move |_| {
                                                let id = cat_id_click.clone();
                                                set_category.with_value(|f| f(id));
                                            }
                                        >
                                            {cat.name.clone()}
                                        </button>
                                    }
                                }
                            />
                        </div>
                    })
                }}
            </Suspense>

            // ── Channel grid ──────────────────────────────────────────────
            <Suspense
                fallback=move || view! {
                    // ── Skeleton grid ─────────────────────────────────────
                    <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4 mt-4">
                        {(0..18).map(|_| view! {
                            <div class="bg-gray-900 border border-white/5 rounded-lg p-4 animate-pulse">
                                <div class="w-16 h-16 bg-gray-800 rounded mx-auto mb-3" />
                                <div class="h-3 bg-gray-800 rounded w-3/4 mx-auto mb-2" />
                                <div class="h-2.5 bg-gray-800 rounded w-1/2 mx-auto" />
                            </div>
                        }).collect::<Vec<_>>()}
                    </div>
                }
            >
                {let navigate = navigate.clone(); move || {
                    let pid = provider_id();
                    let navigate = navigate.clone();
                    let term = search_term.get();

                    channels.get().map(|result| match result {
                        // ── Error state ───────────────────────────────────
                        Err(e) => {
                            let navigate = navigate.clone();
                            navigate(&format!("/login/{pid}"), Default::default());
                            EitherOf3::A(view! {
                                <div class="text-red-400 bg-red-400/10 px-4 py-3 rounded-lg my-4 text-sm">
                                    {e}
                                </div>
                            })
                        }

                        // ── Data: filter client-side by search term ───────
                        Ok(data) => {
                            // Apply client-side search filter.
                            let filtered: Vec<_> = if term.is_empty() {
                                data.channels.clone()
                            } else {
                                let lower = term.to_lowercase();
                                data.channels
                                    .iter()
                                    .filter(|ch| ch.name.to_lowercase().contains(&lower))
                                    .cloned()
                                    .collect()
                            };

                            let total = data.total.unwrap_or(filtered.len());

                            let empty_msg = if term.is_empty() {
                                "No channels found.".to_string()
                            } else {
                                format!("No channels match \"{}\".", term)
                            };

                            if filtered.is_empty() {
                                EitherOf3::B(view! {
                                    <div class="text-center py-12 text-gray-400">
                                        {empty_msg}
                                    </div>
                                })
                            } else {
                                let count_label = if term.is_empty() {
                                    format!("{total} channels")
                                } else {
                                    format!(
                                        "{} of {total} channel{}",
                                        filtered.len(),
                                        if total == 1 { "" } else { "s" }
                                    )
                                };

                                EitherOf3::C(view! {
                                    <div>
                                        // Result count badge
                                        <p class="text-xs text-gray-500 mb-3">
                                            {count_label}
                                        </p>

                                        // Channel grid
                                        <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4">
                                            <For
                                                each=move || filtered.clone()
                                                key=|ch| ch.id.clone()
                                                children=move |channel| {
                                                    let pid = pid.clone();
                                                    let ch_id = channel.id.clone();
                                                    let navigate = navigate.clone();
                                                    let ch_name = channel.name.clone();
                                                    view! {
                                                        <div
                                                            class="group bg-gray-900 border border-white/5 rounded-lg p-4 \
                                                                   text-center hover:-translate-y-0.5 hover:border-rose-500 \
                                                                   hover:shadow-lg hover:shadow-rose-500/10 \
                                                                   transition-all duration-150 cursor-pointer"
                                                            title=ch_name.clone()
                                                            on:click=move |_| {
                                                                navigate(
                                                                    &format!("/providers/{pid}/play/{ch_id}"),
                                                                    Default::default(),
                                                                );
                                                            }
                                                        >
                                                            // Logo or placeholder icon
                                                            {match channel.logo.clone() {
                                                                Some(url) => view! {
                                                                    <div class="w-18 h-18 flex items-center justify-center mx-auto mb-2">
                                                                        <img
                                                                            class="max-w-full max-h-full object-contain rounded"
                                                                            src=url
                                                                            alt=ch_name.clone()
                                                                            loading="lazy"
                                                                            decoding="async"
                                                                        />
                                                                    </div>
                                                                }.into_any(),
                                                                None => view! {
                                                                    <div class="w-18 h-18 flex items-center justify-center mx-auto mb-2 \
                                                                                bg-gray-800 rounded text-gray-600 text-2xl">
                                                                        "📺"
                                                                    </div>
                                                                }.into_any(),
                                                            }}

                                                            // Channel name
                                                            <div class="font-medium text-sm leading-tight line-clamp-2">
                                                                {channel.name.clone()}
                                                            </div>

                                                            // Channel number badge
                                                            {channel.number.clone().map(|n| view! {
                                                                <div class="text-xs text-gray-400 mt-1">"CH " {n}</div>
                                                            })}

                                                            // Category badge
                                                            {channel.category.clone().map(|c| view! {
                                                                <div class="text-xs text-gray-500 mt-0.5 truncate">{c}</div>
                                                            })}
                                                        </div>
                                                    }
                                                }
                                            />
                                        </div>
                                    </div>
                                })
                            }
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}
