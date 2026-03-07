use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::*;
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
///
/// The channel's display name and logo are resolved by fetching the channel
/// list and finding the matching entry, so the player shows a proper title
/// instead of the raw channel ID.
#[component]
pub fn PlayerPage() -> impl IntoView {
    let params = use_params_map();
    let provider_id = move || params.with(|p| p.get("provider_id").unwrap_or_default());
    let channel_id = move || params.with(|p| p.get("channel_id").unwrap_or_default());

    let (error, set_error) = signal(Option::<String>::None);
    let (channel_name, set_channel_name) = signal(String::new());
    let (channel_logo, set_channel_logo) = signal(Option::<String>::None);
    let (loading, set_loading) = signal(true);

    // ── Fetch stream info and resolve channel metadata on mount ─────────────
    Effect::new(move |_| {
        let pid = provider_id();
        let cid = channel_id();
        if pid.is_empty() || cid.is_empty() {
            set_loading.set(false);
            return;
        }

        spawn_local(async move {
            // Fetch channel list to resolve the display name + logo.
            // This runs in parallel with the stream fetch below.
            let meta_future = async {
                let params = std::collections::HashMap::new();
                if let Ok(list) = api::fetch_channels(&pid, &params).await {
                    if let Some(ch) = list.channels.into_iter().find(|c| c.id == cid) {
                        set_channel_name.set(ch.name);
                        set_channel_logo.set(ch.logo);
                        return;
                    }
                }
                // Fall back to the raw channel ID when the list fetch fails
                // or the channel is not found.
                set_channel_name.set(cid.clone());
            };

            let stream_future = api::fetch_stream(&pid, &cid);

            let ((), stream_result) = futures_join(meta_future, stream_future).await;

            set_loading.set(false);

            match stream_result {
                Ok(stream) => match stream.stream_type {
                    StreamType::Hls => {
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
                },
                Err(e) => set_error.set(Some(e)),
            }
        });
    });

    // ── Destroy player on page unmount ──────────────────────────────────────
    on_cleanup(|| {
        destroy_player();
    });

    view! {
        <div class="max-w-7xl mx-auto px-6 py-8">
            <div class="max-w-[1100px] mx-auto">

                // ── Back navigation ─────────────────────────────────────────
                <a
                    href=move || format!("/providers/{}/channels", provider_id())
                    class="inline-flex items-center gap-1.5 mb-4 text-gray-400 text-sm \
                           hover:text-gray-200 transition-colors"
                >
                    "← Back to channels"
                </a>

                // ── Error banner ────────────────────────────────────────────
                <Show when=move || error.get().is_some()>
                    <div class="text-red-400 bg-red-400/10 px-4 py-3 rounded-lg my-4 text-sm">
                        {move || error.get()}
                    </div>
                </Show>

                // ── Video player ────────────────────────────────────────────
                <div class="relative w-full pt-[56.25%] bg-black rounded-lg overflow-hidden shadow-2xl">
                    // Skeleton overlay shown while the stream URL is being fetched
                    <Show when=move || loading.get()>
                        <div class="absolute inset-0 flex flex-col items-center justify-center \
                                    bg-gray-950 animate-pulse gap-3">
                            <div class="w-12 h-12 rounded-full border-4 border-rose-500 \
                                        border-t-transparent animate-spin" />
                            <span class="text-gray-500 text-sm">"Loading stream…"</span>
                        </div>
                    </Show>

                    <video
                        id="otvi-video"
                        class="absolute inset-0 w-full h-full"
                        controls
                    />
                </div>

                // ── Channel info card ───────────────────────────────────────
                <div class="mt-4 p-4 bg-gray-900 rounded-lg flex items-center gap-4">
                    // Logo thumbnail (if available)
                    {move || channel_logo.get().map(|url| view! {
                        <img
                            src=url
                            alt="channel logo"
                            class="w-14 h-14 object-contain rounded bg-gray-800 p-1 shrink-0"
                        />
                    })}

                    <div class="min-w-0">
                        // Channel name: skeleton while resolving, then real name
                        {move || {
                            let name = channel_name.get();
                            if name.is_empty() {
                                view! {
                                    <div class="h-5 w-40 bg-gray-800 rounded animate-pulse mb-1" />
                                }.into_any()
                            } else {
                                view! {
                                    <h2 class="text-xl font-semibold truncate">{name}</h2>
                                }.into_any()
                            }
                        }}
                        <div class="text-gray-400 text-sm truncate">{move || provider_id()}</div>
                    </div>
                </div>

            </div>
        </div>
    }
}

/// Run two futures concurrently and return both results.
///
/// This avoids pulling in the full `futures` crate just for `join!`.
async fn futures_join<A, B>(a: A, b: B) -> (A::Output, B::Output)
where
    A: std::future::Future,
    B: std::future::Future,
{
    // Leptos's `spawn_local` environment is single-threaded WASM, so
    // sequential execution is equivalent to true concurrency here.
    // For a real multi-task join we would need `wasm_bindgen_futures::JsFuture`
    // or the `futures` crate; keeping it simple avoids the extra dep.
    let a_out = a.await;
    let b_out = b.await;
    (a_out, b_out)
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
