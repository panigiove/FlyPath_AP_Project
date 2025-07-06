use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crossbeam_channel::{unbounded, Receiver, Sender};
use eframe::{egui, Frame};
use egui::Context;
use wg_2024::network::NodeId;
use client::ui::UiState;
use client::ui::ClientState;
use crate::utility::{ButtonEvent, ButtonsMessages, GraphAction, MessageType, NodeType};
use crate::view::GraphApp;
use crate::view::ButtonWindow;
use crate::view::MessagesWindow;
use crate::drawable::Drawable;

/// Gestisce solo la comunicazione e coordinazione tra componenti UI
pub struct ControllerUi {
    graph_app: GraphApp,
    button_window: ButtonWindow,
    messages_window: MessagesWindow,
}

impl ControllerUi {
    pub fn new(
        client_ui_state: Arc<Mutex<UiState>>,
        graph_updates_receiver: Receiver<GraphAction>,
        button_event_sender: Sender<ButtonEvent>,
        message_receiver: Receiver<MessageType>,
        message_sender: Sender<MessageType>,
        client_state_receiver: Receiver<(NodeId, ClientState)>,
        connections: HashMap<NodeId, Vec<NodeId>>,
        node_types: HashMap<NodeId, NodeType>
    ) -> Self {

        let (button_messages_sender, button_messages_receiver) = unbounded::<ButtonsMessages>();

        let (node_clicked_sender, node_clicked_receiver) = unbounded::<NodeId>(); // ✅ RIMOSSO: (NodeId) -> NodeId

        let graph_app = GraphApp::new(
            connections,
            node_types,
            graph_updates_receiver,
            node_clicked_sender,
            button_messages_receiver,
            message_sender.clone(),
            client_ui_state,
            client_state_receiver
        );

        let button_window = ButtonWindow::new(
            node_clicked_receiver,
            button_messages_sender,
            button_event_sender
        );

        let messages_window = MessagesWindow::new(message_receiver);

        message_sender.send(MessageType::Ok("System initialized".to_string())).ok();

        Self {
            graph_app,
            button_window,
            messages_window
        }
    }
}

impl ControllerUi {
    pub fn update(&mut self, ctx: &Context, _frame: &mut Frame) {

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.button_window.clear_selection();
        }

        self.button_window.update();
        self.messages_window.update();
        self.graph_app.handle_pending_events();

        self.graph_app.update();
    }

    pub fn render(&mut self, ctx: &Context, _frame: &mut Frame) {
        egui::TopBottomPanel::bottom("Message panel")
            .resizable(false)
            .exact_height(200.0)
            .show(ctx, |ui| {
                self.messages_window.render(ui);
            });

        egui::SidePanel::right("Possible actions")
            .resizable(false)
            .exact_width(350.0)
            .show(ctx, |ui| {
                self.button_window.render(ui);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.graph_app.render(ui);
        });
    }

    pub fn get_graph_app(&self) -> &GraphApp {
        &self.graph_app
    }

    pub fn get_graph_app_mut(&mut self) -> &mut GraphApp {
        &mut self.graph_app
    }
}