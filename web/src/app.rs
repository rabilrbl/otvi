use leptos::*;
use leptos_router::*;

use crate::pages::{channels::ChannelsPage, home::HomePage, login::LoginPage, player::PlayerPage};

/// Root application component with navigation and routing.
#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <nav class="bg-gray-900 px-6 py-3 flex items-center justify-between sticky top-0 z-50 shadow-lg shadow-black/30">
                <a class="text-xl font-bold text-rose-500 hover:text-rose-400 transition-colors" href="/">"OTVI"</a>
                <div class="flex gap-3 items-center">
                    <a class="px-3 py-1.5 text-sm rounded-lg bg-indigo-900 text-gray-200 hover:bg-indigo-800 transition-colors" href="/">"Providers"</a>
                </div>
            </nav>
            <main>
                <Routes>
                    <Route path="/" view=HomePage />
                    <Route path="/login/:provider_id" view=LoginPage />
                    <Route path="/providers/:provider_id/channels" view=ChannelsPage />
                    <Route path="/providers/:provider_id/play/:channel_id" view=PlayerPage />
                </Routes>
            </main>
        </Router>
    }
}
