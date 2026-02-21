use leptos::*;
use leptos_router::*;

use crate::pages::{channels::ChannelsPage, home::HomePage, login::LoginPage, player::PlayerPage};

/// Root application component with navigation and routing.
#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <nav class="nav">
                <a class="nav-logo" href="/">"OTVI"</a>
                <div class="nav-actions">
                    <a class="btn btn-small btn-secondary" href="/">"Providers"</a>
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
