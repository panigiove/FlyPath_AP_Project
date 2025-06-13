use crate::comunication::{FromUiCommunication, ToUICommunication};
use crossbeam_channel::{Receiver, Sender};
use egui::RichText;
use hashbrown::HashSet;
use message::ChatResponse;
use std::collections::HashMap;
use wg_2024::network::NodeId;

pub struct ClientState {
    my_id: NodeId,
    current_chat: Option<NodeId>,
    unread_chat: HashSet<NodeId>,

    chat_message: HashMap<NodeId, Vec<(NodeId, String)>>,
    rx_from_worker: Receiver<ToUICommunication>,
    tx_to_worker: Sender<FromUiCommunication>,
}

impl ClientState {
    pub fn new(
        my_id: NodeId,
        rx_from_worker: Receiver<ToUICommunication>,
        tx_to_worker: Sender<FromUiCommunication>,
    ) -> Self {
        Self {
            my_id,
            current_chat: None,
            unread_chat: HashSet::default(),
            chat_message: HashMap::new(),
            rx_from_worker,
            tx_to_worker,
        }
    }
}

pub struct UiState {
    input: String,
    current_client: Option<NodeId>,
    client_states: HashMap<NodeId, ClientState>,
}

impl UiState {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            current_client: None,
            client_states: HashMap::new(),
        }
    }

    pub fn add_client(&mut self, client_id: NodeId, client_state: ClientState) {
        self.client_states.insert(client_id, client_state);
        if self.current_client.is_none() {
            self.current_client = Some(client_id);
        }
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
pub struct Ui;

impl Ui {
    pub fn render(ui: &mut egui::Ui, state: &mut UiState) {
        let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));

        // Tab system for clients
        ui.horizontal(|ui| {
            ui.label("Clients:");

            if state.client_states.is_empty() {
                ui.label("No clients connected");
                return;
            }

            egui::ScrollArea::horizontal()
                .id_salt("client_tabs")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let mut clients_to_show: Vec<NodeId> =
                            state.client_states.keys().copied().collect();
                        clients_to_show.sort();

                        for client_id in clients_to_show {
                            let is_current = state.current_client == Some(client_id);
                            let button_text = if is_current {
                                RichText::new(format!("Client {}", client_id))
                                    .strong()
                                    .color(egui::Color32::WHITE)
                            } else {
                                RichText::new(format!("Client {}", client_id))
                            };

                            let button = if is_current {
                                egui::Button::new(button_text).fill(egui::Color32::BLUE)
                            } else {
                                egui::Button::new(button_text)
                            };

                            if ui.add(button).clicked() {
                                state.current_client = Some(client_id);
                                state.input.clear();
                            }
                        }
                    });
                });
        });

        ui.separator();

        // Render current client if one is selected
        if let Some(current_client_id) = state.current_client {
            if let Some(client_state) = state.client_states.get_mut(&current_client_id) {
                Self::render_client(ui, &mut state.input, client_state, enter_pressed);
            } else {
                ui.label("Selected client not found");
            }
        } else {
            ui.label("No client selected");
        }
    }

    fn render_client(
        ui: &mut egui::Ui,
        input: &mut String,
        client_state: &mut ClientState,
        enter_pressed: bool,
    ) {
        let old_chat = client_state.current_chat;
        let available_rect = ui.available_rect_before_wrap();
        let total_width = available_rect.width();
        let total_height = available_rect.height();

        // Calculate column widths: 1/8 and 7/8
        let left_width = total_width / 8.0;
        let right_width = total_width * 7.0 / 8.0;

        ui.horizontal(|ui| {
            // Left column (1/8 width) - Chat List
            ui.allocate_ui_with_layout(
                egui::Vec2::new(left_width, total_height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    ui.heading("Chats");
                    ui.separator();

                    // Calculate heights for chat list and button area
                    let button_area_height = 80.0;
                    let heading_height = 40.0;
                    let chat_list_height = total_height - heading_height - button_area_height;

                    // Chat list area
                    ui.allocate_ui_with_layout(
                        egui::Vec2::new(left_width, chat_list_height),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            if !client_state.chat_message.is_empty() {
                                egui::ScrollArea::vertical()
                                    .id_salt("chat_list")
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        let mut chats_to_show: Vec<NodeId> =
                                            client_state.chat_message.keys().copied().collect();
                                        chats_to_show.sort(); // Sort for consistent order

                                        for node_id in chats_to_show {
                                            let unread =
                                                client_state.unread_chat.contains(&node_id);
                                            let label = if unread {
                                                RichText::new(format!("Chat {}", node_id))
                                                    .strong()
                                                    .color(egui::Color32::LIGHT_RED)
                                            } else {
                                                RichText::new(format!("Chat {}", node_id))
                                            };

                                            let is_selected =
                                                client_state.current_chat == Some(node_id);
                                            let button = if is_selected {
                                                egui::Button::new(label).fill(egui::Color32::BLUE)
                                            } else {
                                                egui::Button::new(label)
                                            };

                                            if ui.add(button).clicked() {
                                                client_state.current_chat = Some(node_id);
                                                client_state.unread_chat.remove(&node_id);
                                            }
                                        }
                                    });
                            } else {
                                ui.label("No chats available");
                            }
                        },
                    );

                    // Buttons area
                    ui.separator();
                    ui.allocate_ui_with_layout(
                        egui::Vec2::new(left_width, button_area_height - 10.0), // Subtract separator height
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            ui.vertical(|ui| {
                                // Reload ClientList button
                                if ui
                                    .add_sized(
                                        [left_width - 10.0, 25.0],
                                        egui::Button::new("Reload ClientList"),
                                    )
                                    .clicked()
                                {
                                    client_state
                                        .tx_to_worker
                                        .send(FromUiCommunication::AskClientList)
                                        .expect("Failed to transmit to UI");
                                }

                                ui.add_space(5.0); // Spazio tra i bottoni

                                // Reload All button
                                if ui
                                    .add_sized(
                                        [left_width - 10.0, 25.0],
                                        egui::Button::new("Reload All"),
                                    )
                                    .clicked()
                                {
                                    client_state
                                        .tx_to_worker
                                        .send(FromUiCommunication::RefreshTopology)
                                        .expect("Failed to transmit to UI");
                                }
                            });
                        },
                    );
                },
            );

            // Right column (7/8 width) - Current Chat
            ui.allocate_ui_with_layout(
                egui::Vec2::new(right_width, total_height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    // Clear input if chat changed
                    if old_chat != client_state.current_chat {
                        input.clear();
                    }

                    ui.heading("Current Chat");
                    ui.separator();

                    if let Some(current_chat_id) = client_state.current_chat {
                        if let Some(messages) = client_state.chat_message.get(&current_chat_id) {
                            // Calculate fixed heights
                            let heading_height = 40.0; // Height for heading + separator
                            let input_height = 50.0; // Fixed height for input area
                            let messages_height = total_height - heading_height - input_height;

                            // Messages area
                            ui.allocate_ui_with_layout(
                                egui::Vec2::new(right_width, messages_height),
                                egui::Layout::top_down(egui::Align::LEFT),
                                |ui| {
                                    egui::ScrollArea::vertical()
                                        .id_salt("messages")
                                        .auto_shrink([false, false])
                                        .show(ui, |ui| {
                                            for (sender_id, msg) in messages {
                                                ui.horizontal_wrapped(|ui| {
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "Node {}:",
                                                            sender_id
                                                        ))
                                                        .strong()
                                                        .color(egui::Color32::GRAY),
                                                    );
                                                    ui.add(egui::Label::new(msg).wrap());
                                                });
                                                ui.separator();
                                            }
                                        });
                                },
                            );

                            // Input area
                            ui.separator();
                            ui.allocate_ui_with_layout(
                                egui::Vec2::new(right_width, input_height - 10.0), // Subtract separator height
                                egui::Layout::top_down(egui::Align::LEFT),
                                |ui| {
                                    ui.horizontal(|ui| {
                                        let response = ui.text_edit_singleline(input);

                                        let send_button_clicked = ui.button("Send").clicked();
                                        let enter_send = response.lost_focus() && enter_pressed;

                                        if (send_button_clicked || enter_send)
                                            && !input.trim().is_empty()
                                        {
                                            // Add message to chat
                                            if let Some(chat) =
                                                client_state.chat_message.get_mut(&current_chat_id)
                                            {
                                                chat.push((client_state.my_id, input.clone()));
                                            }

                                            client_state
                                                .tx_to_worker
                                                .send(FromUiCommunication::SendChatMessage {
                                                    to_client: current_chat_id,
                                                    message: input.to_string(),
                                                })
                                                .expect("Failed to transmit to Worker");

                                            input.clear();
                                        }
                                    });
                                },
                            );
                        } else {
                            ui.label("Chat not found");
                        }
                    } else {
                        ui.label("Select a chat on the left");
                    }
                },
            );
        });
    }

    pub fn handle_drone_messages(state: &mut UiState) -> bool {
        let mut messages_handled = false;

        for (_, client_state) in state.client_states.iter_mut() {
            while let Ok(message) = client_state.rx_from_worker.try_recv() {
                messages_handled = true;
                if let ToUICommunication::ChatResponse { response } = message {
                    match response {
                        ChatResponse::ClientList(nids) => {
                            for nid in nids {
                                client_state.chat_message.entry(nid).or_default();
                            }
                        }
                        ChatResponse::MessageFrom { from: nid, message } => {
                            let messages = client_state
                                .chat_message
                                .entry(nid)
                                .or_insert_with(Vec::new);

                            if let Ok(message_string) = String::from_utf8(message) {
                                messages.push((nid, message_string));
                            } else {
                                messages.push((nid, "Invalid Message here".to_string()));
                            }

                            client_state.unread_chat.insert(nid);
                        }
                        _ => {}
                    }
                };
            }
        }

        messages_handled
    }
}
