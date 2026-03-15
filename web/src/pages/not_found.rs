use leptos::prelude::*;
use leptos_router::components::A;

#[component]
pub fn NotFoundPage() -> impl IntoView {
    view! {
        <div
            class="flex flex-col items-center justify-center min-h-[60vh] text-center px-4 gap-4"
            attr:data-testid="not-found-page"
        >
            <span class="text-7xl font-bold text-rose-500">"404"</span>
            <h1 class="text-2xl font-semibold text-gray-200">"Page not found"</h1>
            <p class="text-gray-400 text-sm">"The page you're looking for doesn't exist."</p>
            <A
                href="/"
                attr:class="mt-2 px-4 py-2 rounded-lg bg-rose-600 hover:bg-rose-500 text-white text-sm transition-colors"
            >
                "Go home"
            </A>
        </div>
    }
}
