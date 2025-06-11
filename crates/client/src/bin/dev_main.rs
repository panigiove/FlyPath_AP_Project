use client::comunication::{FromUiCommunication, ToUICommunication};
use client::ui::{ClientState, Ui, UiState};
use crossbeam_channel::{unbounded, Receiver, Sender};
use eframe::egui;
use message::ChatResponse;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn main() -> eframe::Result {
    env_logger::init();

    let mut client_channels = HashMap::new();

    // Create 3 demo clients
    let (tx_to_ui, rx_from_worker) = unbounded::<ToUICommunication>();
    let (tx_to_worker, rx_from_ui) = unbounded::<FromUiCommunication>();

    client_channels.insert(0, (rx_from_worker, tx_to_worker));

    let _worker_handle = thread::spawn(move || {
        simulate_worker(0, tx_to_ui, rx_from_ui);
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([620.0, 540.0]),
        ..Default::default()
    };
    eframe::run_native(
        "EMPTY TEST",
        options,
        Box::new(|_cc| Ok(Box::<MyApp>::new(MyApp::new(client_channels)))),
    )
}

fn simulate_worker(
    _client_id: u8,
    tx_to_ui: Sender<ToUICommunication>,
    rx_from_ui: Receiver<FromUiCommunication>,
) {
    thread::sleep(Duration::from_secs(3));
    tx_to_ui
        .send(ToUICommunication::ChatResponse {
            response: ChatResponse::ClientList(vec![1, 2, 3]),
        })
        .unwrap();
    thread::sleep(Duration::from_secs(3));
    tx_to_ui
        .send(ToUICommunication::ChatResponse {
            response: ChatResponse::MessageFrom {
                from: 1,
                message: "Ciao".to_string().into_bytes(),
            },
        })
        .unwrap();
    loop {
        let message = rx_from_ui.recv().unwrap();
        match message {
            FromUiCommunication::SendChatMessage {
                to_client: _,
                message: _,
            } => {
                tx_to_ui
                    .send(ToUICommunication::ChatResponse {
                        response: ChatResponse::MessageFrom {
                            from: 1,
                            message: "Messaggio ricevuto".to_string().into_bytes(),
                        },
                    })
                    .unwrap();
            }
            FromUiCommunication::AskClientList => {
                tx_to_ui
                    .send(ToUICommunication::ChatResponse {
                        response: ChatResponse::MessageFrom {
                            from: 1,
                            message: "Ricaricare Client List".to_string().into_bytes(),
                        },
                    })
                    .unwrap();
            }
            FromUiCommunication::RefreshTopology => {
                tx_to_ui
                    .send(ToUICommunication::ChatResponse {
                        response: ChatResponse::MessageFrom {
                            from: 1,
                            message: "Ricaricare la topologia".to_string().into_bytes(),
                        },
                    })
                    .unwrap();
            }
        }
    }
}

struct MyApp {
    ui_state: Arc<Mutex<UiState>>,
}

impl MyApp {
    fn new(
        client_channels: HashMap<u8, (Receiver<ToUICommunication>, Sender<FromUiCommunication>)>,
    ) -> Self {
        let mut ui_state = UiState::new();

        for (client_id, (rx_from_worker, tx_to_worker)) in client_channels {
            let client_state = ClientState::new(client_id, rx_from_worker, tx_to_worker);
            ui_state.add_client(client_id, client_state);
        }

        Self {
            ui_state: Arc::new(Mutex::new(ui_state)),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Hello from the root viewport");
        });

        let ui_state_clone = self.ui_state.clone();

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
