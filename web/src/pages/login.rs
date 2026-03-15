use std::collections::HashMap;

use leptos::either::Either;
use leptos::ev;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::*;

use otvi_core::types::*;

use crate::api;

/// Login page: renders a dynamic form based on the provider's auth flows.
#[component]
pub fn LoginPage() -> impl IntoView {
    let params = use_params_map();
    let provider_id = move || params.with(|p| p.get("provider_id").unwrap_or_default());

    // Fetch provider metadata (including auth flows)
    let provider = LocalResource::new(move || {
        let id = provider_id();
        async move { api::fetch_provider(&id).await }
    });

    // UI state
    let (selected_flow_idx, set_selected_flow_idx) = signal(0usize);
    let (inputs, set_inputs) = signal(HashMap::<String, String>::new());
    let (error, set_error) = signal(Option::<String>::None);
    let (loading, set_loading) = signal(false);

    // Multi-step state
    let (session_id, set_session_id) = signal(Option::<String>::None);
    let (current_step, set_current_step) = signal(0usize);
    let (extra_fields, set_extra_fields) = signal(Vec::<FieldInfo>::new());

    let navigate = use_navigate();

    // If the user already has an active provider session, skip to channels.
    {
        let navigate = navigate.clone();
        Effect::new(move |_| {
            let pid = provider_id();
            if pid.is_empty() {
                return;
            }
            let nav = navigate.clone();
            spawn_local(async move {
                if api::check_provider_session(&pid).await {
                    nav(&format!("/providers/{pid}/channels"), Default::default());
                }
            });
        });
    }

    // Handle form submission
    let on_submit = move |ev: ev::SubmitEvent| {
        ev.prevent_default();
        let pid = provider_id();
        let provider_info = provider.get_untracked();
        let flow_idx = selected_flow_idx.get_untracked();
        let step = current_step.get_untracked();
        let current_inputs = inputs.get_untracked();
        let sid = session_id.get_untracked();
        let navigate = navigate.clone();

        set_loading.set(true);
        set_error.set(None);

        spawn_local(async move {
            let provider_info = match provider_info {
                Some(Ok(info)) => info,
                Some(Err(e)) => {
                    set_error.set(Some(e));
                    set_loading.set(false);
                    return;
                }
                None => {
                    set_error.set(Some("Provider details are still loading".into()));
                    set_loading.set(false);
                    return;
                }
            };

            let flow = match provider_info.auth_flows.get(flow_idx) {
                Some(f) => f,
                None => {
                    set_error.set(Some("Invalid flow".into()));
                    set_loading.set(false);
                    return;
                }
            };

            let req = LoginRequest {
                flow_id: flow.id.clone(),
                step,
                inputs: current_inputs,
                session_id: sid,
            };

            match api::login(&pid, &req).await {
                Ok(resp) => {
                    if resp.success {
                        // Session is now stored server-side via JWT sub.
                        navigate(&format!("/providers/{pid}/channels"), Default::default());
                    } else if let Some(next) = resp.next_step {
                        set_session_id.set(resp.session_id);
                        set_current_step.set(next.step_index);
                        set_extra_fields.set(next.fields);
                    } else if let Some(err) = resp.error {
                        set_error.set(Some(err));
                    }
                }
                Err(e) => set_error.set(Some(e)),
            }
            set_loading.set(false);
        });
    };

    view! {
        <div class="max-w-7xl mx-auto px-6 py-8" attr:data-testid="provider-login-page">
            <div class="max-w-md mx-auto mt-12">
                <Suspense fallback=move || view! { <div class="text-center py-12 text-gray-400">"Loading…"</div> }>
                    {let on_submit = on_submit.clone(); move || {
                        let on_submit = on_submit.clone();
                        provider
                            .get()
                            .map(|result| match result {
                                Ok(info) => {
                                    let info_stored = StoredValue::new(info.clone());
                                    Either::Left(view! {
                                        <div class="bg-gray-900 rounded-lg p-8">
                                            <h2 class="text-center text-xl font-semibold mb-6">{format!("Sign in to {}", info.name)}</h2>

                                            // ── Flow selector tabs ──
                                            <Show when=move || info_stored.get_value().auth_flows.len().gt(&1)>
                                                <div class="flex gap-2 mb-6">
                                                    <For
                                                        each=move || {
                                                            info_stored.get_value()
                                                                .auth_flows
                                                                .iter()
                                                                .enumerate()
                                                                .map(|(i, f)| (i, f.name.clone()))
                                                                .collect::<Vec<_>>()
                                                        }
                                                        key=|(i, _)| *i
                                                        children=move |(i, name)| {
                                                            view! {
                                                                <button
                                                                    class="flex-1 py-2.5 rounded-lg text-sm text-center transition-all border cursor-pointer"
                                                                    class=("bg-indigo-900 text-gray-200 border-indigo-900", move || selected_flow_idx.get() == i)
                                                                    class=("bg-gray-950 text-gray-400 border-white/10 hover:bg-indigo-900/50", move || selected_flow_idx.get() != i)
                                                                    on:click=move |_| {
                                                                        set_selected_flow_idx.set(i);
                                                                        set_inputs.set(HashMap::new());
                                                                        set_current_step.set(0);
                                                                        set_extra_fields.set(vec![]);
                                                                        set_session_id.set(None);
                                                                        set_error.set(None);
                                                                    }
                                                                >
                                                                    {name}
                                                                </button>
                                                            }
                                                        }
                                                    />
                                                </div>
                                            </Show>

                                            // ── Dynamic form ──
                                            <form on:submit=on_submit>
                                                // Initial fields for the selected flow
                                                <For
                                                    each=move || {
                                                        info_stored.get_value()
                                                            .auth_flows
                                                            .get(selected_flow_idx.get())
                                                            .map(|f| f.fields.clone())
                                                            .unwrap_or_default()
                                                    }
                                                    key=|f| f.key.clone()
                                                    children=move |field| {
                                                        render_field(field, set_inputs)
                                                    }
                                                />

                                                // Extra fields from multi-step prompt
                                                <For
                                                    each=move || extra_fields.get()
                                                    key=|f| f.key.clone()
                                                    children=move |field| {
                                                        render_field(field, set_inputs)
                                                    }
                                                />

                                                // Error message
                                                <Show when=move || error.get().is_some()>
                                                    <div class="text-red-400 bg-red-400/10 px-4 py-3 rounded-lg my-4 text-sm">{move || error.get()}</div>
                                                </Show>

                                                <button
                                                    type="submit"
                                                    class="w-full py-3 rounded-lg bg-rose-500 text-white font-medium hover:bg-rose-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer"
                                                    disabled=move || loading.get()
                                                >
                                                    {move || {
                                                        if loading.get() {
                                                            "Signing in…"
                                                        } else if current_step.get() > 0 {
                                                            "Verify"
                                                        } else {
                                                            "Sign In"
                                                        }
                                                    }}
                                                </button>
                                            </form>
                                        </div>
                                    }
                                    )
                                }
                                Err(e) => Either::Right(view! { <div class="text-red-400 bg-red-400/10 px-4 py-3 rounded-lg my-4 text-sm">{e}</div> }),
                            })
                    }}
                </Suspense>
            </div>
        </div>
    }
}

/// Render a single form field and wire its value into the shared `inputs` map.
fn render_field(
    field: FieldInfo,
    set_inputs: WriteSignal<HashMap<String, String>>,
) -> impl IntoView {
    let key = field.key.clone();
    let key2 = field.key.clone();

    view! {
        <div class="mb-4">
            <label class="block mb-1.5 text-gray-400 text-sm">{field.label.clone()}</label>
            <input
                type=field.field_type.clone()
                required=field.required
                placeholder=field.label.clone()
                class="w-full px-4 py-3 bg-gray-950 border border-white/10 rounded-lg text-gray-200 text-base focus:outline-none focus:border-rose-500 transition-colors"
                on:input=move |ev| {
                    let value = event_target_value(&ev);
                    let k = key.clone();
                    set_inputs
                        .update(|map| {
                            map.insert(k, value);
                        });
                }
                prop:value=String::new
                name=key2
            />
        </div>
    }
}
