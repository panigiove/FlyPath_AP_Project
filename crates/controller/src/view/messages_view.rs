use crossbeam_channel::Receiver;
use egui::{Color32, RichText};
use crate::utility::{MessageType, ORANGE, LIGHT_BLUE, DARK_BLUE, LIGHT_ORANGE, DARK_GREEN};

use crate::drawable::{Drawable, PanelDrawable, PanelType};

pub struct MessagesWindow {
    pub messages_receiver: Receiver<MessageType>,
    pub log: Vec<MessageType>,
    pub max_messages: usize,
    pub auto_scroll: bool,
}

impl MessagesWindow {
    pub fn new(receiver: Receiver<MessageType>) -> Self {
        Self {
            messages_receiver: receiver,
            log: Vec::new(),
            max_messages: 5000,
            auto_scroll: true,
        }
    }

    pub fn handle_incoming_messages(&mut self) {
        while let Ok(message) = self.messages_receiver.try_recv() {
            self.log.push(message);
            if self.log.len() > self.max_messages {
                self.log.remove(0);
            }
        }
    }
}

impl Drawable for MessagesWindow {
    fn update(&mut self) {
        self.handle_incoming_messages();
    }

    fn render(&mut self, ui: &mut egui::Ui) {
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Messages")
                    .heading()
                    .color(Color32::from_rgb(14,137,146))
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Clear").clicked() {
                    self.log.clear();
                }
                ui.checkbox(&mut self.auto_scroll, "Auto-scroll");
                ui.label(format!("{}/{}", self.log.len(), self.max_messages));
            });
        });

        ui.separator();

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .max_height(150.0)
            .show(ui, |ui| {
                for (index, message) in self.log.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("[{}]", index + 1));

                        match message {
                            MessageType::Error(t) => {
                                let text = RichText::new(format!("❌ {}", t))
                                    .color(ORANGE);
                                ui.label(text);
                            }
                            MessageType::Ok(t) => {
                                let text = RichText::new(format!("✅ {}", t))
                                    .color(LIGHT_BLUE);
                                ui.label(text);
                            }
                            MessageType::PacketSent(t) => {
                                let text = RichText::new(format!("📤 {}", t))
                                    .color(DARK_GREEN);
                                ui.label(text);
                            }
                            MessageType::PacketDropped(t) => {
                                let text = RichText::new(format!("📥 {}", t))
                                    .color(LIGHT_ORANGE);
                                ui.label(text);
                            }
                            MessageType::Info(t) => {
                                let text = RichText::new(format!("ℹ️ {}", t))
                                    .color(DARK_BLUE);
                                ui.label(text);
                            }
                        }
                    });
                }

                if self.auto_scroll && !self.log.is_empty() {
                    ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                }
            });

        ui.add_space(5.0);
    }

    fn needs_continuous_updates(&self) -> bool {
        true
    }

    fn component_name(&self) -> &'static str {
        "Messages"
    }
}

impl PanelDrawable for MessagesWindow {
    fn preferred_panel(&self) -> PanelType {
        PanelType::Bottom
    }

    fn preferred_size(&self) -> Option<egui::Vec2> {
        Some(egui::Vec2::new(0.0, 200.0))
    }
    fn is_resizable(&self) -> bool {
        false
    }
}