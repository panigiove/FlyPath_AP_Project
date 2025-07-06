use crossbeam_channel::unbounded;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Import corretti basati sui re-export nel lib.rs
use controller::{ButtonsMessages, ButtonEvent, GraphAction, MessageType, NodeType, GraphApp, ButtonWindow};
use wg_2024::network::NodeId;
use client::ui::{UiState, ClientState};

#[test]
fn test_complete_sync_between_components() {
    // Setup dati iniziali
    let mut connections = HashMap::new();
    let mut node_types = HashMap::new();

    // Aggiungi alcuni nodi di test
    node_types.insert(1, NodeType::Drone);
    node_types.insert(2, NodeType::Client);
    connections.insert(1, vec![2]);
    connections.insert(2, vec![1]);

    // Setup canali condivisi per simulare il vero sistema
    let (tx_graph_action, rx_graph_action) = unbounded::<GraphAction>();
    let (tx_node_clicked, rx_node_clicked) = unbounded::<NodeId>();
    let (tx_button_messages, rx_button_messages) = unbounded::<ButtonsMessages>();
    let (tx_message_type, rx_message_type) = unbounded::<MessageType>();
    let (tx_button_event, rx_button_event) = unbounded::<ButtonEvent>();
    let (tx_client_state, rx_client_state) = unbounded::<(NodeId, ClientState)>();

    let client_ui_state = Arc::new(Mutex::new(UiState::new()));

    // Crea componenti usando i veri costruttori
    let mut graph_app = GraphApp::new(
        connections,
        node_types,
        rx_graph_action,
        tx_node_clicked.clone(),
        rx_button_messages,
        tx_message_type,
        client_ui_state,
        rx_client_state,
    );

    let mut button_window = ButtonWindow::new(
        rx_node_clicked,
        tx_button_messages.clone(),
        tx_button_event.clone(),
    );

    // Test 1: Simula click su nodo in GraphApp
    tx_node_clicked.send(1).unwrap();

    // ButtonWindow dovrebbe ricevere e processare il click
    button_window.handle_node_clicks();
    assert_eq!(button_window.node_id1, Some(1));

    // ButtonWindow invia aggiornamento di selezione
    button_window.update_graph_selection();

    // GraphApp riceve e processa l'aggiornamento
    graph_app.handle_pending_events();

    // Test 2: Simula secondo click per selezione multipla
    tx_node_clicked.send(2).unwrap();
    button_window.handle_node_clicks();

    assert_eq!(button_window.node_id1, Some(1));
    assert_eq!(button_window.node_id2, Some(2));

    // Test 3: Verifica che ButtonWindow possa generare eventi
    button_window.send_button_event(ButtonEvent::NewDrone(1, 0.85));

    // Controller dovrebbe ricevere l'evento
    match rx_button_event.try_recv() {
        Ok(ButtonEvent::NewDrone(node_id, pdr)) => {
            assert_eq!(node_id, 1);
            assert_eq!(pdr, 0.85);
        }
        _ => panic!("Evento non ricevuto dal controller"),
    }

    // Test 4: Simula aggiunta di nodo dal controller
    tx_graph_action.send(GraphAction::AddNode(3, NodeType::Server)).unwrap();
    graph_app.handle_pending_events();

    // Verifica che il nodo sia stato aggiunto
    assert!(graph_app.node_id_to_index.contains_key(&3));
}

#[test]
fn test_complete_workflow_node_creation() {
    // Setup base system
    let connections = HashMap::new();
    let node_types = HashMap::new();

    let (_tx_graph_action, rx_graph_action) = unbounded::<GraphAction>();
    let (tx_node_clicked, rx_node_clicked) = unbounded::<NodeId>();
    let (tx_button_messages, rx_button_messages) = unbounded::<ButtonsMessages>();
    let (tx_message_type, _rx_message_type) = unbounded::<MessageType>();
    let (tx_button_event, rx_button_event) = unbounded::<ButtonEvent>();
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

    let mut button_window = ButtonWindow::new(
        rx_node_clicked,
        tx_button_messages,
        tx_button_event,
    );

    // Workflow completo: creazione di un nuovo drone

    // 1. Aggiungi un nodo base per connessione
    let result = graph_app.add_node(1, NodeType::Drone);
    assert!(result.is_ok());

    // 2. Seleziona il nodo
    button_window.node_id1 = Some(1);

    // 3. Imposta PDR
    button_window.selected_pdr = 0.75;

    // 4. Genera evento per nuovo drone
    button_window.send_button_event(ButtonEvent::NewDrone(1, 0.75));

    // 5. Verifica che l'evento sia stato generato correttamente
    match rx_button_event.recv_timeout(Duration::from_millis(100)) {
        Ok(ButtonEvent::NewDrone(connection_id, pdr)) => {
            assert_eq!(connection_id, 1);
            assert_eq!(pdr, 0.75);
        }
        _ => panic!("Evento NewDrone non ricevuto nel tempo previsto"),
    }
}

#[test]
fn test_edge_management_workflow() {
    // Setup con due nodi per testare le connessioni
    let mut connections = HashMap::new();
    let mut node_types = HashMap::new();

    node_types.insert(1, NodeType::Drone);
    node_types.insert(2, NodeType::Drone);
    connections.insert(1, Vec::new());
    connections.insert(2, Vec::new());

    let (_tx_graph_action, rx_graph_action) = unbounded::<GraphAction>();
    let (tx_node_clicked, rx_node_clicked) = unbounded::<NodeId>();
    let (tx_button_messages, rx_button_messages) = unbounded::<ButtonsMessages>();
    let (tx_message_type, _rx_message_type) = unbounded::<MessageType>();
    let (tx_button_event, rx_button_event) = unbounded::<ButtonEvent>();
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

    let mut button_window = ButtonWindow::new(
        rx_node_clicked,
        tx_button_messages,
        tx_button_event,
    );

    // Test aggiunta di edge tra due nodi
    let result = graph_app.add_edge(1, 2);
    assert!(result.is_ok());

    // Seleziona entrambi i nodi
    button_window.node_id1 = Some(1);
    button_window.node_id2 = Some(2);

    // Genera evento per rimuovere connessione
    button_window.send_button_event(ButtonEvent::RemoveConection(1, 2));

    // Verifica evento
    match rx_button_event.try_recv() {
        Ok(ButtonEvent::RemoveConection(id1, id2)) => {
            assert_eq!(id1, 1);
            assert_eq!(id2, 2);
        }
        _ => panic!("Evento RemoveConnection non ricevuto"),
    }

    // Test rimozione edge
    let result = graph_app.remove_edge(1, 2);
    assert!(result.is_ok());
}

#[test]
fn test_message_flow_and_error_handling() {
    // Test del flusso dei messaggi e gestione errori
    let connections = HashMap::new();
    let node_types = HashMap::new();

    let (_tx_graph_action, rx_graph_action) = unbounded::<GraphAction>();
    let (tx_node_clicked, rx_node_clicked) = unbounded::<NodeId>();
    let (tx_button_messages, rx_button_messages) = unbounded::<ButtonsMessages>();
    let (tx_message_type, rx_message_type) = unbounded::<MessageType>();
    let (tx_button_event, _rx_button_event) = unbounded::<ButtonEvent>();
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

    let mut button_window = ButtonWindow::new(
        rx_node_clicked,
        tx_button_messages,
        tx_button_event,
    );

    // Test operazione non valida (rimuovere nodo inesistente)
    let result = graph_app.remove_node(255);
    assert!(result.is_err());

    // Test clear selection
    button_window.clear_selection();

    // GraphApp dovrebbe processare il clear
    graph_app.handle_pending_events();

    // Verifica che le selezioni siano state pulite
    assert_eq!(graph_app.selected_nodes.len(), 0);
    assert_eq!(graph_app.selected_edge, None);
}