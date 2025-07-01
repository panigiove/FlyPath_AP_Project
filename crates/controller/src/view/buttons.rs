use crossbeam_channel::{Receiver, Sender};
use egui::{Button, Color32, RichText};
use wg_2024::network::NodeId;
use crate::utility::{ButtonEvent, ButtonsMessages};

// NUOVO IMPORT
use crate::drawable::{Drawable, PanelDrawable, PanelType};

const BUTTON_COLOR: Color32 = Color32::from_rgb(140, 182, 188);
const BUTTON_TEXT_COLOR: Color32 = Color32::WHITE;

#[derive(Clone)]
pub struct ButtonWindow {
    pub node_id1: Option<NodeId>,
    pub node_id2: Option<NodeId>,
    // ✅ RIMOSSO: multiple_selection_mode - ora non serve più
    pub reciver_node_clicked: Receiver<NodeId>,
    pub sender_buttom_messages: Sender<ButtonsMessages>,
    pub button_event_sender: Sender<ButtonEvent>,
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
            // ✅ RIMOSSO: multiple_selection_mode
            reciver_node_clicked,
            sender_buttom_messages,
            button_event_sender,
            selected_pdr: 0.1,
        }
    }

    // ✅ LOGICA SEMPLIFICATA: Sempre massimo 2 nodi selezionabili
    pub fn handle_node_clicks(&mut self) {
        if let Ok(clicked_node) = self.reciver_node_clicked.try_recv() {
            // Logica semplice: sempre massimo 2 nodi
            match (self.node_id1, self.node_id2) {
                (None, None) => {
                    // Nessun nodo selezionato -> seleziona il primo
                    self.node_id1 = Some(clicked_node);
                }
                (Some(id1), None) => {
                    if id1 == clicked_node {
                        // Click sullo stesso nodo -> deseleziona
                        self.node_id1 = None;
                    } else {
                        // Click su nodo diverso -> seleziona il secondo
                        self.node_id2 = Some(clicked_node);
                    }
                }
                (Some(id1), Some(id2)) => {
                    if id1 == clicked_node {
                        // Click sul primo nodo -> rimuovilo, sposta il secondo al primo
                        self.node_id1 = self.node_id2;
                        self.node_id2 = None;
                    } else if id2 == clicked_node {
                        // Click sul secondo nodo -> rimuovilo
                        self.node_id2 = None;
                    } else {
                        // Click su nuovo nodo -> sostituisci il secondo
                        self.node_id2 = Some(clicked_node);
                    }
                }
                (None, Some(_)) => unreachable!(), // Non dovrebbe mai succedere
            }
            self.update_graph_selection();
        }
    }

    pub fn update_graph_selection(&self) {
        let _ = self.sender_buttom_messages.try_send(
            ButtonsMessages::UpdateSelection(self.node_id1, self.node_id2)
        );
    }

    pub fn clear_selection(&mut self) {
        self.node_id1 = None;
        self.node_id2 = None;
        // ✅ RIMOSSO: multiple_selection_mode = false
        let _ = self.sender_buttom_messages.try_send(ButtonsMessages::ClearAllSelections);
    }

    pub fn send_button_event(&self, event: ButtonEvent) {
        if let Err(e) = self.button_event_sender.try_send(event) {
            eprintln!("Failed to send button event: {}", e);
        }
    }

    // ✅ RIMOSSO: enter_multiple_selection_mode() - non serve più
}

// NUOVA IMPLEMENTAZIONE DRAWABLE
impl Drawable for ButtonWindow {
    fn render(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {

            ui.add_space(5.0);
            ui.label(
                RichText::new("Network Controls")
                    .heading()
                    .color(Color32::from_rgb(14,137,146))
            );
            ui.separator();

            // ✅ SEZIONE OPERAZIONI NODO (usando primo nodo)
            ui.label(
                RichText::new("Node Operations")
                    .strong()
                    .size(15.0)
                    .color(Color32::from_rgb(140,182,188))
            );

            ui.horizontal(|ui| {
                let can_remove = self.node_id1.is_some();
                ui.add_enabled_ui(can_remove, |ui| {
                    if ui.add(
                        Button::new(RichText::new("Remove Node").color(BUTTON_TEXT_COLOR))
                            .fill(BUTTON_COLOR)
                    ).clicked() {
                        if let Some(id) = self.node_id1 {
                            self.send_button_event(ButtonEvent::Crash(id));
                            self.clear_selection();
                        }
                    }
                });
            });

            ui.horizontal(|ui| {
                ui.label("PDR:");
                ui.add(egui::Slider::new(&mut self.selected_pdr, 0.0..=1.0).step_by(0.01));
            });

            ui.horizontal(|ui| {
                let can_change_pdr = self.node_id1.is_some();
                ui.add_enabled_ui(can_change_pdr, |ui| {
                    if ui.add(
                        Button::new(RichText::new("Apply PDR").color(BUTTON_TEXT_COLOR))
                            .fill(BUTTON_COLOR)
                    ).clicked(){
                        if let Some(id) = self.node_id1 {
                            self.send_button_event(ButtonEvent::ChangePdr(id, self.selected_pdr));
                        }
                    }
                });
            });

            ui.separator();

            // ✅ SEZIONE CREAZIONE NODI (usando primo nodo come connessione)
            ui.label(
                RichText::new("Create New Nodes")
                    .strong()
                    .size(15.0)
                    .color(Color32::from_rgb(140,182,188))
            );

            // ✅ CREAZIONE SEMPLIFICATA per Client e Drone
            ui.group(|ui| {
                ui.label("Requires 1 selected drone");

                let has_one_node = self.node_id1.is_some();

                if !has_one_node {
                    ui.label("Select a drone to enable creation");
                }

                ui.horizontal(|ui| {
                    ui.add_enabled_ui(has_one_node, |ui| {
                        if ui.add(
                            Button::new(RichText::new("New Drone").color(BUTTON_TEXT_COLOR))
                                .fill(BUTTON_COLOR)
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
                                .fill(BUTTON_COLOR)
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
                ui.label("Requires 2 selected drones");

                let has_two_nodes = self.node_id1.is_some() && self.node_id2.is_some();

                if !has_two_nodes {
                    if self.node_id1.is_some() {
                        ui.label("Select a second drone to create server");
                    } else {
                        ui.label("Select 2 drones to create server");
                    }
                }

                ui.horizontal(|ui| {
                    ui.add_enabled_ui(has_two_nodes, |ui| {
                        if ui.add(
                            Button::new(RichText::new("Create Server").color(BUTTON_TEXT_COLOR))
                                .fill(BUTTON_COLOR)
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

            // ✅ SEZIONE CONNESSIONI (usando entrambi i nodi)
            ui.label(
                RichText::new("Connection Operations")
                    .strong()
                    .size(15.0)
                    .color(Color32::from_rgb(140,182,188))
            );
            

            // ✅ Edge operations (richiedono 2 nodi)
            ui.horizontal(|ui| {
                let can_add_edge = self.node_id1.is_some() && self.node_id2.is_some();
                ui.add_enabled_ui(can_add_edge, |ui| {
                    if ui.add(
                        Button::new(RichText::new("Create Edge").color(BUTTON_TEXT_COLOR))
                            .fill(BUTTON_COLOR)
                    ).clicked() {
                        if let (Some(id1), Some(id2)) = (self.node_id1, self.node_id2) {
                            self.send_button_event(ButtonEvent::NewConnection(id1, id2));
                            self.clear_selection();
                        }
                    }
                });

                ui.add_enabled_ui(can_add_edge, |ui| {
                    if ui.add(
                        Button::new(RichText::new("Remove Edge").color(BUTTON_TEXT_COLOR))
                            .fill(BUTTON_COLOR)
                    ).clicked() {
                        if let (Some(id1), Some(id2)) = (self.node_id1, self.node_id2) {
                            self.send_button_event(ButtonEvent::RemoveConection(id1, id2));
                            self.clear_selection();
                        }
                    }
                });
            });

            ui.separator();

            // Clear button
            if ui.add(
                Button::new(RichText::new("Clear Selections").color(BUTTON_TEXT_COLOR))
                    .fill(BUTTON_COLOR)
            ).clicked() {
                self.clear_selection();
            }

            // ✅ SELECTION INFO dal basso (semplificata)
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {

                // ✅ LOGICA SEMPLIFICATA per Selection Info
                match (self.node_id1, self.node_id2) {
                    (None, None) => {
                        ui.colored_label(Color32::GRAY, "Click nodes to select (max 2)");
                        ui.label("No nodes selected");
                    }
                    (Some(id1), None) => {
                        ui.colored_label(Color32::from_rgb(14,137,146), "Can create Client/Drone or select second node");
                        ui.label(format!("Selected Node: {}", id1));
                    }
                    (Some(id1), Some(id2)) => {
                        ui.colored_label(Color32::from_rgb(14,137,146), "Can create Server or Edge!");
                        ui.label(format!("Selected Nodes: {} and {}", id1, id2));
                    }
                    (None, Some(_)) => unreachable!(),
                }

                ui.separator();

                ui.label(
                    RichText::new("Selection Info")
                        .strong()
                        .size(15.0)
                        .color(Color32::from_rgb(140,182,188))
                );
            });
        });
    }

    fn update(&mut self) {
        self.handle_node_clicks();
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
        Some(egui::Vec2::new(320.0, 0.0))
    }

    fn is_resizable(&self) -> bool {
        false
    }
}