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

    // Redirect to login if no session token exists
    {
        let navigate = navigate.clone();
        create_effect(move |_| {
            let pid = provider_id();
            if !pid.is_empty() && api::get_session(&pid).is_none() {
                navigate(&format!("/login/{pid}"), Default::default());
            }
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
            let _ = api::logout(&pid).await;
            nav("/", Default::default());
        });
    };

    view! {
        <div class="container">
            <div class="page-header" style="display:flex;justify-content:space-between;align-items:flex-start">
                <div>
                    <h1>"Channels"</h1>
                    <p>"Browse and pick a channel to watch"</p>
                </div>
                <button class="btn btn-small btn-danger" on:click=on_logout>"Sign Out"</button>
            </div>

            // ── Category filter ──
            <Suspense fallback=|| ()>
                {move || {
                    categories.get().flatten().map(|cats| {
                        if cats.categories.is_empty() {
                            return view! { <div></div> }.into_view();
                        }
                        view! {
                            <div class="categories-bar">
                                <button
                                    class="cat-tag"
                                    class:active=move || selected_category.get().is_empty()
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
                                        view! {
                                            <button
                                                class="cat-tag"
                                                class:active=move || selected_category.get() == cat_id
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
            <Suspense fallback=move || view! { <div class="loading">"Loading channels…"</div> }>
                {let navigate = navigate.clone(); move || {
                    let pid = provider_id();
                    let navigate = navigate.clone();
                    channels.get().map(|result| match result {
                        Ok(data) if data.channels.is_empty() => {
                            view! { <div class="loading">"No channels found."</div> }.into_view()
                        }
                        Ok(data) => {
                            view! {
                                <div class="channels-grid">
                                    <For
                                        each=move || data.channels.clone()
                                        key=|ch| ch.id.clone()
                                        children=move |channel| {
                                            let pid = pid.clone();
                                            let ch_id = channel.id.clone();
                                            let navigate = navigate.clone();
                                            view! {
                                                <div
                                                    class="channel-card"
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
                                                            view! { <img src=url alt="logo" /> }
                                                        })}
                                                    <div class="name">{&channel.name}</div>
                                                    {channel
                                                        .number
                                                        .clone()
                                                        .map(|n| {
                                                            view! { <div class="number">"CH " {n}</div> }
                                                        })}
                                                    {channel
                                                        .category
                                                        .clone()
                                                        .map(|c| {
                                                            view! { <div class="category">{c}</div> }
                                                        })}
                                                </div>
                                            }
                                        }
                                    />
                                </div>
                            }.into_view()
                        }
                        Err(e) => {
                            // Redirect to login if session is invalid
                            if e.contains("Not logged in") || e.contains("401") || e.contains("Unauthorized") {
                                api::clear_session(&pid);
                                let navigate = navigate.clone();
                                navigate(&format!("/login/{pid}"), Default::default());
                            }
                            view! { <div class="error-msg">{e}</div> }.into_view()
                        },
                    })
                }}
            </Suspense>
        </div>
    }
}
