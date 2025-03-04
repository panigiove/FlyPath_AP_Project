mod utility;

use crossbeam_channel::{unbounded, Receiver, Sender};
use eframe::egui;
use std::thread;
use utility::UIcommand;

struct MyApp {
    sender: Sender<UIcommand>,
    receiver: Receiver<String>,
    input_text: String,
    received_text: String,
}

impl MyApp {
    fn new(sender: Sender<UIcommand>, receiver: Receiver<String>) -> Self {
        Self {
            sender,
            receiver,
            input_text: String::new(),
            received_text: String::new(),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Comunicazione tra egui e Crossbeam");
            ui.text_edit_singleline(&mut self.input_text);
            if ui.button("Invia").clicked() {
                //self.sender.send(self.input_text.clone()).unwrap();
                self.input_text.clear();
            }
            if let Ok(message) = self.receiver.try_recv() {
                self.received_text = message;
            }
            ui.label(format!("Messaggio ricevuto: {}", self.received_text));
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let (ui_sender, ui_receiver) = unbounded();
    let (bg_sender, bg_receiver) = unbounded();

    // Thread in background che invia messaggi all'interfaccia utente
    thread::spawn(move || {
        loop {
            if let Ok(message) = bg_receiver.recv() {
                // println!("Thread in background ha ricevuto: {}", message);
                // ui_sender.send(format!("Elaborato: {}", message)).unwrap();
            }
        }
    });

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "App con egui e Crossbeam",
        options,
        Box::new(|_cc| Box::new(MyApp::new(bg_sender, ui_receiver))),
    )
}