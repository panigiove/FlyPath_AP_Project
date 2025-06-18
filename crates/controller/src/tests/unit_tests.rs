use crossbeam_channel::{unbounded, Receiver, Sender};
use std::collections::HashMap;
use std::time::{Duration, Instant};

// Importa i tuoi moduli (adatta i path secondo la tua struttura)
use tuo_progetto::utility::{ButtonsMessages, ButtonEvent, GraphAction, MessageType, NodeType};
use tuo_progetto::graph_app::GraphApp;
use tuo_progetto::button_window::ButtonWindow;
use wg_2024::network::NodeId;

// Helper function per creare tutti i canali necessari
fn create_test_channels() -> (
    Sender<GraphAction>, Receiver<GraphAction>,
    Sender<NodeId>, Receiver<NodeId>,
    Sender<(NodeId, NodeId)>, Receiver<(NodeId, NodeId)>,
    Sender<ButtonsMessages>, Receiver<ButtonsMessages>,
    Sender<ButtonsMessages>, Receiver<ButtonsMessages>,
    Sender<MessageType>, Receiver<MessageType>,
    Sender<ButtonEvent>, Receiver<ButtonEvent>
) {
    let (tx_graph_action, rx_graph_action) = unbounded();
    let (tx_node_clicked, rx_node_clicked) = unbounded();
    let (tx_edge_clicked, rx_edge_clicked) = unbounded();
    let (tx_button_msg_to_graph, rx_button_msg_to_graph) = unbounded();
    let (tx_button_msg_to_button, rx_button_msg_to_button) = unbounded();
    let (tx_msg_type, rx_msg_type) = unbounded();
    let (tx_button_event, rx_button_event) = unbounded();

    (tx_graph_action, rx_graph_action,
     tx_node_clicked, rx_node_clicked,
     tx_edge_clicked, rx_edge_clicked,
     tx_button_msg_to_graph, rx_button_msg_to_graph,
     tx_button_msg_to_button, rx_button_msg_to_button,
     tx_msg_type, rx_msg_type,
     tx_button_event, rx_button_event)
}

#[test]
fn test_graph_app_node_selection() {
    // Setup
    let (_tx_graph_action, rx_graph_action,
        tx_node_clicked, _rx_node_clicked,
        tx_edge_clicked, _rx_edge_clicked,
        _tx_button_msg_to_graph, rx_button_msg_to_graph,
        tx_button_msg_to_button, rx_button_msg_to_button,
        tx_msg_type, _rx_msg_type,
        _tx_button_event, _rx_button_event) = create_test_channels();

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
        sender_buttom_messages: tx_button_msg_to_button,
        sender_message_type: tx_msg_type,
    };

    // Test: seleziona un nodo
    graph_app.handle_node_click(1);

    // Verifica che il nodo sia selezionato
    assert_eq!(graph_app.selected_node_id1, Some(1));
    assert_eq!(graph_app.selected_node_id2, None);

    // Verifica che sia stato inviato il messaggio di sincronizzazione
    match rx_button_msg_to_button.try_recv() {
        Ok(ButtonsMessages::UpdateSelection(node1, node2)) => {
            assert_eq!(node1, Some(1));
            assert_eq!(node2, None);
        }
        _ => panic!("Messaggio di sincronizzazione non ricevuto"),
    }
}

#[test]
fn test_button_window_node_selection() {
    // Setup
    let (_tx_graph_action, _rx_graph_action,
        _tx_node_clicked, rx_node_clicked,
        _tx_edge_clicked, rx_edge_clicked,
        tx_button_msg_to_graph, _rx_button_msg_to_graph,
        _tx_button_msg_to_button, rx_button_msg_to_button,
        _tx_msg_type, _rx_msg_type,
        tx_button_event, _rx_button_event) = create_test_channels();

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
        sender_buttom_messages: tx_button_msg_to_graph,
        reciver_buttom_messages: rx_button_msg_to_button,
    };

    // Test: gestione click su nodo
    button_window.node_clicked_handler(5);

    // Verifica stato interno
    assert_eq!(button_window.node_id1, Some(5));
    assert_eq!(button_window.node_id2, None);
    assert!(button_window.selection_timestamp.is_some());
}

#[test]
fn test_clear_selection() {
    // Setup
    let (_tx_graph_action, _rx_graph_action,
        _tx_node_clicked, rx_node_clicked,
        _tx_edge_clicked, rx_edge_clicked,
        tx_button_msg_to_graph, rx_button_msg_to_graph,
        _tx_button_msg_to_button, rx_button_msg_to_button,
        _tx_msg_type, _rx_msg_type,
        tx_button_event, _rx_button_event) = create_test_channels();

    let mut button_window = ButtonWindow {
        node_id1: Some(1),
        node_id2: Some(2),
        is_multiple_selection_allowed: false,
        pdr_change: None,
        current_node_clicked: None,
        current_pdr: None,
        show_pdr_input: false,
        show_change_pdr_input: false,
        input_pdr: String::new(),
        selection_timestamp: Some(Instant::now()),
        reciver_node_clicked: rx_node_clicked,
        reciver_edge_clicked: rx_edge_clicked,
        sender_button_event: tx_button_event,
        sender_buttom_messages: tx_button_msg_to_graph,
        reciver_buttom_messages: rx_button_msg_to_button,
    };

    // Test: pulisci selezione
    button_window.clear_selection();

    // Verifica che tutto sia stato pulito
    assert_eq!(button_window.node_id1, None);
    assert_eq!(button_window.node_id2, None);
    assert!(button_window.selection_timestamp.is_none());

    // Verifica che sia stato inviato il messaggio
    match rx_button_msg_to_graph.try_recv() {
        Ok(ButtonsMessages::ClearAllSelections) => {}
        _ => panic!("Messaggio ClearAllSelections non ricevuto"),
    }
}

#[test]
fn test_pdr_operations() {
    // Setup
    let (_tx_graph_action, _rx_graph_action,
        _tx_node_clicked, rx_node_clicked,
        _tx_edge_clicked, rx_edge_clicked,
        tx_button_msg_to_graph, _rx_button_msg_to_graph,
        _tx_button_msg_to_button, rx_button_msg_to_button,
        _tx_msg_type, _rx_msg_type,
        tx_button_event, rx_button_event) = create_test_channels();

    let mut button_window = ButtonWindow {
        node_id1: Some(1),
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
        sender_buttom_messages: tx_button_msg_to_graph,
        reciver_buttom_messages: rx_button_msg_to_button,
    };

    // Test: crea drone con PDR
    button_window.create_drone_with_pdr(0.75);

    // Verifica che l'evento sia stato inviato
    match rx_button_event.try_recv() {
        Ok(ButtonEvent::NewDrone(node_id, pdr)) => {
            assert_eq!(node_id, 1);
            assert_eq!(pdr, 0.75);
        }
        _ => panic!("Evento NewDrone non ricevuto"),
    }

    // Test: cambia PDR
    button_window.change_node_pdr(0.9);

    match rx_button_event.try_recv() {
        Ok(ButtonEvent::ChangePdr(node_id, pdr)) => {
            assert_eq!(node_id, 1);
            assert_eq!(pdr, 0.9);
        }
        _ => panic!("Evento ChangePdr non ricevuto"),
    }
}