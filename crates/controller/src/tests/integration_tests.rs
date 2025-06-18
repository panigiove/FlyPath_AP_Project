use crossbeam_channel::{unbounded, select};
use std::collections::HashMap;
use std::thread;
use std::time::Duration;

// Importa i tuoi moduli
use controller::utility::{ButtonsMessages, ButtonEvent, GraphAction, MessageType, NodeType};
use controller::graph_app::GraphApp;
use controller::button_window::ButtonWindow;
use wg_2024::network::NodeId;

#[test]
fn test_sync_between_components() {
    // Crea i canali
    let (tx_graph_action, rx_graph_action) = unbounded::<GraphAction>();
    let (tx_node_clicked, rx_node_clicked) = unbounded::<NodeId>();
    let (tx_edge_clicked, rx_edge_clicked) = unbounded::<(NodeId, NodeId)>();
    let (tx_button_msg_to_graph, rx_button_msg_to_graph) = unbounded::<ButtonsMessages>();
    let (tx_button_msg_to_button, rx_button_msg_to_button) = unbounded::<ButtonsMessages>();
    let (tx_msg_type, rx_msg_type) = unbounded::<MessageType>();
    let (tx_button_event, rx_button_event) = unbounded::<ButtonEvent>();

    // Crea GraphApp
    let mut graph_app = GraphApp {
        selected_node_id1: None,
        selected_node_id2: None,
        selected_edge: None,
        node_textures: HashMap::new(),
        connection: HashMap::new(),
        node_types: HashMap::new(),
        receiver_updates: rx_graph_action,
        sender_node_clicked: tx_node_clicked,
        sender_edge_clicked: tx_edge_clicked,
        reciver_buttom_messages: rx_button_msg_to_graph,
        sender_buttom_messages: tx_button_msg_to_button.clone(),
        sender_message_type: tx_msg_type,
    };

    // Crea ButtonWindow
    let mut button_window = ButtonWindow {
        node_id1: None,
        node_id2: None,
        is_multiple_selection_allowed: false,
        pdr_change: None,
        current_node_clicked: None,
        current_pdr: None,
        show_pdr_input: false,
        show_change_pdr_input: false,
        input_pdr: String::new(),
        selection_timestamp: None,
        reciver_node_clicked: rx_node_clicked,
        reciver_edge_clicked: rx_edge_clicked,
        sender_button_event: tx_button_event,
        sender_buttom_messages: tx_button_msg_to_graph.clone(),
        reciver_buttom_messages: rx_button_msg_to_button,
    };

    // Test 1: Click in GraphApp si propaga a ButtonWindow
    graph_app.handle_node_click(10);

    // ButtonWindow riceve l'aggiornamento
    if let Ok(msg) = button_window.reciver_buttom_messages.try_recv() {
        button_window.handle_graph_message(msg);
    }

    // Verifica sincronizzazione
    assert_eq!(graph_app.selected_node_id1, Some(10));
    assert_eq!(button_window.node_id1, Some(10));

    // Test 2: Click in ButtonWindow si propaga a GraphApp
    button_window.node_clicked_handler(20);

    // GraphApp riceve l'aggiornamento
    if let Ok(msg) = graph_app.reciver_buttom_messages.try_recv() {
        graph_app.button_messages_handler(msg);
    }

    // Verifica sincronizzazione
    assert_eq!(button_window.node_id1, Some(20));
    assert_eq!(graph_app.selected_node_id1, Some(20));
}

#[test]
fn test_complete_workflow() {
    // Setup canali
    let (tx_button_event, rx_button_event) = unbounded::<ButtonEvent>();
    let (tx_graph_action, rx_graph_action) = unbounded::<GraphAction>();

    // Simula workflow completo

    // 1. Utente seleziona un nodo
    let selected_node = 5u8;

    // 2. Utente crea un nuovo drone
    tx_button_event.send(ButtonEvent::NewDrone(selected_node, 0.85)).unwrap();

    // 3. Controller riceve l'evento
    if let Ok(event) = rx_button_event.recv_timeout(Duration::from_millis(100)) {
        match event {
            ButtonEvent::NewDrone(node_id, pdr) => {
                assert_eq!(node_id, 5);
                assert_eq!(pdr, 0.85);

                // 4. Controller crea il nuovo nodo
                let new_drone_id = 100u8;
                tx_graph_action.send(GraphAction::AddNode(new_drone_id, NodeType::Drone)).unwrap();
                tx_graph_action.send(GraphAction::AddEdge(node_id, new_drone_id)).unwrap();
            }
            _ => panic!("Evento non previsto"),
        }
    }

    // 5. Verifica che le azioni siano state inviate
    let mut actions_received = 0;
    while let Ok(action) = rx_graph_action.recv_timeout(Duration::from_millis(100)) {
        match action {
            GraphAction::AddNode(id, node_type) => {
                assert_eq!(id, 100);
                assert_eq!(node_type, NodeType::Drone);
                actions_received += 1;
            }
            GraphAction::AddEdge(id1, id2) => {
                assert_eq!(id1, 5);
                assert_eq!(id2, 100);
                actions_received += 1;
            }
            _ => {}
        }
    }

    assert_eq!(actions_received, 2);
}

