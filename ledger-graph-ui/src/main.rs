mod components;
mod config;
mod models;
mod server;
mod state;

use components::app::App;

fn main() {
    dioxus::launch(App);
}
