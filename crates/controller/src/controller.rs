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

pub struct Controller {
    pub controller_handler: ControllerHandler,
    pub graph_app: GraphApp,
    pub messages_window: MessagesWindow,
    pub button_window: ButtonWindow,
}

impl Controller {
    pub fn new(
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
        cc: &eframe::CreationContext<'_>,
    ) -> Self {
        // Channels setup
        let (button_sender, button_receiver) = unbounded::<ButtonEvent>();
        let (graph_action_sender, graph_action_receiver) = unbounded::<GraphAction>();
        let (message_sender, message_receiver) = unbounded::<MessageType>();
        let (sender_node_clicked, receiver_node_clicked) = unbounded::<NodeId>();
        let (sender_edge_clicked, receiver_edge_clicked) = unbounded::<(NodeId, NodeId)>();
        let (sender_buttom_messages, receiver_buttom_messages) = unbounded::<ButtonsMessages>();
        let (sender_message_type, receiver_message_type) = unbounded::<MessageType>();

        // Build node types map
        let mut node_types: HashMap<NodeId, NodeType> = HashMap::new();

        for (id, _) in drones.iter() {
            node_types.insert(*id, NodeType::Drone);
        }

        for (id, _) in clients.iter() {
            node_types.insert(*id, NodeType::Client);
        }

        for (id, _) in servers.iter() {
            node_types.insert(*id, NodeType::Server);
        }

        // Create components
        let controller_handler = ControllerHandler::new(
            drones,
            drones_type,
            drone_senders,
            clients,
            servers,
            connections.clone(),
            send_command_drone,
            send_command_node,
            reciver_event,
            receriver_node_event,
            client_ui_state,
            button_receiver,
            graph_action_sender,
            message_sender,
        );

        let graph_app = GraphApp::new(
            cc,
            connections,
            node_types,
            graph_action_receiver,
            sender_node_clicked,
            sender_edge_clicked,
            receiver_buttom_messages,
            sender_message_type,
        );

        let messages_window = MessagesWindow::new(message_receiver);

        let button_window = ButtonWindow::new(
            receiver_node_clicked,
            sender_buttom_messages,
            button_sender,
        );

        Self {
            controller_handler,
            graph_app,
            messages_window,
            button_window,
        }
    }

    pub fn spawn_threads(&mut self) -> ControllerUI {

        self.controller_handler.run();

        self.graph_app.run();

        self.messages_window.run();

        ControllerUI {
            button_window: self.button_window.clone(),
        }
    }
}

// Separate struct for UI components that need to run in the main thread
pub struct ControllerUI {
    button_window: ButtonWindow,
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
impl eframe::App for Controller {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request continuous updates
        ctx.request_repaint();

        // Process controller handler events (non-blocking)
        // This should be done in small chunks to avoid blocking the UI
        for _ in 0..10 {  // Process up to 10 events per frame
            if let Ok(command) = self.controller_handler.button_receiver.try_recv() {
                self.controller_handler.handle_button_event(command);
            }
        }

        // Update graph actions
        while let Ok(action) = self.graph_app.receiver_updates.try_recv() {
            self.graph_app.graph_action_handler(action);
        }

        // Layout implementation

        // Bottom panel for messages (150px height)
        self.messages_window.update(ctx);

        // Right side panel for buttons (300px width)
        egui::SidePanel::right("control_panel")
            .exact_width(300.0)
            .resizable(false)
            .show(ctx, |ui| {
                // Embed button window content
                egui::Window::new("Network Controls")
                    .vscroll(true)
                    .default_width(280.0)
                    .collapsible(false)
                    .anchor(egui::Align2::RIGHT_TOP, [0.0, 0.0])
                    .show(ctx, |ui| {
                        self.button_window.update(ctx);
                    });
            });

        // Central panel for graph (remaining space)
        egui::CentralPanel::default().show(ctx, |ui| {
            // The graph app update
            self.graph_app.update(ctx, _frame);
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
            Ok(Box::new(Controller::new(
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