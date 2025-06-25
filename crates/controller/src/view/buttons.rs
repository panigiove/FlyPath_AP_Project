use crossbeam_channel::{Receiver, Sender};
use wg_2024::network::NodeId;
use crate::utility::{ButtonEvent, ButtonsMessages};

// NUOVO IMPORT
use crate::drawable::{Drawable, PanelDrawable, PanelType};

#[derive(Clone)]
pub struct ButtonWindow {
    pub node_id1: Option<NodeId>,
    pub node_id2: Option<NodeId>,
    pub multiple_selection_mode: bool,
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
            multiple_selection_mode: false,
            reciver_node_clicked,
            sender_buttom_messages,
            button_event_sender,
            selected_pdr: 0.1,
        }
    }

    pub fn handle_node_clicks(&mut self) {
        if let Ok(clicked_node) = self.reciver_node_clicked.try_recv() {
            if self.multiple_selection_mode {
                if self.node_id1.is_none() {
                    self.node_id1 = Some(clicked_node);
                } else if self.node_id2.is_none() && self.node_id1 != Some(clicked_node) {
                    self.node_id2 = Some(clicked_node);
                } else {
                    self.node_id1 = Some(clicked_node);
                    self.node_id2 = None;
                }
            } else {
                self.node_id1 = Some(clicked_node);
                self.node_id2 = None;
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
        self.multiple_selection_mode = false;
        let _ = self.sender_buttom_messages.try_send(ButtonsMessages::ClearAllSelections);
    }

    pub fn send_button_event(&self, event: ButtonEvent) {
        if let Err(e) = self.button_event_sender.try_send(event) {
            eprintln!("Failed to send button event: {}", e);
        }
    }

    pub fn enter_multiple_selection_mode(&mut self) {
        self.multiple_selection_mode = true;
        self.node_id1 = None;
        self.node_id2 = None;
        let _ = self.sender_buttom_messages.try_send(ButtonsMessages::MultipleSelectionAllowed);
        self.update_graph_selection();
    }
}

// NUOVA IMPLEMENTAZIONE DRAWABLE
impl Drawable for ButtonWindow {
    fn update(&mut self) {
        self.handle_node_clicks();
    }

    fn render(&mut self, ui: &mut egui::Ui) {
        ui.add_space(5.0);
        ui.heading("🎛️ Network Controls");
        ui.separator();

        // Selection info
        ui.heading("🎯 Selection Info");
        match (self.node_id1, self.node_id2) {
            (None, None) => {
                ui.label("No nodes selected");
                ui.label("Click on a node to select it");
            }
            (Some(id1), None) => {
                ui.label(format!("Selected Node: {}", id1));
            }
            (Some(id1), Some(id2)) => {
                ui.label(format!("Selected Nodes: {} and {}", id1, id2));
                ui.label("Ready to create/remove edge");
            }
            (None, Some(_)) => unreachable!(),
        }

        if self.multiple_selection_mode {
            ui.colored_label(egui::Color32::YELLOW, "🔗 Multi-selection mode active");
        }

        ui.separator();

        // Node operations
        ui.heading("🔧 Node Operations");

        ui.horizontal(|ui| {
            let can_remove = self.node_id1.is_some();
            ui.add_enabled_ui(can_remove, |ui| {
                if ui.button("🗑️ Remove Node").clicked() {
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
                if ui.button("📡 Apply PDR").clicked() {
                    if let Some(id) = self.node_id1 {
                        self.send_button_event(ButtonEvent::ChangePdr(id, self.selected_pdr));
                    }
                }
            });
        });

        ui.separator();

        // Connection operations
        ui.heading("🔗 Connection Operations");

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

        ui.separator();

        // Creation operations
        ui.heading("➕ Create New Nodes");

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
                if ui.button("🖥️ New Server").clicked() {
                    if let Some(connection_id) = self.node_id1 {
                        self.send_button_event(ButtonEvent::NewServer(connection_id));
                        self.clear_selection();
                    }
                }
            });
        });

        ui.separator();
        if ui.button("🔄 Clear Selection").clicked() {
            self.clear_selection();
        }
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