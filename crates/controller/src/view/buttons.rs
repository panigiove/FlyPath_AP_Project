use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use eframe::App;
use wg_2024::network::NodeId;
use crate::utility::{ButtonEvent, ButtonsMessages};
use crate::utility::ButtonEvent::{ChangePdr, Crash, NewDrone};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct ButtonWindow {
    // ===== STATO INTERNO DELLA SELEZIONE =====
    pub node_id1: Option<NodeId>,
    pub node_id2: Option<NodeId>,
    pub multiple_selection_mode: bool,

    // ===== COMUNICAZIONE CON GRAPHAPP =====
    pub reciver_node_clicked: Receiver<NodeId>,          // Riceve click dai nodi
    pub sender_buttom_messages: Sender<ButtonsMessages>, // Solo sender per aggiornare visualizzazione

    // ===== COMUNICAZIONE CON CONTROLLER =====
    pub button_event_sender: Sender<ButtonEvent>,        // Invia comandi al controller

    // ===== UI STATE =====
    pub selected_pdr: f32,
}

impl ButtonWindow {
    pub fn new(
        reciver_node_clicked: Receiver<NodeId>,
        sender_buttom_messages: Sender<ButtonsMessages>,
        button_event_sender: Sender<ButtonEvent>,
    ) -> Self {
        Self {
            node_id1: None,
            node_id2: None,
            multiple_selection_mode: false,
            reciver_node_clicked,
            sender_buttom_messages,
            button_event_sender,
            selected_pdr: 0.1,
        }
    }

    pub fn update(&mut self, ctx: &egui::Context) {
        // 1. Gestisci click sui nodi
        self.handle_node_clicks();

        // 2. Mostra UI
        self.show_ui(ctx);
    }

    // ===== GESTIONE CLICK SUI NODI =====
    fn handle_node_clicks(&mut self) {
        if let Ok(clicked_node) = self.reciver_node_clicked.try_recv() {
            if self.multiple_selection_mode {
                // Modalità selezione multipla (per aggiungere edge)
                if self.node_id1.is_none() {
                    self.node_id1 = Some(clicked_node);
                } else if self.node_id2.is_none() && self.node_id1 != Some(clicked_node) {
                    self.node_id2 = Some(clicked_node);
                } else {
                    // Reset se click su nodo già selezionato o terzo nodo
                    self.node_id1 = Some(clicked_node);
                    self.node_id2 = None;
                }
            } else {
                // Modalità selezione singola
                self.node_id1 = Some(clicked_node);
                self.node_id2 = None;
            }

            // Aggiorna visualizzazione nel grafo
            self.update_graph_selection();
        }
    }

    // ===== UI PRINCIPALE =====
    fn show_ui(&mut self, ctx: &egui::Context) {
        egui::Window::new("Network Controls")
            .default_width(300.0)
            .show(ctx, |ui| {
                self.show_selection_info(ui);
                ui.separator();
                self.show_node_operations(ui);
                ui.separator();
                self.show_connection_operations(ui);
                ui.separator();
                self.show_creation_operations(ui);
            });
    }

    fn show_selection_info(&self, ui: &mut egui::Ui) {
        ui.heading("Selection");

        match (self.node_id1, self.node_id2) {
            (None, None) => {
                ui.label("No nodes selected");
            }
            (Some(id1), None) => {
                ui.label(format!("Selected: Node {}", id1));
            }
            (Some(id1), Some(id2)) => {
                ui.label(format!("Selected: Node {} and Node {}", id1, id2));
            }
            (None, Some(_)) => unreachable!(),
        }

        if self.multiple_selection_mode {
            ui.colored_label(egui::Color32::YELLOW, "Multiple selection mode active");
        }
    }

    fn show_node_operations(&mut self, ui: &mut egui::Ui) {
        ui.heading("Node Operations");

        // Remove Node
        ui.horizontal(|ui| {
            let can_remove = self.node_id1.is_some();
            ui.add_enabled_ui(can_remove, |ui| {
                if ui.button("🗑 Remove Node").clicked() {
                    if let Some(id) = self.node_id1 {
                        self.send_button_event(ButtonEvent::Crash(id));
                        self.clear_selection();
                    }
                }
            });
        });

        // Change PDR
        ui.horizontal(|ui| {
            let can_change_pdr = self.node_id1.is_some();
            ui.add_enabled_ui(can_change_pdr, |ui| {
                ui.label("PDR:");
                ui.add(egui::Slider::new(&mut self.selected_pdr, 0.0..=1.0).step_by(0.1));
                if ui.button("Apply").clicked() {
                    if let Some(id) = self.node_id1 {
                        self.send_button_event(ButtonEvent::ChangePdr(id, self.selected_pdr));
                    }
                }
            });
        });
    }

    fn show_connection_operations(&mut self, ui: &mut egui::Ui) {
        ui.heading("Connection Operations");

        // Add Edge
        ui.horizontal(|ui| {
            if ui.button("🔗 Add Edge Mode").clicked() {
                self.enter_multiple_selection_mode();
            }

            let can_add_edge = self.node_id1.is_some() && self.node_id2.is_some();
            ui.add_enabled_ui(can_add_edge, |ui| {
                if ui.button("✅ Create Edge").clicked() {
                    if let (Some(id1), Some(id2)) = (self.node_id1, self.node_id2) {
                        self.send_button_event(ButtonEvent::NewConnection(id1, id2));
                        self.clear_selection();
                    }
                }
            });
        });

        // Remove Edge
        ui.horizontal(|ui| {
            let can_remove_edge = self.node_id1.is_some() && self.node_id2.is_some();
            ui.add_enabled_ui(can_remove_edge, |ui| {
                if ui.button("❌ Remove Edge").clicked() {
                    if let (Some(id1), Some(id2)) = (self.node_id1, self.node_id2) {
                        self.send_button_event(ButtonEvent::RemoveConection(id1, id2));
                        self.clear_selection();
                    }
                }
            });
        });
    }

    fn show_creation_operations(&mut self, ui: &mut egui::Ui) {
        ui.heading("Create New Nodes");

        let has_selection = self.node_id1.is_some();

        ui.horizontal(|ui| {
            ui.add_enabled_ui(has_selection, |ui| {
                if ui.button("🤖 New Drone").clicked() {
                    if let Some(connection_id) = self.node_id1 {
                        self.send_button_event(ButtonEvent::NewDrone(connection_id, self.selected_pdr));
                        self.clear_selection();
                    }
                }
            });
        });

        ui.horizontal(|ui| {
            ui.add_enabled_ui(has_selection, |ui| {
                if ui.button("💻 New Client").clicked() {
                    if let Some(connection_id) = self.node_id1 {
                        self.send_button_event(ButtonEvent::NewClient(connection_id));
                        self.clear_selection();
                    }
                }
            });
        });

        ui.horizontal(|ui| {
            ui.add_enabled_ui(has_selection, |ui| {
                if ui.button("🖥 New Server").clicked() {
                    if let Some(connection_id) = self.node_id1 {
                        self.send_button_event(ButtonEvent::NewServer(connection_id));
                        self.clear_selection();
                    }
                }
            });
        });

        // Clear Selection
        ui.separator();
        if ui.button("🔄 Clear Selection").clicked() {
            self.clear_selection();
        }
    }

    // ===== HELPER METHODS =====

    fn enter_multiple_selection_mode(&mut self) {
        self.multiple_selection_mode = true;
        self.node_id1 = None;
        self.node_id2 = None;

        // Notifica al grafo che può selezionare più nodi
        let _ = self.sender_buttom_messages.try_send(ButtonsMessages::MultipleSelectionAllowed);
        self.update_graph_selection();
    }

    fn update_graph_selection(&self) {
        let _ = self.sender_buttom_messages.try_send(
            ButtonsMessages::UpdateSelection(self.node_id1, self.node_id2)
        );
    }

    fn clear_selection(&mut self) {
        self.node_id1 = None;
        self.node_id2 = None;
        self.multiple_selection_mode = false;

        // Notifica al grafo di pulire le selezioni
        let _ = self.sender_buttom_messages.try_send(ButtonsMessages::ClearAllSelections);
    }

    fn send_button_event(&self, event: ButtonEvent) {
        if let Err(e) = self.button_event_sender.try_send(event) {
            eprintln!("Failed to send button event: {}", e);
        }
    }
}

