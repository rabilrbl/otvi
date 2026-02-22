use std::collections::HashMap;

use leptos::*;
use leptos_router::*;

use crate::api;

/// Channel browse page: displays a grid of channels with optional category filter.
#[component]
pub fn ChannelsPage() -> impl IntoView {
    let params = use_params_map();
    let provider_id =
        move || params.with(|p| p.get("provider_id").cloned().unwrap_or_default());

    let (selected_category, set_selected_category) = create_signal(String::new());
    let navigate = use_navigate();

    // Redirect to provider login if no authenticated provider session exists.
    {
        let navigate = navigate.clone();
        create_effect(move |_| {
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

    // Fetch channels whenever the selected category changes
    let channels = create_local_resource(
        move || (provider_id(), selected_category.get()),
        |(pid, cat)| async move {
            let mut params = HashMap::new();
            if !cat.is_empty() {
                params.insert("category".to_string(), cat);
            }
            api::fetch_channels(&pid, &params).await
        },
    );

    // Fetch categories once
    let categories = create_local_resource(provider_id, |pid| async move {
        api::fetch_categories(&pid).await.ok()
    });

    // Logout handler
    let pid_for_logout = provider_id.clone();
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
            <div class="flex justify-between items-start mb-8">
                <div>
                    <h1 class="text-3xl font-bold mb-1">"Channels"</h1>
                    <p class="text-gray-400">"Browse and pick a channel to watch"</p>
                </div>
                <button class="px-3 py-1.5 text-sm rounded-lg border border-red-500 text-red-500 bg-transparent hover:bg-red-500/15 transition-colors cursor-pointer" on:click=on_logout>"Sign Out"</button>
            </div>

            // ── Category filter ──
            <Suspense fallback=|| ()>
                {move || {
                    categories.get().flatten().map(|cats| {
                        if cats.categories.is_empty() {
                            return view! { <div></div> }.into_view();
                        }
                        view! {
                            <div class="flex gap-2 flex-wrap mb-4">
                                <button
                                    class="px-4 py-1.5 rounded-full text-sm cursor-pointer transition-all border"
                                    class=("bg-rose-500 text-white border-rose-500", move || selected_category.get().is_empty())
                                    class=("bg-gray-900 text-gray-400 border-white/10 hover:bg-rose-500 hover:text-white hover:border-rose-500", move || !selected_category.get().is_empty())
                                    on:click=move |_| set_selected_category.set(String::new())
                                >
                                    "All"
                                </button>
                                <For
                                    each=move || cats.categories.clone()
                                    key=|c| c.id.clone()
                                    children=move |cat| {
                                        let cat_id = cat.id.clone();
                                        let cat_id2 = cat.id.clone();
                                        let cat_id3 = cat.id.clone();
                                        view! {
                                            <button
                                                class="px-4 py-1.5 rounded-full text-sm cursor-pointer transition-all border"
                                                class=("bg-rose-500 text-white border-rose-500", move || selected_category.get() == cat_id)
                                                class=("bg-gray-900 text-gray-400 border-white/10 hover:bg-rose-500 hover:text-white hover:border-rose-500", move || selected_category.get() != cat_id3)
                                                on:click=move |_| set_selected_category
                                                    .set(cat_id2.clone())
                                            >
                                                {&cat.name}
                                            </button>
                                        }
                                    }
                                />
                            </div>
                        }.into_view()
                    })
                }}
            </Suspense>

            // ── Channel grid ──
            <Suspense fallback=move || view! { <div class="text-center py-12 text-gray-400">"Loading channels…"</div> }>
                {let navigate = navigate.clone(); move || {
                    let pid = provider_id();
                    let navigate = navigate.clone();
                    channels.get().map(|result| match result {
                        Ok(data) if data.channels.is_empty() => {
                            view! { <div class="text-center py-12 text-gray-400">"No channels found."</div> }.into_view()
                        }
                        Ok(data) => {
                            view! {
                                <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4 mt-4">
                                    <For
                                        each=move || data.channels.clone()
                                        key=|ch| ch.id.clone()
                                        children=move |channel| {
                                            let pid = pid.clone();
                                            let ch_id = channel.id.clone();
                                            let navigate = navigate.clone();
                                            view! {
                                                <div
                                                    class="bg-gray-900 border border-white/5 rounded-lg p-4 text-center hover:-translate-y-0.5 hover:border-rose-500 transition-all duration-150 cursor-pointer"
                                                    on:click=move |_| {
                                                        navigate(
                                                            &format!(
                                                                "/providers/{pid}/play/{ch_id}"
                                                            ),
                                                            Default::default(),
                                                        );
                                                    }
                                                >
                                                    {channel
                                                        .logo
                                                        .clone()
                                                        .map(|url| {
                                                            view! { <img class="w-18 h-18 object-contain rounded mx-auto mb-2" src=url alt="logo" /> }
                                                        })}
                                                    <div class="font-medium text-sm">{&channel.name}</div>
                                                    {channel
                                                        .number
                                                        .clone()
                                                        .map(|n| {
                                                            view! { <div class="text-xs text-gray-400">"CH " {n}</div> }
                                                        })}
                                                    {channel
                                                        .category
                                                        .clone()
                                                        .map(|c| {
                                                            view! { <div class="text-xs text-gray-400 mt-0.5">{c}</div> }
                                                        })}
                                                </div>
                                            }
                                        }
                                    />
                                </div>
                            }.into_view()
                        }
                        Err(e) => {
                            // Redirect to login if provider session is invalid
                            let navigate = navigate.clone();
                            navigate(&format!("/login/{pid}"), Default::default());
                            view! { <div class="text-red-400 bg-red-400/10 px-4 py-3 rounded-lg my-4 text-sm">{e}</div> }.into_view()
                        },
                    })
                }}
            </Suspense>
        </div>
    }
}
