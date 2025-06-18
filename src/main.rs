use client::comunication::{FromUiCommunication, ToUICommunication};
use client::ui::{ClientState, Ui, UiState};
use crossbeam_channel::{Receiver, Sender};
use eframe::{egui, Frame};
use initializer::start;
use std::collections::HashMap;
use std::process;
use std::sync::{Arc, Mutex};
use egui::Context;
use wg_2024::network::NodeId;

// TODO: make start more efficient, dont need to clone EVERY CHANNEL, and return USELESS CHANNELS
// TODO: gentle crash
fn main() -> eframe::Result {
    let (to_ui, from_ui, _handlers) = start().unwrap_or_else(|e| {
        eprintln!("Errore durante l'avvio del sistema: {}", e);
        process::exit(1);
    });

    let client_ui_state = _setup_ui_client_state(to_ui, from_ui);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([620.0, 540.0]),
        ..Default::default()
    };
    eframe::run_native(
        "FLYPATH",
        options,
        Box::new(|_cc| Ok(Box::<App>::new(App::new(client_ui_state)))),
    )
}

struct App {
    client_ui_state: Arc<Mutex<UiState>>,
}

impl App {
    fn new (ui_state: UiState) -> Self {
        Self {
            client_ui_state: Arc::new(Mutex::new(ui_state)),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            //TODO: draw controller
            ui.label("Hello from controller");
        });


        let ui_state_clone = self.client_ui_state.clone();
        ctx.show_viewport_deferred(
            egui::ViewportId::from_hash_of("CLIENT UI"),
            egui::ViewportBuilder::default()
                .with_title("Client Chat")
                .with_inner_size([640.0, 520.0]),
            move |ctx, class| {
                assert!(
                    class == egui::ViewportClass::Deferred,
                    "This egui backend doesn't support multiple viewports"
                );
                egui::CentralPanel::default().show(ctx, |ui| match ui_state_clone.lock() {
                    Ok(mut state) => {
                        let messages_handled = Ui::handle_drone_messages(&mut state);
                        if messages_handled {
                            ctx.request_repaint();
                        }
                        Ui::render(ui, &mut state)
                    }
                    Err(_poisoned) => {
                        ui.label("Error: State mutex is poisoned");
                    }
                });
            },
        );
    }
}

fn _setup_ui_client_state(
    to_ui: HashMap<NodeId, (Sender<ToUICommunication>, Receiver<ToUICommunication>)>,
    from_ui: HashMap<NodeId, (Sender<FromUiCommunication>, Receiver<FromUiCommunication>)>,
) -> UiState {
    let mut ui_state = UiState::new();

    for (node_id, (_tx_to_ui, rx_to_ui)) in to_ui {
        if let Some((tx_from_ui, _rx_from_ui_unused)) = from_ui.get(&node_id) {
            let client_state = ClientState::new(
                node_id,
                rx_to_ui,
                tx_from_ui.clone(),
            );
            ui_state.add_client(node_id, client_state);
        }
    }

    ui_state
}

