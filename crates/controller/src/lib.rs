pub mod view {
    pub mod graph;
    pub mod buttons;
    pub mod messages_view;
    pub mod panel_view;
    pub mod graph_components;
}
pub mod utility;
pub mod controller_handler;
mod controller;

// Re-export
pub use view::graph::GraphApp;
pub use utility::*;