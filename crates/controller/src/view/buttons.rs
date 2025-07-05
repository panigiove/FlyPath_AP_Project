use crossbeam_channel::{Receiver, Sender};
use egui::{Button, Color32, RichText};
use wg_2024::network::NodeId;
use crate::utility::{ButtonEvent, ButtonsMessages, LIGHT_BLUE, DARK_BLUE, LIGHT_ORANGE};
use crate::drawable::{Drawable, PanelDrawable, PanelType};
const BUTTON_TEXT_COLOR: Color32 = Color32::WHITE;

#[derive(Clone)]
pub struct ButtonWindow {
    pub node_id1: Option<NodeId>,
    pub node_id2: Option<NodeId>,

    //CHANNELS
    pub receiver_node_clicked: Receiver<NodeId>,
    pub sender_button_messages: Sender<ButtonsMessages>,
    pub button_event_sender: Sender<ButtonEvent>,
    pub selected_pdr: f32,
}

impl ButtonWindow {
    pub fn new(
        receiver_node_clicked: Receiver<NodeId>,
        sender_button_messages: Sender<ButtonsMessages>,
        button_event_sender: Sender<ButtonEvent>,
    ) -> Self {
        Self {
            node_id1: None,
            node_id2: None,
            receiver_node_clicked,
            sender_button_messages,
            button_event_sender,
            selected_pdr: 0.1,
        }
    }

    pub fn handle_node_clicks(&mut self) {
        if let Ok(clicked_node) = self.receiver_node_clicked.try_recv() {
            match (self.node_id1, self.node_id2) {
                (None, None) => {
                    self.node_id1 = Some(clicked_node);
                }
                (Some(id1), None) => {
                    if id1 == clicked_node {
                        self.node_id1 = None;
                    } else {
                        self.node_id2 = Some(clicked_node);
                    }
                }
                (Some(id1), Some(id2)) => {
                    if id1 == clicked_node {
                        self.node_id1 = self.node_id2;
                        self.node_id2 = None;
                    } else if id2 == clicked_node {
                        self.node_id2 = None;
                    }
                    else {
                        self.node_id2 = Some(clicked_node);
                    }
                }
                (None, Some(_)) => unreachable!(), //should never append
            }
            self.update_graph_selection();
        }
    }

    pub fn update_graph_selection(&self) {
        let _ = self.sender_button_messages.try_send(
            ButtonsMessages::UpdateSelection(self.node_id1, self.node_id2)
        );
    }

    pub fn clear_selection(&mut self) {
        self.node_id1 = None;
        self.node_id2 = None;

        let _ = self.sender_button_messages.try_send(ButtonsMessages::ClearAllSelections);
    }

    pub fn send_button_event(&self, event: ButtonEvent) {
        if let Err(e) = self.button_event_sender.try_send(event) {
            eprintln!("Failed to send button event: {}", e);
        }
    }

    // Helper functions
    fn can_do_node_operations(&self) -> bool {
        self.node_id1.is_some()
    }

    fn can_create_edge(&self) -> bool {
        self.node_id1.is_some() && self.node_id2.is_some()
    }

    fn can_create_server(&self) -> bool {
        self.node_id1.is_some() && self.node_id2.is_some()
    }

    // ✅ RIMOSSO: handle_keyboard_input() - ora gestito a livello globale nel ControllerUi
}

// Enum per descrivere lo stato di selezione
#[derive(Debug, Clone, PartialEq)]
pub enum SelectionState {
    None,
    OneNode(NodeId),
    TwoNodes(NodeId, NodeId),
}

impl ButtonWindow {
    pub fn selection_state(&self) -> SelectionState {
        match (self.node_id1, self.node_id2) {
            (None, None) => SelectionState::None,
            (Some(id1), None) => SelectionState::OneNode(id1),
            (Some(id1), Some(id2)) => SelectionState::TwoNodes(id1, id2),
            (None, Some(_)) => SelectionState::None, // Stato inconsistente, tratta come None
        }
    }
}

// IMPLEMENTAZIONE DRAWABLE
impl Drawable for ButtonWindow {
    fn update(&mut self) {
        self.handle_node_clicks();

    }

    fn render(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {

            ui.add_space(5.0);
            ui.label(
                RichText::new("Network Controls")
                    .heading()
                    .color(DARK_BLUE)
            );

            ui.label("Double click nodes to select them (max 2)");

            ui.separator();

            // SEZIONE OPERAZIONI NODI
            ui.label(
                RichText::new("Node Operations")
                    .strong()
                    .size(15.0)
                    .color(LIGHT_BLUE)
            );

            // Remove operations
            ui.horizontal(|ui| {
                let can_remove_node = self.can_do_node_operations();

                ui.add_enabled_ui(can_remove_node, |ui| {
                    if ui.add(
                        Button::new(RichText::new("Remove Node").color(BUTTON_TEXT_COLOR))
                            .fill(LIGHT_BLUE)
                    ).clicked() {
                        if let Some(id) = self.node_id1 {
                            self.send_button_event(ButtonEvent::Crash(id));
                            self.clear_selection();
                        }
                    }
                });
            });

            // PDR operations
            ui.horizontal(|ui| {
                ui.label("PDR:");
                ui.add(egui::Slider::new(&mut self.selected_pdr, 0.0..=1.0).step_by(0.01));
            });

            ui.horizontal(|ui| {
                let can_change_pdr = self.can_do_node_operations();
                ui.add_enabled_ui(can_change_pdr, |ui| {
                    if ui.add(
                        Button::new(RichText::new("Apply PDR").color(BUTTON_TEXT_COLOR))
                            .fill(LIGHT_BLUE)
                    ).clicked(){
                        if let Some(id) = self.node_id1 {
                            self.send_button_event(ButtonEvent::ChangePdr(id, self.selected_pdr));
                        }
                    }
                });
            });

            ui.separator();

            // SEZIONE CREAZIONE NODI
            ui.label(
                RichText::new("Create New Nodes")
                    .strong()
                    .size(15.0)
                    .color(LIGHT_BLUE)
            );

            // Creation operations
            ui.group(|ui| {
                ui.label("Requires 1 selected node");

                let has_one_node = self.can_do_node_operations();

                if !has_one_node {
                    ui.colored_label(Color32::GRAY, "Select a node to enable creation");
                }

                ui.horizontal(|ui| {
                    ui.add_enabled_ui(has_one_node, |ui| {
                        if ui.add(
                            Button::new(RichText::new("New Drone").color(BUTTON_TEXT_COLOR))
                                .fill(LIGHT_BLUE)
                        ).clicked() {
                            if let Some(connection_id) = self.node_id1 {
                                self.send_button_event(ButtonEvent::NewDrone(connection_id, self.selected_pdr));
                                self.clear_selection();
                            }
                        }
                    });

                    ui.add_enabled_ui(has_one_node, |ui| {
                        if ui.add(
                            Button::new(RichText::new("New Client").color(BUTTON_TEXT_COLOR))
                                .fill(LIGHT_BLUE)
                        ).clicked() {
                            if let Some(connection_id) = self.node_id1 {
                                self.send_button_event(ButtonEvent::NewClient(connection_id));
                                self.clear_selection();
                            }
                        }
                    });
                });
            });

            ui.group(|ui| {
                ui.label("Requires 2 selected nodes");

                let has_two_nodes = self.can_create_server();

                if !has_two_nodes {
                    if self.node_id1.is_some() {
                        ui.colored_label(Color32::GRAY, "Select a second node to create server");
                    } else {
                        ui.colored_label(Color32::GRAY, "Select 2 nodes to create server");
                    }
                }

                ui.horizontal(|ui| {
                    ui.add_enabled_ui(has_two_nodes, |ui| {
                        if ui.add(
                            Button::new(RichText::new("Create Server").color(BUTTON_TEXT_COLOR))
                                .fill(LIGHT_BLUE)
                        ).clicked() {
                            if let (Some(drone1), Some(drone2)) = (self.node_id1, self.node_id2) {
                                self.send_button_event(ButtonEvent::NewServerWithTwoConnections(drone1, drone2));
                                self.clear_selection();
                            }
                        }
                    });
                });
            });

            ui.separator();

            // ✅ SEZIONE CONNESSIONI - CORRETTA
            ui.label(
                RichText::new("Connection Operations")
                    .strong()
                    .size(15.0)
                    .color(LIGHT_BLUE)
            );

            let can_manage_edge = self.can_create_edge(); // Entrambi richiedono 2 nodi selezionati

            // Feedback per l'utente
            if !can_manage_edge {
                if self.node_id1.is_some() {
                    ui.colored_label(LIGHT_ORANGE, "Select a second node for edge operations");
                } else {
                    ui.colored_label(LIGHT_ORANGE, "Select 2 nodes to create or remove edges");
                }
            } else if let (Some(id1), Some(id2)) = (self.node_id1, self.node_id2) {
                ui.colored_label(Color32::GRAY,
                                 format!("Create or remove edge between {} and {}", id1, id2));
            }

            // ✅ PULSANTI AFFIANCATI
            ui.horizontal(|ui| {
                // ✅ CREATE EDGE BUTTON
                if ui.add_enabled(
                    can_manage_edge,
                    Button::new(RichText::new("Create Edge").color(BUTTON_TEXT_COLOR))
                        .fill(LIGHT_BLUE)
                ).clicked() {
                    if let (Some(id1), Some(id2)) = (self.node_id1, self.node_id2) {
                        self.send_button_event(ButtonEvent::NewConnection(id1, id2));
                        self.clear_selection();
                    }
                }

                // ✅ REMOVE EDGE BUTTON
                if ui.add_enabled(
                    can_manage_edge,
                    Button::new(RichText::new("Remove Edge").color(BUTTON_TEXT_COLOR))
                        .fill(LIGHT_BLUE)
                ).clicked() {
                    if let (Some(id1), Some(id2)) = (self.node_id1, self.node_id2) {
                        self.send_button_event(ButtonEvent::RemoveConection(id1, id2));
                        self.clear_selection();
                    }
                }
            });

            ui.separator();

            // Clear button
            ui.horizontal(|ui| {
                if ui.add(
                    Button::new(RichText::new("Clear All Selections").color(BUTTON_TEXT_COLOR))
                        .fill(DARK_BLUE)
                ).clicked() {
                    self.clear_selection();
                }
            });

            // SELECTION INFO dal basso con DEBUG
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {

                match self.selection_state() {
                    SelectionState::None => {
                        ui.colored_label(Color32::GRAY, "Double click nodes to select (max 2)");
                        ui.label("Nothing selected");
                    }
                    SelectionState::OneNode(id1) => {
                        ui.colored_label(LIGHT_BLUE, "Can create Client/Drone or select second node");
                        ui.label(format!("Selected Node: {}", id1));
                    }
                    SelectionState::TwoNodes(id1, id2) => {
                        ui.colored_label(LIGHT_BLUE, "Can create Server, Edge, or remove Edge!");
                        ui.label(format!("Selected Nodes: {} and {}", id1, id2));
                    }
                }

                ui.separator();
                ui.label(
                    RichText::new("Selection Info")
                        .heading()
                        .color(DARK_BLUE)
                );
            });
        });
    }

    fn needs_continuous_updates(&self) -> bool {
        true
    }

    fn component_name(&self) -> &'static str {
        "Network Controls"
    }
}

impl PanelDrawable for ButtonWindow {
    fn preferred_panel(&self) -> PanelType {
        PanelType::Right
    }

    fn preferred_size(&self) -> Option<egui::Vec2> {
        Some(egui::Vec2::new(350.0, 0.0))
    }

    fn is_resizable(&self) -> bool {
        false
    }
}