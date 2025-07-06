use crossbeam_channel::unbounded;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use controller::{ButtonsMessages, ButtonEvent, GraphAction, MessageType, NodeType, GraphApp, ButtonWindow};
use wg_2024::network::NodeId;
use client::ui::{UiState, ClientState};

#[test]
fn test_button_window_creation_and_basic_functionality() {
    let (tx_node_clicked, rx_node_clicked) = unbounded::<NodeId>();
    let (tx_button_messages, _rx_button_messages) = unbounded::<ButtonsMessages>();
    let (tx_button_event, _rx_button_event) = unbounded::<ButtonEvent>();

    let mut button_window = ButtonWindow::new(
        rx_node_clicked,
        tx_button_messages,
        tx_button_event.clone(),
    );

    assert_eq!(button_window.node_id1, None);
    assert_eq!(button_window.node_id2, None);
    assert_eq!(button_window.selected_pdr, 0.1);

    tx_node_clicked.send(5).unwrap();
    button_window.handle_node_clicks();

    assert_eq!(button_window.node_id1, Some(5));
    assert_eq!(button_window.node_id2, None);

    tx_node_clicked.send(10).unwrap();
    button_window.handle_node_clicks();

    assert_eq!(button_window.node_id1, Some(5));
    assert_eq!(button_window.node_id2, Some(10));
}

#[test]
fn test_button_window_clear_selection() {
    let (_tx_node_clicked, rx_node_clicked) = unbounded::<NodeId>();
    let (tx_button_messages, rx_button_messages) = unbounded::<ButtonsMessages>();
    let (tx_button_event, _rx_button_event) = unbounded::<ButtonEvent>();

    let mut button_window = ButtonWindow::new(
        rx_node_clicked,
        tx_button_messages,
        tx_button_event,
    );

    button_window.node_id1 = Some(1);
    button_window.node_id2 = Some(2);

    button_window.clear_selection();

    assert_eq!(button_window.node_id1, None);
    assert_eq!(button_window.node_id2, None);

    match rx_button_messages.try_recv() {
        Ok(ButtonsMessages::ClearAllSelections) => {}
        _ => panic!("Messaggio ClearAllSelections non ricevuto"),
    }
}

#[test]
fn test_button_window_send_button_event() {
    // Setup
    let (_tx_node_clicked, rx_node_clicked) = unbounded::<NodeId>();
    let (tx_button_messages, _rx_button_messages) = unbounded::<ButtonsMessages>();
    let (tx_button_event, rx_button_event) = unbounded::<ButtonEvent>();

    let button_window = ButtonWindow::new(
        rx_node_clicked,
        tx_button_messages,
        tx_button_event,
    );

    let test_event = ButtonEvent::NewDrone(5, 0.75);
    button_window.send_button_event(test_event.clone());

    match rx_button_event.try_recv() {
        Ok(received_event) => {
            match (received_event, test_event) {
                (ButtonEvent::NewDrone(id1, pdr1), ButtonEvent::NewDrone(id2, pdr2)) => {
                    assert_eq!(id1, id2);
                    assert_eq!(pdr1, pdr2);
                }
                _ => panic!("Evento ricevuto non corrisponde"),
            }
        }
        _ => panic!("Evento non ricevuto"),
    }
}

#[test]
fn test_graph_app_creation() {
    let connections = HashMap::new();
    let node_types = HashMap::new();

    let (_tx_graph_action, rx_graph_action) = unbounded::<GraphAction>();
    let (tx_node_clicked, _rx_node_clicked) = unbounded::<NodeId>();
    let (_tx_button_messages, rx_button_messages) = unbounded::<ButtonsMessages>();
    let (tx_message_type, _rx_message_type) = unbounded::<MessageType>();
    let (_tx_client_state, rx_client_state) = unbounded::<(NodeId, ClientState)>();

    let client_ui_state = Arc::new(Mutex::new(UiState::new()));

    let graph_app = GraphApp::new(
        connections,
        node_types,
        rx_graph_action,
        tx_node_clicked,
        rx_button_messages,
        tx_message_type,
        client_ui_state,
        rx_client_state,
    );

    assert_eq!(graph_app.selected_nodes.len(), 0);
    assert_eq!(graph_app.selected_edge, None);
    assert_eq!(graph_app.labels_always, true);
    assert_eq!(graph_app.dragging_enabled, true);
}

#[test]
fn test_graph_app_basic_operations() {
    let mut connections = HashMap::new();
    let mut node_types = HashMap::new();

    node_types.insert(1, NodeType::Drone);
    connections.insert(1, Vec::new());

    let (_tx_graph_action, rx_graph_action) = unbounded::<GraphAction>();
    let (tx_node_clicked, _rx_node_clicked) = unbounded::<NodeId>();
    let (_tx_button_messages, rx_button_messages) = unbounded::<ButtonsMessages>();
    let (tx_message_type, _rx_message_type) = unbounded::<MessageType>();
    let (_tx_client_state, rx_client_state) = unbounded::<(NodeId, ClientState)>();

    let client_ui_state = Arc::new(Mutex::new(UiState::new()));

    let mut graph_app = GraphApp::new(
        connections,
        node_types,
        rx_graph_action,
        tx_node_clicked,
        rx_button_messages,
        tx_message_type,
        client_ui_state,
        rx_client_state,
    );

    let result = graph_app.add_node(2, NodeType::Client);
    assert!(result.is_ok());

    assert!(graph_app.node_id_to_index.contains_key(&2));

    let result = graph_app.add_edge(1, 2);
    assert!(result.is_ok());

    let result = graph_app.remove_edge(1, 2);
    assert!(result.is_ok());

    let result = graph_app.remove_node(2);
    assert!(result.is_ok());

    assert!(!graph_app.node_id_to_index.contains_key(&2));
}

#[test]
fn test_button_window_pdr_functionality() {

    let (_tx_node_clicked, rx_node_clicked) = unbounded::<NodeId>();
    let (tx_button_messages, _rx_button_messages) = unbounded::<ButtonsMessages>();
    let (tx_button_event, rx_button_event) = unbounded::<ButtonEvent>();

    let mut button_window = ButtonWindow::new(
        rx_node_clicked,
        tx_button_messages,
        tx_button_event,
    );

    assert_eq!(button_window.selected_pdr, 0.1);

    button_window.selected_pdr = 0.85;
    assert_eq!(button_window.selected_pdr, 0.85);

    button_window.node_id1 = Some(1);

    button_window.send_button_event(ButtonEvent::ChangePdr(1, 0.85));

    match rx_button_event.try_recv() {
        Ok(ButtonEvent::ChangePdr(node_id, pdr)) => {
            assert_eq!(node_id, 1);
            assert_eq!(pdr, 0.85);
        }
        _ => panic!("Evento ChangePdr non ricevuto"),
    }
}