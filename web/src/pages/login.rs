use std::collections::HashMap;

use leptos::*;
use leptos_router::*;

use otvi_core::types::*;

use crate::api;

/// Login page: renders a dynamic form based on the provider's auth flows.
#[component]
pub fn LoginPage() -> impl IntoView {
    let params = use_params_map();
    let provider_id =
        move || params.with(|p| p.get("provider_id").cloned().unwrap_or_default());

    // Fetch provider metadata (including auth flows)
    let provider = create_local_resource(provider_id, |id| async move {
        api::fetch_provider(&id).await
    });

    // UI state
    let (selected_flow_idx, set_selected_flow_idx) = create_signal(0usize);
    let (inputs, set_inputs) = create_signal(HashMap::<String, String>::new());
    let (error, set_error) = create_signal(Option::<String>::None);
    let (loading, set_loading) = create_signal(false);

    // Multi-step state
    let (session_id, set_session_id) = create_signal(Option::<String>::None);
    let (current_step, set_current_step) = create_signal(0usize);
    let (extra_fields, set_extra_fields) = create_signal(Vec::<FieldInfo>::new());

    let navigate = use_navigate();

    // If already logged in, redirect straight to channels
    {
        let navigate = navigate.clone();
        create_effect(move |_| {
            let pid = provider_id();
            if !pid.is_empty() && api::get_session(&pid).is_some() {
                let nav = navigate.clone();
                spawn_local(async move {
                    if api::check_session(&pid).await {
                        nav(&format!("/providers/{pid}/channels"), Default::default());
                    }
                });
            }
        });
    }

    // Handle form submission
    let on_submit = move |ev: ev::SubmitEvent| {
        ev.prevent_default();
        let pid = provider_id();
        let flow_idx = selected_flow_idx.get_untracked();
        let step = current_step.get_untracked();
        let current_inputs = inputs.get_untracked();
        let sid = session_id.get_untracked();
        let navigate = navigate.clone();

        set_loading.set(true);
        set_error.set(None);

        spawn_local(async move {
            let provider_info = match api::fetch_provider(&pid).await {
                Ok(p) => p,
                Err(e) => {
                    set_error.set(Some(e));
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
                        if let Some(sid) = &resp.session_id {
                            api::store_session(&pid, sid);
                        }
                        navigate(
                            &format!("/providers/{pid}/channels"),
                            Default::default(),
                        );
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
        <div class="container">
            <div class="login-container">
                <Suspense fallback=move || view! { <div class="loading">"Loading…"</div> }>
                    {let on_submit = on_submit.clone(); move || {
                        let on_submit = on_submit.clone();
                        provider
                            .get()
                            .map(|result| match result {
                                Ok(info) => {
                                    let info_stored = store_value(info.clone());
                                    view! {
                                        <div class="login-form">
                                            <h2>{format!("Sign in to {}", info.name)}</h2>

                                            // ── Flow selector tabs ──
                                            <Show when=move || info_stored.get_value().auth_flows.len().gt(&1)>
                                                <div class="flow-tabs">
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
                                                                    class="flow-tab"
                                                                    class:active=move || selected_flow_idx.get() == i
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
                                                    <div class="error-msg">{move || error.get()}</div>
                                                </Show>

                                                <button
                                                    type="submit"
                                                    class="btn btn-primary"
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
                                        .into_view()
                                }
                                Err(e) => view! { <div class="error-msg">{e}</div> }.into_view(),
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
        <div class="form-group">
            <label>{&field.label}</label>
            <input
                type=field.field_type.clone()
                required=field.required
                placeholder=field.label.clone()
                on:input=move |ev| {
                    let value = event_target_value(&ev);
                    let k = key.clone();
                    set_inputs
                        .update(|map| {
                            map.insert(k, value);
                        });
                }
                prop:value=move || String::new()
                name=key2
            />
        </div>
    }
}
