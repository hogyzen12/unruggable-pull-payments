use dioxus::prelude::*;

mod api;
mod wallet;
mod components;
mod timed_delegation;
mod rpc;

use components::DelegationModal;

fn main() {
    dioxus_logger::init(dioxus_logger::tracing::Level::INFO).expect("failed to init logger");
    launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        div {
            style: "width: 100%; height: 100vh; display: flex; align-items: center; justify-content: center; background: #0f172a;",
            DelegationModal {}
        }
    }
}
