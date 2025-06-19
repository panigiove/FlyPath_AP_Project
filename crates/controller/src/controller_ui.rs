use std::collections::HashMap;
use crossbeam_channel::{unbounded, Receiver, Sender};
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::drone::Drone;
use wg_2024::network::NodeId;
use wg_2024::packet::Packet;
use client::ui::UiState;
use client::worker::Worker;
use message::{NodeCommand, NodeEvent};
use server::ChatServer;
use crate::controller_handler::ControllerHandler;
use crate::{ButtonEvent, DroneGroup, GraphApp};
use crate::utility::{ButtonsMessages, GraphAction, MessageType, NodeType};
use crate::view::buttons::ButtonWindow;
use crate::view::messages_view::MessagesWindow;

pub struct ControllerUi {
    pub graph_app: GraphApp,
    pub messages_window: MessagesWindow,
    pub button_window: ButtonWindow,
}

impl ControllerUi {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        
        reciver_node_clicked: Receiver<NodeId>,
        sender_buttom_messages: Sender<ButtonsMessages>,
        button_event_sender: Sender<ButtonEvent>,
        
        connection: HashMap<NodeId, Vec<NodeId>>,
        node_types: HashMap<NodeId, NodeType>,
        receiver_updates: Receiver<GraphAction>,
        sender_node_clicked: Sender<NodeId>,
        sender_edge_clicked: Sender<(NodeId, NodeId)>,
        reciver_buttom_messages_graph: Receiver<ButtonsMessages>,
        sender_message_type: Sender<MessageType>,
        
        messages_receiver: Receiver<MessageType>,
    ) -> Self {
        let button_window = ButtonWindow::new(
            reciver_node_clicked,
            sender_buttom_messages,
            button_event_sender,
        );

        let graph_app = GraphApp::new(
            cc,
            connection,
            node_types,
            receiver_updates,
            sender_node_clicked,
            sender_edge_clicked,
            reciver_buttom_messages_graph,
            sender_message_type,
        );

        let messages_window = MessagesWindow::new(messages_receiver);

        Self {
            button_window,
            graph_app,
            messages_window,
        }
    }

    pub fn handle_background_updates(&mut self) {
        // Gestisce gli aggiornamenti del grafo
        if let Ok(command) = self.graph_app.receiver_updates.try_recv() {
            self.graph_app.graph_action_handler(command);
        }

        // Gestisce i messaggi dai pulsanti per il grafo
        if let Ok(command) = self.graph_app.reciver_buttom_messages.try_recv() {
            self.graph_app.button_messages_handler(command);
        }

        // Gestisce i messaggi in arrivo per la finestra dei messaggi
        self.messages_window.handle_incoming_messages();
    }
}


impl eframe::App for ControllerUI {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request continuous updates
        ctx.request_repaint();

        // Layout: Top area split into left (graph) and right (buttons), bottom for messages

        // Bottom panel for messages (fixed height)
        egui::TopBottomPanel::bottom("messages_panel")
            .exact_height(150.0)
            .show(ctx, |ui| {
                // Messages are handled by MessagesWindow in its own thread
                // This is just a placeholder since MessagesWindow handles its own rendering
                ui.heading("Messages");
                ui.separator();
                ui.label("Messages appear here...");
            });

        // Right panel for buttons (fixed width)
        egui::SidePanel::right("buttons_panel")
            .exact_width(300.0)
            .resizable(false)
            .show(ctx, |ui| {
                // Button window update
                self.button_window.update(ctx);
            });

        // Central panel for graph (takes remaining space)
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Network Graph");
            ui.separator();
            // Graph is handled by GraphApp in its own thread
            ui.label("Graph visualization appears here...");
        });
    }
}

// Alternative: Single-threaded implementation
impl eframe::App for ControllerUi {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
            // Gestisce aggiornamenti in background
            self.handle_background_updates();

            // Layout principale usando TopBottomPanel per i messaggi in basso
            egui::TopBottomPanel::bottom("messages_panel")
                .exact_height(200.0)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.add_space(5.0);
                    ui.heading("📝 Messages");
                    ui.separator();

                    // Mostra la finestra dei messaggi
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .max_height(150.0)
                        .show(ui, |ui| {
                            for message in &self.messages_window.log {
                                match message {
                                    MessageType::Error(t) => {
                                        let text = egui::RichText::new(t)
                                            .color(egui::Color32::from_rgb(234, 162, 124));
                                        ui.label(text);
                                    }
                                    MessageType::Ok(t) => {
                                        let text = egui::RichText::new(t)
                                            .color(egui::Color32::from_rgb(232, 187, 166));
                                        ui.label(text);
                                    }
                                    MessageType::PacketSent(t) => {
                                        let text = egui::RichText::new(t)
                                            .color(egui::Color32::from_rgb(14, 137, 145));
                                        ui.label(text);
                                    }
                                    MessageType::PacketDropped(t) => {
                                        let text = egui::RichText::new(t)
                                            .color(egui::Color32::from_rgb(12, 49, 59));
                                        ui.label(text);
                                    }
                                    MessageType::Info(t) => {
                                        let text = egui::RichText::new(t)
                                            .color(egui::Color32::from_rgb(141, 182, 188));
                                        ui.label(text);
                                    }
                                    _ => {
                                        let text = egui::RichText::new("Messaggio non classificato")
                                            .color(egui::Color32::GRAY);
                                        ui.label(text);
                                    }
                                }
                            }

                            // Auto-scroll verso il basso
                            if !self.messages_window.log.is_empty() {
                                ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                            }
                        });
                });

            // Area centrale con pannelli laterali
            egui::CentralPanel::default().show(ctx, |ui| {
                // Pannello destro per i pulsanti
                egui::SidePanel::right("buttons_panel")
                    .exact_width(320.0)
                    .resizable(false)
                    .show_inside(ui, |ui| {
                        ui.add_space(5.0);
                        ui.heading("🎛️ Network Controls");
                        ui.separator();

                        // Gestisce i click sui nodi per ButtonWindow
                        self.button_window.handle_node_clicks();

                        // Mostra le informazioni di selezione
                        self.show_selection_info(ui);
                        ui.separator();

                        // Operazioni sui nodi
                        self.show_node_operations(ui);
                        ui.separator();

                        // Operazioni di connessione
                        self.show_connection_operations(ui);
                        ui.separator();

                        // Operazioni di creazione
                        self.show_creation_operations(ui);
                    });

                // Pannello centrale per il grafo
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    ui.add_space(5.0);
                    ui.heading("🌐 Network Graph");
                    ui.separator();

                    // Pannello informativo laterale per il grafo
                    egui::SidePanel::left("graph_info_panel")
                        .exact_width(220.0)
                        .resizable(false)
                        .show_inside(ui, |ui| {
                            ui.heading("📊 Graph Info");
                            ui.separator();

                            ui.label(format!("Nodes: {}", self.graph_app.node_types.len()));
                            ui.label(format!("Edges: {}",
                                             self.graph_app.connection.values().map(|v| v.len()).sum::<usize>() / 2));

                            ui.separator();

                            // Informazioni nodo selezionato
                            if let Some(selected_node_id) = self.graph_app.selected_node_id1 {
                                if let Some(&node_type) = self.graph_app.node_types.get(&selected_node_id) {
                                    ui.heading("🔵 Selected Node");
                                    ui.label(format!("ID: {}", selected_node_id));
                                    ui.label(format!("Type: {:?}", node_type));

                                    if let Some(connections) = self.graph_app.connection.get(&selected_node_id) {
                                        ui.label(format!("Connected to: {:?}", connections));
                                    }

                                    if ui.button("Deselect Node").clicked() {
                                        self.graph_app.selected_node_id1 = None;
                                        self.graph_app.selected_node_id2 = None;
                                    }
                                }
                            }

                            // Informazioni edge selezionato
                            if let Some((id1, id2)) = self.graph_app.selected_edge {
                                ui.separator();
                                ui.heading("🔗 Selected Edge");
                                ui.label(format!("Connection: {} ↔ {}", id1, id2));

                                if let (Some(&type1), Some(&type2)) = (
                                    self.graph_app.node_types.get(&id1),
                                    self.graph_app.node_types.get(&id2)
                                ) {
                                    ui.label(format!("Types: {:?} ↔ {:?}", type1, type2));
                                }

                                if ui.button("Deselect Edge").clicked() {
                                    self.graph_app.selected_edge = None;
                                }
                            }

                            if self.graph_app.selected_node_id1.is_none() && self.graph_app.selected_edge.is_none() {
                                ui.label("No selection");
                                ui.label("Click on nodes or edges to select them");
                            }

                            ui.separator();

                            // Statistiche sui tipi di nodi
                            if !self.graph_app.node_types.is_empty() {
                                ui.label("Node types:");
                                for node_type in [NodeType::Client, NodeType::Drone, NodeType::Server] {
                                    let count = self.graph_app.node_types.values()
                                        .filter(|&&nt| nt == node_type).count();
                                    if count > 0 {
                                        ui.label(format!("{:?}: {}", node_type, count));
                                    }
                                }
                            }
                        });

                    // Area principale per il grafo
                    egui::CentralPanel::default().show_inside(ui, |ui| {
                        self.graph_app.draw_custom_graph(ui);
                    });
                });
            });
        }
}

// Helper function to create and run the app
pub fn run_controller(
    drones: HashMap<NodeId, Box<dyn Drone>>,
    drones_type: HashMap<NodeId, DroneGroup>,
    drone_senders: HashMap<NodeId, Sender<Packet>>,
    clients: HashMap<NodeId, Worker>,
    servers: HashMap<NodeId, ChatServer>,
    connections: HashMap<NodeId, Vec<NodeId>>,
    send_command_drone: HashMap<NodeId, Sender<DroneCommand>>,
    send_command_node: HashMap<NodeId, Sender<NodeCommand>>,
    reciver_event: HashMap<NodeId, Receiver<DroneEvent>>,
    receriver_node_event: HashMap<NodeId, Receiver<NodeEvent>>,
    client_ui_state: UiState,
) -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Network Controller"),
        ..Default::default()
    };

    eframe::run_native(
        "Network Controller",
        options,
        Box::new(|cc| {
            Ok(Box::new(ControllerUi::new(
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
                cc,
            ))as Box<dyn eframe::App>)
        }),
    )
}