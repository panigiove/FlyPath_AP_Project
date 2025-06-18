use std::collections::HashMap;
use crossbeam_channel::unbounded;
use eframe::egui;

// Import dalla tua struttura
use controller::view::graph::GraphApp;  // Il tuo GraphApp
use controller::utility::{GraphAction, ButtonsMessages, MessageType, NodeType}; // Le tue utility
use wg_2024::network::NodeId;

fn main() -> Result<(), eframe::Error> {
    // Abilita i log per debugging (opzionale)
    env_logger::init();

    // 1. Crea tutti i canali richiesti da GraphApp::new()
    let (_graph_action_sender, graph_action_receiver) = unbounded::<GraphAction>();
    let (node_clicked_sender, node_clicked_receiver) = unbounded::<NodeId>();
    let (edge_clicked_sender, edge_clicked_receiver) = unbounded::<(NodeId, NodeId)>();
    let (_button_messages_sender, button_messages_receiver) = unbounded::<ButtonsMessages>();
    let (button_messages_sender2, _button_messages_receiver2) = unbounded::<ButtonsMessages>();
    let (message_type_sender, message_type_receiver) = unbounded::<MessageType>();

    // 2. Crea dati iniziali per testare il grafo
    let mut connections: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    let mut node_types: HashMap<NodeId, NodeType> = HashMap::new();

    // Aggiungi nodi di esempio
    node_types.insert(1, NodeType::Client);
    node_types.insert(2, NodeType::Drone);
    node_types.insert(3, NodeType::Drone);
    node_types.insert(4, NodeType::Server);

    // Aggiungi connessioni di esempio
    connections.insert(1, vec![2]);        // Client 1 -> Drone 2
    connections.insert(2, vec![1, 3, 4]);  // Drone 2 -> Client 1, Drone 3, Server 4
    connections.insert(3, vec![2, 4]);     // Drone 3 -> Drone 2, Server 4
    connections.insert(4, vec![2, 3]);     // Server 4 -> Drone 2, Drone 3

    // 3. Stampa le interazioni (thread separati per gestire i canali)
    std::thread::spawn(move || {
        while let Ok(node_id) = node_clicked_receiver.recv() {
            println!("🖱️ Nodo cliccato: {}", node_id);
        }
    });

    std::thread::spawn(move || {
        while let Ok((id1, id2)) = edge_clicked_receiver.recv() {
            println!("🔗 Edge cliccato: {} <-> {}", id1, id2);
        }
    });

    std::thread::spawn(move || {
        while let Ok(message) = message_type_receiver.recv() {
            match message {
                MessageType::Ok(msg) => println!("✅ {}", msg),
                MessageType::PacketSent(msg) => println!("📤 {}", msg),
                MessageType::Error(msg) => println!("❌ {}", msg),
            }
        }
    });

    // 4. Configura la finestra
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("Network Graph Controller"),
        ..Default::default()
    };

    // 5. Avvia l'applicazione
    eframe::run_native(
        "Network Graph Controller",
        options,
        Box::new(move |cc| {
            // Crea GraphApp con tutti i parametri dalla tua implementazione
            Ok(Box::new(GraphApp::new(
                cc,                         // CreationContext per le texture
                connections,                // HashMap delle connessioni
                node_types,                 // HashMap dei tipi di nodi
                graph_action_receiver,      // Receiver per GraphAction
                node_clicked_sender,        // Sender per click sui nodi
                edge_clicked_sender,        // Sender per click sugli edge
                button_messages_receiver,   // Receiver per ButtonsMessages
                button_messages_sender2,    // Sender per ButtonsMessages
                message_type_sender,        // Sender per MessageType
            )))
        }),
    )
}

// Funzione helper per testare l'aggiunta di nodi/edge via canali
#[allow(dead_code)]
fn send_test_commands(sender: &crossbeam_channel::Sender<GraphAction>) {
    use std::time::Duration;

    // Aspetta un po' prima di inviare comandi
    std::thread::sleep(Duration::from_secs(2));

    // Aggiungi un nuovo nodo
    let _ = sender.send(GraphAction::AddNode(5, NodeType::Client));

    std::thread::sleep(Duration::from_secs(1));

    // Connetti il nuovo nodo
    let _ = sender.send(GraphAction::AddEdge(5, 2));

    std::thread::sleep(Duration::from_secs(2));

    // Rimuovi una connessione esistente
    let _ = sender.send(GraphAction::RemoveEdge(1, 2));
}