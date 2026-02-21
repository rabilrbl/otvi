use leptos::*;
use leptos_router::*;

use crate::api;

/// Home page: lists all available TV providers.
#[component]
pub fn HomePage() -> impl IntoView {
    let providers = create_local_resource(|| (), |_| async move { api::fetch_providers().await });

    view! {
        <div class="container">
            <div class="page-header">
                <h1>"Choose Your Provider"</h1>
                <p>"Select a TV provider to sign in and start watching"</p>
            </div>

            <Suspense fallback=move || view! { <div class="loading">"Loading providers…"</div> }>
                {move || {
                    providers
                        .get()
                        .map(|result| match result {
                            Ok(list) if list.is_empty() => {
                                view! {
                                    <div class="loading">
                                        "No providers configured. Add a YAML file to the "
                                        <code>"providers/"</code>
                                        " directory."
                                    </div>
                                }
                                    .into_view()
                            }
                            Ok(list) => {
                                view! {
                                    <div class="providers-grid">
                                        <For
                                            each=move || list.clone()
                                            key=|p| p.id.clone()
                                            children=|provider| {
                                                let pid = provider.id.clone();
                                                let has_session = api::get_session(&pid).is_some();
                                                let href = if has_session {
                                                    format!("/providers/{}/channels", pid)
                                                } else {
                                                    format!("/login/{}", pid)
                                                };
                                                let flows_text = provider
                                                    .auth_flows
                                                    .iter()
                                                    .map(|f| f.name.clone())
                                                    .collect::<Vec<_>>()
                                                    .join(", ");
                                                let badge = if has_session {
                                                    view! { <div class="session-status signed-in">"Signed in ✓"</div> }.into_view()
                                                } else {
                                                    view! { <div class="session-status">"Sign in →"</div> }.into_view()
                                                };
                                                view! {
                                                    <A href=href class="provider-card">
                                                        {provider
                                                            .logo
                                                            .map(|url| {
                                                                view! { <img src=url alt="logo" /> }
                                                            })}

                                                        <h3>{provider.name}</h3>
                                                        <div class="flows">{flows_text}</div>
                                                        {badge}
                                                    </A>
                                                }
                                            }
                                        />

                                    </div>
                                }
                                    .into_view()
                            }
                            Err(e) => view! { <div class="error-msg">{e}</div> }.into_view(),
                        })
                }}

            </Suspense>
        </div>
    }
}
