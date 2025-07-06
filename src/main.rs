use client::communication::{FromUiCommunication, ToUICommunication};
use client::ui::{ClientState, Ui, UiState};
use crossbeam_channel::{Receiver, Sender};
use eframe::{egui, Frame};
use initializer::start;
use std::collections::HashMap;
use std::process;
use std::sync::{Arc, Mutex};
use egui::Context;
use wg_2024::network::NodeId;
use controller::{ButtonEvent, ControllerUi, GraphAction, MessageType, NodeType};

// TODO: make start more efficient, dont need to clone EVERY CHANNEL, and return USELESS CHANNELS
// TODO: gentle crash

fn main() -> eframe::Result {
    // console print of debug
    env_logger::init();
    
    // redirect debug to app.log
    // env_logger::Builder::from_default_env()
    //     .target(env_logger::Target::Pipe(Box::new(File::create("app.log").unwrap())))
    //     .init();
    
    let args: Vec<String> = std::env::args().collect();
    let config_path = if args.len() > 1 {
        args[1].clone()
    } else {
        "./crates/initializer/src/test_data/input11.toml".to_string()
    };
    
    let (to_ui,
        from_ui,
        button_sender,
        graph_action_receiver,
        message_receiver,
        message_sender,
        client_state_receiver,
        connections,
        nodes) = start(&config_path).unwrap_or_else(|e| {  // ← PASSA IL PARAMETRO
        eprintln!("Errore durante l'avvio del sistema con config '{}': {}", config_path, e);
        process::exit(1);
    });

    let client_ui_state = _setup_ui_client_state(to_ui, from_ui);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1240.0, 1080.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("FLYPATH - Network Controller"),
        ..Default::default()
    };

    eframe::run_native(
        "FLYPATH",
        options,
        Box::new(move |cc| Ok(Box::<App>::new(App::new(
            cc,
            client_ui_state,
            button_sender,
            graph_action_receiver,
            message_receiver,
            message_sender,
            client_state_receiver,
            connections,
            nodes,
        )))),
    )
}

struct App {
    client_ui_state: Arc<Mutex<UiState>>,
    controller_ui: ControllerUi,
}

impl App {
    fn new (_cc: &eframe::CreationContext<'_>, ui_state: UiState, button_sender: Sender<ButtonEvent>,
            graph_action_receiver: Receiver<GraphAction>,
            message_receiver: Receiver<MessageType>,
            message_sender: Sender<MessageType>,
            client_state_receiver: Receiver<(NodeId, ClientState)>,
            connections: HashMap<NodeId, Vec<NodeId>>,
            nodes: HashMap<NodeId, NodeType>) -> Self {
        let client_ui_state =  Arc::new(Mutex::new(ui_state));
        let controller_ui = ControllerUi::new(
            client_ui_state.clone(),
            graph_action_receiver,
            button_sender,
            message_receiver,
            message_sender,
            client_state_receiver,
            connections,
            nodes,
        );
        
        Self {
            client_ui_state,
            controller_ui
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        self.controller_ui.update(ctx, _frame);

        self.controller_ui.render(ctx, _frame);

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

