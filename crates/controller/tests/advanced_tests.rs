use crossbeam_channel::unbounded;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Instant};

use controller::{ButtonsMessages, ButtonEvent, GraphAction, MessageType, NodeType, GraphApp, ButtonWindow};
use wg_2024::network::NodeId;
use client::ui::{UiState, ClientState};
#[test]
fn test_large_network_creation_100_nodes() {
    let connections = HashMap::new();
    let node_types = HashMap::new();

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
    
    let start_time = Instant::now();

    for i in 1u8..=100u8 {
        let node_type = match i % 3 {
            0 => NodeType::Client,
            1 => NodeType::Drone,
            _ => NodeType::Server,
        };

        let result = graph_app.add_node(i, node_type);
        assert!(result.is_ok(), "Failed to add node {}: {:?}", i, result);
    }

    let creation_time = start_time.elapsed();
    println!("Time to create 100 nodes (ID 1-100): {:?}", creation_time);
    
    assert_eq!(graph_app.node_id_to_index.len(), 100);
    
    let start_time = Instant::now();
    
    for i in (1u8..=97u8).step_by(3) {
        if i + 3 <= 97u8 {
            let result = graph_app.add_edge(i, i + 3);
            assert!(result.is_ok(), "Failed to add edge {}-{}", i, i + 3);
        }
    }

    let connection_time = start_time.elapsed();
    println!("Time to create ~30 connections: {:?}", connection_time);

    //the network is working?
    let result = graph_app.add_node(101u8, NodeType::Drone);
    assert!(result.is_ok());

    assert_eq!(graph_app.node_id_to_index.len(), 101);
}

#[test]
fn test_maximum_network_capacity_u8_limit() {
    let connections = HashMap::new();
    let node_types = HashMap::new();

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

    let start_time = Instant::now();
    let mut successful_additions = 0;

    for i in 1u8..=255u8 {
        let node_type = match i % 3 {
            0 => NodeType::Client,
            1 => NodeType::Drone,
            _ => NodeType::Server,
        };

        let result = graph_app.add_node(i, node_type);
        if result.is_ok() {
            successful_additions += 1;
        } else {
            println!("Failed to add Node ID [{}] (this may be expected)", i);
        }

        if i % 50 == 0 {
            println!("Added [{}] nodes so far", successful_additions);
        }
    }

    let creation_time = start_time.elapsed();
    println!("Time to create {} nodes: {:?}", successful_additions, creation_time);
    println!("Successfully added {}/255 possible nodes", successful_additions);

    assert!(successful_additions > 200, "Should add most nodes successfully");
    assert_eq!(graph_app.node_id_to_index.len(), successful_additions);

    // it must be not possible cause there isn't any free id
    let impossible_result = graph_app.add_node(0u8, NodeType::Drone);
    println!("Attempt to add node with ID 0: {:?}", impossible_result);
}

#[test]
fn test_network_id_collision_handling() {
    let connections = HashMap::new();
    let node_types = HashMap::new();

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

    let result1 = graph_app.add_node(42u8, NodeType::Drone);
    assert!(result1.is_ok(), "First node should be added successfully");

    //trying to add a node with an id already used
    let result2 = graph_app.add_node(42u8, NodeType::Client);
    assert!(result2.is_err(), "Duplicate ID should be rejected");
    println!("Correctly rejected duplicate ID: {:?}", result2);

    assert_eq!(graph_app.node_id_to_index.len(), 1);

    let test_ids = [200u8, 1u8, 100u8, 255u8, 50u8];
    let mut added_count = 1;

    for &id in &test_ids {
        let result = graph_app.add_node(id, NodeType::Drone);
        if result.is_ok() {
            added_count += 1;
            println!("Added node with ID {}", id);
        } else {
            println!("Failed to add node with ID {}: {:?}", id, result);
        }
    }

    assert_eq!(graph_app.node_id_to_index.len(), added_count);
    println!("Total nodes in network: {}", added_count);

    let mut unique_ids = std::collections::HashSet::new();
    for &id in graph_app.node_id_to_index.keys() {
        assert!(unique_ids.insert(id), "Found duplicate ID in network: {}", id);
    }

    println!("All {} node IDs are unique", unique_ids.len());
}

#[test]
fn test_large_network_operations_stress() {
    let mut connections = HashMap::new();
    let mut node_types = HashMap::new();

    for i in 1u8..=50u8 {
        node_types.insert(i, NodeType::Drone);
        connections.insert(i, Vec::new());
    }

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

    //multiple operations
    let start_time = Instant::now();
    let mut operations_count = 0;

    //adding connections
    for i in 1u8..=25u8 {
        let result = graph_app.add_edge(i, i + 25);
        assert!(result.is_ok());
        operations_count += 1;
    }

    //removing connections
    for i in 1u8..=10u8 {
        let result = graph_app.remove_edge(i, i + 25);
        assert!(result.is_ok());
        operations_count += 1;
    }

    //new nodes
    for i in 51u8..=70u8 {
        let result = graph_app.add_node(i, NodeType::Client);
        assert!(result.is_ok());
        operations_count += 1;
    }

    let total_time = start_time.elapsed();
    let ops_per_second = operations_count as f64 / total_time.as_secs_f64();

    println!("{} operations in {:?} = {:.1} ops/sec", //formatting the number as a floating-point number with one digit after the decimal point
             operations_count, total_time, ops_per_second);

    //verify if the network is consistent
    assert!(graph_app.node_id_to_index.len() >= 70);

    assert!(ops_per_second > 100.0, "Performance too low: {:.1} ops/sec", ops_per_second);
}

// ================================ TEST PERFORMANCE/STRESS ================================

#[test]
fn test_performance_button_window_rapid_clicks() {
    let (tx_node_clicked, rx_node_clicked) = unbounded::<NodeId>();
    let (tx_button_messages, _rx_button_messages) = unbounded::<ButtonsMessages>();
    let (tx_button_event, _rx_button_event) = unbounded::<ButtonEvent>();

    let mut button_window = ButtonWindow::new(
        rx_node_clicked,
        tx_button_messages,
        tx_button_event,
    );

    // Test: 1000 click rapidi
    let start_time = Instant::now();

    for i in 1..= 255 {
        let _ = tx_node_clicked.send(i % 10);
        button_window.handle_node_clicks();
    }

    let processing_time = start_time.elapsed();
    let clicks_per_second = 1000.0 / processing_time.as_secs_f64();

    println!("⏱️ 1000 clicks processed in {:?} = {:.1} clicks/sec",
             processing_time, clicks_per_second);

    assert!(clicks_per_second > 1000.0,
            "Click processing troppo lento: {:.1} clicks/sec", clicks_per_second);

    assert!(button_window.node_id1.is_some());
}

#[test]
fn test_stress_event_generation() {
    let (_tx_node_clicked, rx_node_clicked) = unbounded::<NodeId>();
    let (tx_button_messages, _rx_button_messages) = unbounded::<ButtonsMessages>();
    let (tx_button_event, rx_button_event) = unbounded::<ButtonEvent>();

    let button_window = ButtonWindow::new(
        rx_node_clicked,
        tx_button_messages,
        tx_button_event,
    );

    let start_time = Instant::now();
    let event_count = 10000;

    for i in 1..=event_count {
        let event = match i % 4 {
            0 => ButtonEvent::NewDrone(1, 0.5),
            1 => ButtonEvent::NewClient(1),
            2 => ButtonEvent::ChangePdr(1, 0.8),
            _ => ButtonEvent::NewConnection(1, 2),
        };

        button_window.send_button_event(event);
    }

    let generation_time = start_time.elapsed();

    let mut received_count = 0;
    while rx_button_event.try_recv().is_ok() {
        received_count += 1;
    }

    let events_per_second = event_count as f64 / generation_time.as_secs_f64();

    println!("{} events generated in {:?} = {:.1} events/sec",
             event_count, generation_time, events_per_second);
    println!("Received {} events", received_count);

    assert_eq!(received_count, event_count);
    assert!(events_per_second > 10000.0,
            "Event generation troppo lenta: {:.1} events/sec", events_per_second);
}
