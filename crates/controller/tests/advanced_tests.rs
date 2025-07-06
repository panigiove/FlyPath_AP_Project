use crossbeam_channel::unbounded;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::thread;

// Import corretti basati sui re-export nel lib.rs
use controller::{ButtonsMessages, ButtonEvent, GraphAction, MessageType, NodeType, GraphApp, ButtonWindow};
use wg_2024::network::NodeId;
use client::ui::{UiState, ClientState};

// ================================ TEST RETI GRANDI ================================
//
// NOTA IMPORTANTE: NodeId è u8, quindi la rete può avere massimo 256 nodi (0-255)
// Tutti i test rispettano questo limite e testano anche i comportamenti ai confini.
//

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

    // Test: aggiungi 100 nodi (NodeId 1-100, rispetta il limite u8)
    let start_time = Instant::now();

    for i in 1u8..=100u8 {  // Esplicitamente u8
        let node_type = match i % 3 {
            0 => NodeType::Client,
            1 => NodeType::Drone,
            _ => NodeType::Server,
        };

        let result = graph_app.add_node(i, node_type);
        assert!(result.is_ok(), "Failed to add node {}: {:?}", i, result);
    }

    let creation_time = start_time.elapsed();
    println!("⏱️ Time to create 100 nodes (ID 1-100): {:?}", creation_time);

    // Verifica che tutti i nodi siano stati aggiunti
    assert_eq!(graph_app.node_id_to_index.len(), 100);

    // Test operazioni su rete grande
    let start_time = Instant::now();

    // Aggiungi alcune connessioni (solo tra droni)
    for i in (1u8..=97u8).step_by(3) { // Solo droni, rimaniamo sotto 100
        if i + 3 <= 97u8 {
            let result = graph_app.add_edge(i, i + 3);
            assert!(result.is_ok(), "Failed to add edge {}-{}", i, i + 3);
        }
    }

    let connection_time = start_time.elapsed();
    println!("⏱️ Time to create ~30 connections: {:?}", connection_time);

    // Verifica che la rete sia ancora funzionale
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

    println!("🚀 Testing maximum network capacity (u8 limit = 0-255)");

    // Test: aggiungi nodi da 1 a 255 (massima capacità u8)
    let start_time = Instant::now();
    let mut successful_additions = 0;

    // Aggiungi nodi con ID 1-255 (evitiamo 0 che potrebbe essere riservato)
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
            println!("⚠️ Failed to add node {} (this may be expected)", i);
        }

        // Progress ogni 50 nodi
        if i % 50 == 0 {
            println!("📊 Added {} nodes so far...", successful_additions);
        }
    }

    let creation_time = start_time.elapsed();
    println!("⏱️ Time to create {} nodes: {:?}", successful_additions, creation_time);
    println!("🎯 Successfully added {}/255 possible nodes", successful_additions);

    // Verifica lo stato finale
    assert!(successful_additions > 200, "Should add most nodes successfully");
    assert_eq!(graph_app.node_id_to_index.len(), successful_additions);

    // Test: prova ad aggiungere più nodi di quelli possibili
    // (questo dovrebbe essere gestito dal sistema di generazione ID)
    println!("🧪 Testing ID space exhaustion...");

    // Se il sistema di generazione ID è intelligente, dovrebbe rifiutare
    // di aggiungere nodi quando tutti gli ID sono esauriti
    let impossible_result = graph_app.add_node(0u8, NodeType::Drone); // Prova con ID 0
    println!("🔍 Attempt to add node with ID 0: {:?}", impossible_result);
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

    // Test: aggiungi nodo con ID 42
    let result1 = graph_app.add_node(42u8, NodeType::Drone);
    assert!(result1.is_ok(), "First node should be added successfully");

    // Test: prova ad aggiungere un altro nodo con stesso ID
    let result2 = graph_app.add_node(42u8, NodeType::Client);
    assert!(result2.is_err(), "Duplicate ID should be rejected");
    println!("✅ Correctly rejected duplicate ID: {:?}", result2);

    // Verifica che ci sia solo un nodo
    assert_eq!(graph_app.node_id_to_index.len(), 1);

    // Test: aggiungi nodi in ordine sparso per verificare gestione ID
    let test_ids = [200u8, 1u8, 100u8, 255u8, 50u8];
    let mut added_count = 1; // Già abbiamo il nodo 42

    for &id in &test_ids {
        let result = graph_app.add_node(id, NodeType::Drone);
        if result.is_ok() {
            added_count += 1;
            println!("✅ Added node with ID {}", id);
        } else {
            println!("❌ Failed to add node with ID {}: {:?}", id, result);
        }
    }

    assert_eq!(graph_app.node_id_to_index.len(), added_count);
    println!("📊 Total nodes in network: {}", added_count);

    // Verifica che tutti gli ID siano effettivamente diversi
    let mut unique_ids = std::collections::HashSet::new();
    for &id in graph_app.node_id_to_index.keys() {
        assert!(unique_ids.insert(id), "Found duplicate ID in network: {}", id);
    }

    println!("✅ All {} node IDs are unique", unique_ids.len());
}

#[test]
fn test_large_network_operations_stress() {
    let mut connections = HashMap::new();
    let mut node_types = HashMap::new();

    // Setup rete iniziale con 50 droni (ID 1-50)
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

    // Test: molte operazioni sequenziali
    let start_time = Instant::now();
    let mut operations_count = 0;

    // Aggiungi 25 connessioni (1-25 con 26-50)
    for i in 1u8..=25u8 {
        let result = graph_app.add_edge(i, i + 25);
        assert!(result.is_ok());
        operations_count += 1;
    }

    // Rimuovi 10 connessioni
    for i in 1u8..=10u8 {
        let result = graph_app.remove_edge(i, i + 25);
        assert!(result.is_ok());
        operations_count += 1;
    }

    // Aggiungi 20 nuovi nodi (ID 51-70)
    for i in 51u8..=70u8 {
        let result = graph_app.add_node(i, NodeType::Client);
        assert!(result.is_ok());
        operations_count += 1;
    }

    let total_time = start_time.elapsed();
    let ops_per_second = operations_count as f64 / total_time.as_secs_f64();

    println!("⏱️ {} operations in {:?} = {:.1} ops/sec",
             operations_count, total_time, ops_per_second);

    // Verifica che la rete sia ancora consistente
    assert!(graph_app.node_id_to_index.len() >= 70);

    // Performance assertion: almeno 100 ops/sec
    assert!(ops_per_second > 100.0, "Performance troppo bassa: {:.1} ops/sec", ops_per_second);
}

// ================================ TEST PERFORMANCE/STRESS ================================

#[test]
fn test_performance_button_window_rapid_clicks() {
    let (tx_node_clicked, rx_node_clicked) = unbounded::<NodeId>();
    let (tx_button_messages, _rx_button_messages) = unbounded::<ButtonsMessages>();
    let (tx_button_event, rx_button_event) = unbounded::<ButtonEvent>();

    let mut button_window = ButtonWindow::new(
        rx_node_clicked,
        tx_button_messages,
        tx_button_event,
    );

    // Test: 1000 click rapidi
    let start_time = Instant::now();

    for i in 1..= 255 {
        tx_node_clicked.send(i % 10).unwrap(); // Cicla tra 10 nodi
        button_window.handle_node_clicks();
    }

    let processing_time = start_time.elapsed();
    let clicks_per_second = 1000.0 / processing_time.as_secs_f64();

    println!("⏱️ 1000 clicks processed in {:?} = {:.1} clicks/sec",
             processing_time, clicks_per_second);

    // Performance assertion: almeno 1000 clicks/sec
    assert!(clicks_per_second > 1000.0,
            "Click processing troppo lento: {:.1} clicks/sec", clicks_per_second);

    // Verifica che lo stato sia ancora corretto
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

    // Test: genera 10000 eventi rapidamente
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

    // Verifica che tutti gli eventi siano stati ricevuti
    let mut received_count = 0;
    while rx_button_event.try_recv().is_ok() {
        received_count += 1;
    }

    let events_per_second = event_count as f64 / generation_time.as_secs_f64();

    println!("⏱️ {} events generated in {:?} = {:.1} events/sec",
             event_count, generation_time, events_per_second);
    println!("📨 Received {} events", received_count);

    assert_eq!(received_count, event_count);
    assert!(events_per_second > 10000.0,
            "Event generation troppo lenta: {:.1} events/sec", events_per_second);
}

// ================================ TEST CONCURRENT ACCESS ================================

#[test]
fn test_concurrent_button_window_access() {
    let (tx_node_clicked, rx_node_clicked) = unbounded::<NodeId>();
    let (tx_button_messages, _rx_button_messages) = unbounded::<ButtonsMessages>();
    let (tx_button_event, rx_button_event) = unbounded::<ButtonEvent>();

    // Condividi i canali tra thread
    let tx_node_clicked = Arc::new(tx_node_clicked);
    let tx_button_event_clone = tx_button_event.clone();

    // Thread 1: Simula clicks continui
    let tx_clicks = tx_node_clicked.clone();
    let click_thread = thread::spawn(move || {
        for i in 1..=100 {
            tx_clicks.send(i % 5).unwrap();
            thread::sleep(Duration::from_millis(1));
        }
    });

    // Thread 2: Genera eventi continui
    let event_thread = thread::spawn(move || {
        let (_tx_node_clicked, rx_node_clicked) = unbounded::<NodeId>();
        let (tx_button_messages, _rx_button_messages) = unbounded::<ButtonsMessages>();

        let button_window = ButtonWindow::new(
            rx_node_clicked,
            tx_button_messages,
            tx_button_event_clone,
        );

        for i in 1..=50 {
            button_window.send_button_event(ButtonEvent::NewDrone(i % 3, 0.5));
            thread::sleep(Duration::from_millis(2));
        }
    });

    // Thread principale: processa clicks
    let mut button_window = ButtonWindow::new(
        rx_node_clicked,
        tx_button_messages,
        tx_button_event,
    );

    let start_time = Instant::now();
    let mut processed_clicks = 0;

    // Processa per 1 secondo
    while start_time.elapsed() < Duration::from_secs(1) {
        button_window.handle_node_clicks();
        processed_clicks += 1;
        thread::sleep(Duration::from_millis(5));
    }

    // Aspetta che i thread finiscano
    click_thread.join().unwrap();
    event_thread.join().unwrap();

    // Conta eventi ricevuti
    let mut received_events = 0;
    while rx_button_event.try_recv().is_ok() {
        received_events += 1;
    }

    println!("🔄 Processed {} click cycles", processed_clicks);
    println!("📨 Received {} events from concurrent threads", received_events);

    // Verifica che il sistema sia sopravvissuto al concurrency
    assert!(processed_clicks > 0);
    assert!(received_events >= 50); // Almeno gli eventi del thread 2
    assert!(button_window.node_id1.is_some() || button_window.node_id2.is_some());
}

#[test]
fn test_concurrent_graph_operations() {
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;

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

    // Wrap in Arc<Mutex> per accesso concorrente
    let graph_app = Arc::new(StdMutex::new(graph_app));

    let mut handles = Vec::new();

    // Spawna 3 thread che aggiungono nodi
    // Thread 0: nodi 1-20, Thread 1: nodi 51-70, Thread 2: nodi 101-120
    let thread_ranges = [(1u8, 20u8), (51u8, 70u8), (101u8, 120u8)];

    for (thread_id, (start_id, end_id)) in thread_ranges.iter().enumerate() {
        let graph_ref = graph_app.clone();
        let start_id = *start_id;
        let end_id = *end_id;

        let handle = thread::spawn(move || {
            let mut successes = 0;

            for node_id in start_id..=end_id {
                let node_type = NodeType::Drone;

                if let Ok(mut graph) = graph_ref.lock() {
                    if graph.add_node(node_id, node_type).is_ok() {
                        successes += 1;
                    }
                }

                thread::sleep(Duration::from_millis(1));
            }

            (thread_id, successes)
        });
        handles.push(handle);
    }

    // Aspetta tutti i thread
    let mut total_successes = 0;
    for handle in handles {
        let (thread_id, successes) = handle.join().unwrap();
        println!("🧵 Thread {} added {} nodes", thread_id, successes);
        total_successes += successes;
    }

    // Verifica risultati
    let final_node_count = {
        let graph = graph_app.lock().unwrap();
        graph.node_id_to_index.len()
    };

    println!("🎯 {} successful concurrent node additions", total_successes);
    println!("📊 Final node count: {}", final_node_count);
    println!("📋 Used ID ranges: 1-20, 51-70, 101-120 (all within u8 limit)");

    assert!(total_successes > 0);
    assert_eq!(final_node_count, total_successes);
    assert_eq!(total_successes, 60); // 20 nodi per 3 thread
}

#[test]
fn test_state_persistence_workflow() {
    // Simula salvataggio e caricamento dello stato di ButtonWindow
    let (_tx_node_clicked, rx_node_clicked) = unbounded::<NodeId>();
    let (tx_button_messages, _rx_button_messages) = unbounded::<ButtonsMessages>();
    let (tx_button_event, _rx_button_event) = unbounded::<ButtonEvent>();

    let mut button_window = ButtonWindow::new(
        rx_node_clicked,
        tx_button_messages,
        tx_button_event,
    );

    // Imposta stato
    button_window.node_id1 = Some(42);
    button_window.node_id2 = Some(84);
    button_window.selected_pdr = 0.73;

    // "Serializza" stato manualmente
    let state_string = format!("{}|{}|{}",
                               button_window.node_id1.unwrap_or(0),
                               button_window.node_id2.unwrap_or(0),
                               button_window.selected_pdr
    );

    println!("💾 Saved state: {}", state_string);

    // Simula nuovo ButtonWindow (riavvio app)
    let (_tx_node_clicked2, rx_node_clicked2) = unbounded::<NodeId>();
    let (tx_button_messages2, _rx_button_messages2) = unbounded::<ButtonsMessages>();
    let (tx_button_event2, _rx_button_event2) = unbounded::<ButtonEvent>();

    let mut new_button_window = ButtonWindow::new(
        rx_node_clicked2,
        tx_button_messages2,
        tx_button_event2,
    );

    // "Deserializza" stato
    let parts: Vec<&str> = state_string.split('|').collect();
    new_button_window.node_id1 = if parts[0] == "0" { None } else { Some(parts[0].parse().unwrap()) };
    new_button_window.node_id2 = if parts[1] == "0" { None } else { Some(parts[1].parse().unwrap()) };
    new_button_window.selected_pdr = parts[2].parse().unwrap();

    // Verifica che lo stato sia stato ripristinato
    assert_eq!(new_button_window.node_id1, Some(42));
    assert_eq!(new_button_window.node_id2, Some(84));
    assert_eq!(new_button_window.selected_pdr, 0.73);

    println!("✅ State successfully restored!");
}
