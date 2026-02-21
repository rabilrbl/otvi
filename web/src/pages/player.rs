use leptos::*;
use leptos_router::*;
use wasm_bindgen::prelude::*;

use otvi_core::types::StreamType;

use crate::api;

// ── JS bridge ───────────────────────────────────────────────────────────────

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = otviInitHls)]
    fn init_hls_player(video_id: &str, url: &str);

    #[wasm_bindgen(js_name = otviInitDash)]
    fn init_dash_player(video_id: &str, url: &str, drm_config_json: &str);

    #[wasm_bindgen(js_name = otviDestroyPlayer)]
    fn destroy_player();
}

/// Video player page: fetches stream info and initialises HLS or DASH playback.
#[component]
pub fn PlayerPage() -> impl IntoView {
    let params = use_params_map();
    let provider_id =
        move || params.with(|p| p.get("provider_id").cloned().unwrap_or_default());
    let channel_id =
        move || params.with(|p| p.get("channel_id").cloned().unwrap_or_default());

    let (error, set_error) = create_signal(Option::<String>::None);
    let (channel_name, set_channel_name) = create_signal(String::new());

    // Fetch stream and init player on mount
    create_effect(move |_| {
        let pid = provider_id();
        let cid = channel_id();
        if pid.is_empty() || cid.is_empty() {
            return;
        }

        spawn_local(async move {
            match api::fetch_stream(&pid, &cid).await {
                Ok(stream) => {
                    set_channel_name.set(cid.clone());
                    match stream.stream_type {
                        StreamType::Hls => {
                            // Small delay to ensure <video> element is in DOM
                            gloo_timers_delay(100).await;
                            init_hls_player("otvi-video", &stream.url);
                        }
                        StreamType::Dash => {
                            let drm_json = stream
                                .drm
                                .as_ref()
                                .map(|d| serde_json::to_string(d).unwrap_or_default())
                                .unwrap_or_default();
                            gloo_timers_delay(100).await;
                            init_dash_player("otvi-video", &stream.url, &drm_json);
                        }
                    }
                }
                Err(e) => set_error.set(Some(e)),
            }
        });
    });

    // Destroy player on unmount
    on_cleanup(|| {
        destroy_player();
    });

    view! {
        <div class="container">
            <div class="player-container">
                <A
                    href=move || format!("/providers/{}/channels", provider_id())
                    class="back-link"
                >
                    "← Back to channels"
                </A>

                <Show when=move || error.get().is_some()>
                    <div class="error-msg">{move || error.get()}</div>
                </Show>

                <div class="video-wrapper">
                    <video id="otvi-video" controls></video>
                </div>

                <div class="player-info">
                    <h2>{move || {
                        let name = channel_name.get();
                        if name.is_empty() { "Loading…".into() } else { name }
                    }}</h2>
                    <div class="meta">{move || provider_id()}</div>
                </div>
            </div>
        </div>
    }
}

/// Tiny async delay helper (avoids pulling in the full gloo-timers crate).
async fn gloo_timers_delay(ms: u32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let _ = web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms as i32);
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}
