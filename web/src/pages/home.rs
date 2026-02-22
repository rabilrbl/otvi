use leptos::*;

use crate::api;

/// Home page: lists all available TV providers.
#[component]
pub fn HomePage() -> impl IntoView {
    let providers = create_local_resource(|| (), |_| async move { api::fetch_providers().await });

    view! {
        <div class="max-w-7xl mx-auto px-6 py-8">
            <div class="mb-8">
                <h1 class="text-3xl font-bold mb-1">"Choose Your Provider"</h1>
                <p class="text-gray-400">"Select a TV provider to sign in and start watching"</p>
            </div>

            <Suspense fallback=move || view! { <div class="text-center py-12 text-gray-400">"Loading providers…"</div> }>
                {move || {
                    providers.get().map(|result| match result {
                        Ok(list) if list.is_empty() => view! {
                            <div class="text-center py-12 text-gray-400">
                                "No providers configured. Add a YAML file to the "
                                <code class="bg-gray-800 px-1.5 py-0.5 rounded text-sm">"providers/"</code>
                                " directory."
                            </div>
                        }.into_view(),

                        Ok(list) => view! {
                            <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-6 mt-6">
                                <For
                                    each=move || list.clone()
                                    key=|p| p.id.clone()
                                    children=move |provider| {
                                        let pid = provider.id.clone();
                                        let pid2 = pid.clone();
                                        // Check provider session status asynchronously.
                                        let session_status = create_local_resource(
                                            move || pid2.clone(),
                                            |id| async move { api::check_provider_session(&id).await },
                                        );
                                        let flows_text = provider
                                            .auth_flows
                                            .iter()
                                            .map(|f| f.name.clone())
                                            .collect::<Vec<_>>()
                                            .join(", ");
                                        view! {
                                            <a
                                                href=move || {
                                                    if session_status.get().unwrap_or(false) {
                                                        format!("/providers/{}/channels", pid)
                                                    } else {
                                                        format!("/login/{}", pid)
                                                    }
                                                }
                                                class="block bg-gray-900 border border-white/5 rounded-lg p-6 hover:-translate-y-1 hover:border-rose-500 transition-all duration-200 cursor-pointer no-underline"
                                            >
                                                {provider.logo.map(|url| view! {
                                                    <img class="max-w-full h-15 object-contain mb-4" src=url alt="logo" />
                                                })}
                                                <h3 class="font-semibold text-lg mb-1">{provider.name}</h3>
                                                <div class="text-sm text-gray-400">{flows_text}</div>
                                                {move || {
                                                    match session_status.get() {
                                                        Some(true) => view! {
                                                            <div class="text-sm text-emerald-400 mt-2 font-medium">"Signed in ✓"</div>
                                                        }.into_view(),
                                                        _ => view! {
                                                            <div class="text-sm text-gray-400 mt-2">"Sign in →"</div>
                                                        }.into_view(),
                                                    }
                                                }}
                                            </a>
                                        }
                                    }
                                />
                            </div>
                        }.into_view(),

                        Err(_) => view! {
                            <div class="text-center py-16 text-gray-400">
                                <p class="text-lg mb-2">"Could not load providers."</p>
                            </div>
                        }.into_view(),
                    })
                }}
            </Suspense>
        </div>
    }
}
