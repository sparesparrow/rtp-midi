use yew::prelude::*;

#[function_component(App)]
pub fn app() -> Html {
    html! { <h1>{ "rtp-midi UI (WASM/Yew)" }</h1> }
}

#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn run_app() {
    yew::Renderer::<App>::new().render();
}
