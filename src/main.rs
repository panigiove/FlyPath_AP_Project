use std::collections::HashMap;
use crossbeam_channel::unbounded;
use eframe::egui;

// Import dai crates del workspace
use controller::view::graph::GraphApp;
use controller::utility::{GraphAction, ButtonsMessages, MessageType, NodeType};
use wg_2024::network::NodeId;
use client::ui::UiState;
use controller::

fn main() -> Result<(), eframe::Error> {
    // Initialize empty collections for the network
    let drones = HashMap::new();
    let drones_type = HashMap::new();
    let drone_senders = HashMap::new();
    let clients = HashMap::new();
    let servers = HashMap::new();
    let connections = HashMap::new();
    let send_command_drone = HashMap::new();
    let send_command_node = HashMap::new();
    let reciver_event = HashMap::new();
    let receriver_node_event = HashMap::new();
    let client_ui_state = UiState::new();

    // Run the controller application
    controller::run_controller(
        drones,
        drones_type,
        drone_senders,
        clients,
        servers,
        connections,
        send_command_drone,
        send_command_node,
        reciver_event,
        receriver_node_event,
        client_ui_state,
    )
}

// Alternative main with some initial network setup
fn main_with_initial_network() -> Result<(), eframe::Error> {
    // You can add initial nodes here if needed
    let mut drones = HashMap::new();
    let mut drones_type = HashMap::new();
    let mut drone_senders = HashMap::new();
    let mut clients = HashMap::new();
    let mut servers = HashMap::new();
    let mut connections = HashMap::new();
    let mut send_command_drone = HashMap::new();
    let mut send_command_node = HashMap::new();
    let mut reciver_event = HashMap::new();
    let mut receriver_node_event = HashMap::new();
    let client_ui_state = UiState::new();

    // Example: Create initial drone
    // This would need to be done through the controller's spawn_drone method
    // after the app starts

    controller::run_controller(
        drones,
        drones_type,
        drone_senders,
        clients,
        servers,
        connections,
        send_command_drone,
        send_command_node,
        reciver_event,
        receriver_node_event,
        client_ui_state,
    )
}