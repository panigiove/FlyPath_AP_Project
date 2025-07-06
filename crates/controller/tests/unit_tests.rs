use crossbeam_channel::unbounded;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// Import corretti basati sui re-export nel lib.rs
use controller::{ButtonsMessages, ButtonEvent, GraphAction, MessageType, NodeType, GraphApp, ButtonWindow};
use wg_2024::network::NodeId;
use client::ui::{UiState, ClientState};

#[test]
fn test_button_window_creation_and_basic_functionality() {
    // Setup canali
    let (tx_node_clicked, rx_node_clicked) = unbounded::<NodeId>();
    let (tx_button_messages, _rx_button_messages) = unbounded::<ButtonsMessages>();
    let (tx_button_event, rx_button_event) = unbounded::<ButtonEvent>();

    // Crea ButtonWindow usando il vero costruttore
    let mut button_window = ButtonWindow::new(
        rx_node_clicked,
        tx_button_messages,
        tx_button_event.clone(),
    );

    // Test: stato iniziale
    assert_eq!(button_window.node_id1, None);
    assert_eq!(button_window.node_id2, None);
    assert_eq!(button_window.selected_pdr, 0.1);

    // Test: simula click su nodo
    tx_node_clicked.send(5).unwrap();
    button_window.handle_node_clicks();

    // Verifica che il primo nodo sia selezionato
    assert_eq!(button_window.node_id1, Some(5));
    assert_eq!(button_window.node_id2, None);

    // Test: simula secondo click
    tx_node_clicked.send(10).unwrap();
    button_window.handle_node_clicks();

    // Verifica che ora ci siano due nodi selezionati
    assert_eq!(button_window.node_id1, Some(5));
    assert_eq!(button_window.node_id2, Some(10));
}

#[test]
fn test_button_window_clear_selection() {
    // Setup
    let (_tx_node_clicked, rx_node_clicked) = unbounded::<NodeId>();
    let (tx_button_messages, rx_button_messages) = unbounded::<ButtonsMessages>();
    let (tx_button_event, _rx_button_event) = unbounded::<ButtonEvent>();

    let mut button_window = ButtonWindow::new(
        rx_node_clicked,
        tx_button_messages,
        tx_button_event,
    );

    // Imposta manualmente alcune selezioni
    button_window.node_id1 = Some(1);
    button_window.node_id2 = Some(2);

    // Test: pulisci selezione
    button_window.clear_selection();

    // Verifica che tutto sia stato pulito
    assert_eq!(button_window.node_id1, None);
    assert_eq!(button_window.node_id2, None);

    // Verifica che sia stato inviato il messaggio
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

    let mut button_window = ButtonWindow::new(
        rx_node_clicked,
        tx_button_messages,
        tx_button_event,
    );

    // Test: invia evento
    let test_event = ButtonEvent::NewDrone(5, 0.75);
    button_window.send_button_event(test_event.clone());

    // Verifica che l'evento sia stato ricevuto
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
    // Setup dati iniziali
    let connections = HashMap::new();
    let node_types = HashMap::new();

    // Setup canali
    let (_tx_graph_action, rx_graph_action) = unbounded::<GraphAction>();
    let (tx_node_clicked, _rx_node_clicked) = unbounded::<NodeId>();
    let (_tx_button_messages, rx_button_messages) = unbounded::<ButtonsMessages>();
    let (tx_message_type, _rx_message_type) = unbounded::<MessageType>();
    let (_tx_client_state, rx_client_state) = unbounded::<(NodeId, ClientState)>();

    // Setup UI state
    let client_ui_state = Arc::new(Mutex::new(UiState::new()));

    // Crea GraphApp usando SOLO il vero costruttore
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

    // Test: verifica stato iniziale
    assert_eq!(graph_app.selected_nodes.len(), 0);
    assert_eq!(graph_app.selected_edge, None);
    assert_eq!(graph_app.labels_always, true);
    assert_eq!(graph_app.dragging_enabled, true);
}

#[test]
fn test_graph_app_basic_operations() {
    // Setup dati iniziali con un nodo
    let mut connections = HashMap::new();
    let mut node_types = HashMap::new();

    // Aggiungi un nodo di test
    node_types.insert(1, NodeType::Drone);
    connections.insert(1, Vec::new());

    // Setup canali
    let (_tx_graph_action, rx_graph_action) = unbounded::<GraphAction>();
    let (tx_node_clicked, _rx_node_clicked) = unbounded::<NodeId>();
    let (_tx_button_messages, rx_button_messages) = unbounded::<ButtonsMessages>();
    let (tx_message_type, _rx_message_type) = unbounded::<MessageType>();
    let (_tx_client_state, rx_client_state) = unbounded::<(NodeId, ClientState)>();

    let client_ui_state = Arc::new(Mutex::new(UiState::new()));

    // USA SOLO il costruttore new()
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

    // Test: aggiungi un nuovo nodo
    let result = graph_app.add_node(2, NodeType::Client);
    assert!(result.is_ok());

    // Verifica che il nodo sia stato aggiunto
    assert!(graph_app.node_id_to_index.contains_key(&2));

    // Test: aggiungi un edge
    let result = graph_app.add_edge(1, 2);
    assert!(result.is_ok());

    // Test: rimuovi edge
    let result = graph_app.remove_edge(1, 2);
    assert!(result.is_ok());

    // Test: rimuovi nodo
    let result = graph_app.remove_node(2);
    assert!(result.is_ok());

    // Verifica che il nodo sia stato rimosso
    assert!(!graph_app.node_id_to_index.contains_key(&2));
}

#[test]
fn test_button_window_pdr_functionality() {
    // Test della funzionalità PDR che è pubblica
    let (_tx_node_clicked, rx_node_clicked) = unbounded::<NodeId>();
    let (tx_button_messages, _rx_button_messages) = unbounded::<ButtonsMessages>();
    let (tx_button_event, rx_button_event) = unbounded::<ButtonEvent>();

    let mut button_window = ButtonWindow::new(
        rx_node_clicked,
        tx_button_messages,
        tx_button_event,
    );

    // Test: PDR iniziale
    assert_eq!(button_window.selected_pdr, 0.1);

    // Test: modifica PDR
    button_window.selected_pdr = 0.85;
    assert_eq!(button_window.selected_pdr, 0.85);

    // Test: con un nodo selezionato, dovremmo poter inviare eventi
    button_window.node_id1 = Some(1);

    // Simula invio di evento con PDR
    button_window.send_button_event(ButtonEvent::ChangePdr(1, 0.85));

    // Verifica che l'evento sia stato inviato
    match rx_button_event.try_recv() {
        Ok(ButtonEvent::ChangePdr(node_id, pdr)) => {
            assert_eq!(node_id, 1);
            assert_eq!(pdr, 0.85);
        }
        _ => panic!("Evento ChangePdr non ricevuto"),
    }
}