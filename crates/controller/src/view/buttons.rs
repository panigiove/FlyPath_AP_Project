use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use eframe::App;
use wg_2024::network::NodeId;
use crate::utility::{ButtonEvent, ButtonsMessages};
use crate::utility::ButtonEvent::{ChangePdr, Crash, NewDrone};

struct ButtonWindow{
    //these two are the nodes that are selected at the exact instant we are working
    pub node_id1: Option<NodeId>, //entrambi inizializzati a None quando azione fatta dinuovo a none
    pub node_id2: Option<NodeId>,
    pub is_multiple_selection_allowed: bool, //inizializzato a false
    pub pdr_change: Option<f32>, //inizializza a none, quando cambio fatto dinuovo a none
    pub current_node_clicked: Option<NodeId>,
    pub current_pdr: Option<f32>,

    // Campi per l'input PDR (solo per drone e change PDR)
    pub show_pdr_input: bool, // mostra/nasconde la casella di input per drone
    pub show_change_pdr_input: bool, // mostra/nasconde la casella di input per change pdr
    pub input_pdr: String,    // contenuto della casella di input

    //CHANNELS
    pub reciver_node_clicked: Receiver<NodeId>, //riceve dal grafo il nodo che è stato premuto
    pub reciver_edge_clicked: Receiver<(NodeId, NodeId)>,
    pub sender_button_event: Sender<ButtonEvent>, //invia al controller le azioni da fare
    pub sender_buttom_messages: Sender<ButtonsMessages>, // dice al grafo quando deve deselezionare un nodo e quando ne può selezionare più di uno
}

impl Default for ButtonWindow {
    fn default() -> Self {
        let (sender_button_event, _receiver_button_event) = unbounded::<ButtonEvent>();
        let (sender_buttom_messages, _receiver_buttom_messages) = unbounded::<ButtonsMessages>();
        let (sender_node_clicked, receiver_node_clicked) = unbounded::<NodeId>();
        let (sender_edge_clicked, receiver_edge_clicked) = unbounded::<(NodeId, NodeId)>();

        Self {
            reciver_node_clicked: receiver_node_clicked,
            reciver_edge_clicked: receiver_edge_clicked,
            sender_button_event,
            sender_buttom_messages,
            node_id1: None,
            node_id2: None,
            is_multiple_selection_allowed: false,
            pdr_change: None,
            current_node_clicked: None,
            current_pdr: None,
            show_pdr_input: false,
            show_change_pdr_input: false,
            input_pdr: String::new(),
        }
    }
}

impl App for ButtonWindow{
    fn update(&mut self, ctx: &eframe::egui::Context, _: &mut eframe::Frame){
        eframe::egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Drone Control Panel");

            // Griglia 3x3 per i bottoni
            eframe::egui::Grid::new("button_grid")
                .num_columns(3)
                .spacing([10.0, 10.0])
                .show(ui, |ui| {
                    // Prima riga
                    if ui.button("Add New Drone").clicked() {
                        self.new_drone();
                    }
                    if ui.button("Add New Server").clicked() {
                        self.new_server();
                    }
                    if ui.button("Add New Client").clicked() {
                        self.new_client();
                    }
                    ui.end_row();

                    // Seconda riga
                    if ui.button("Add Connection").clicked() {
                        self.add_new_connection();
                    }
                    if ui.button("Delete Connection").clicked() {
                        self.remove_connection();
                    }
                    if ui.button("Change PDR").clicked() {
                        self.change_pdr();
                    }
                    ui.end_row();

                    // Terza riga
                    if ui.button("Crash Drone").clicked() {
                        self.let_drone_crash();
                    }
                    // Celle vuote per completare la griglia
                    ui.label("");
                    ui.label("");
                    ui.end_row();
                });

            ui.separator();

            // Input per PDR quando si vuole creare un nuovo drone
            if self.show_pdr_input {
                ui.group(|ui| {
                    ui.label("Inserisci PDR per il nuovo drone:");
                    ui.horizontal(|ui| {
                        ui.label("PDR (0.0 - 1.0):");
                        ui.text_edit_singleline(&mut self.input_pdr);

                        if ui.button("Create Drone").clicked() {
                            if let Ok(pdr) = self.input_pdr.parse::<f32>() {
                                if pdr >= 0.0 && pdr <= 1.0 {
                                    self.create_drone_with_pdr(pdr);
                                    self.show_pdr_input = false;
                                    self.input_pdr.clear();
                                } else {
                                    // Mostra errore - PDR deve essere tra 0 e 1
                                }
                            } else {
                                // Mostra errore - input non valido
                            }
                        }

                        if ui.button("Annulla").clicked() {
                            self.show_pdr_input = false;
                            self.input_pdr.clear();
                        }
                    });
                });
            }

            // Input per cambiare PDR di un nodo esistente
            if self.show_change_pdr_input {
                ui.group(|ui| {
                    ui.label(format!("Cambia PDR del nodo {:?}:", self.node_id1));
                    ui.horizontal(|ui| {
                        ui.label("Nuovo PDR (0.0 - 1.0):");
                        ui.text_edit_singleline(&mut self.input_pdr);

                        if ui.button("Cambia PDR").clicked() {
                            if let Ok(pdr) = self.input_pdr.parse::<f32>() {
                                if pdr >= 0.0 && pdr <= 1.0 {
                                    self.change_node_pdr(pdr);
                                    self.show_change_pdr_input = false;
                                    self.input_pdr.clear();
                                } else {
                                    // Mostra errore - PDR deve essere tra 0 e 1
                                }
                            } else {
                                // Mostra errore - input non valido
                            }
                        }

                        if ui.button("Annulla").clicked() {
                            self.show_change_pdr_input = false;
                            self.input_pdr.clear();
                        }
                    });
                });
            }

            // Informazioni di stato
            if let Some(node) = self.node_id1 {
                ui.label(format!("Selected Node 1: {}", node));
            }

            if let Some(node) = self.node_id2 {
                ui.label(format!("Selected Node 2: {}", node));
            }

            if self.is_multiple_selection_allowed {
                ui.label("Multiple selection mode: ON");
            }
        });
    }
}

impl ButtonWindow{
    pub fn new() -> Self{
        Self::default()
    }

    pub fn run(&mut self){
        loop {
            if let Ok(command) = self.reciver_node_clicked.try_recv() {
                self.node_clicked_handler(command);
            }

            if let Ok(command) = self.reciver_edge_clicked.try_recv(){
                // Gestisci click su edge se necessario
            }

            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        loop{
            select_biased!{
                recv(self.reciver_node_clicked) -> command =>{
                    if let Ok(command) = command{
                        self.node_clicked_handler(command);
                    }
                }
                default => {
                    // for (_, reciver) in self.receive_event.clone(){
                    //     if let Ok(event) = reciver.try_recv(){
                    //         self.event_handler(event);
                    //     }
                    // }
                }
            }
            std::thread::yield_now();
        }
    }

    pub fn node_clicked_handler(&mut self, node_id: NodeId){
        //teoricamente attivato dopo che noi abbiamo selezionato il primo nodo -> perchè successivamente si potrà selezionare il tasto per aggiungere un nuovo arco
        if self.is_multiple_selection_allowed{
            self.node_id2 = Some(node_id);
        }
        else{
            self.node_id1 = Some(node_id);
        }
    }

    pub fn new_drone(&mut self) {
        // Mostra la casella di input per il PDR invece di creare subito il drone
        self.hide_all_inputs();
        self.show_pdr_input = true;
        self.input_pdr.clear();
        // La funzione si ferma qui - aspetta che l'utente inserisca il PDR
        // Il messaggio verrà inviato quando l'utente clicca "Crea Drone"
    }

    pub fn new_server(&mut self) {
        if let Some(id) = self.node_id1{
            if let Err(e) = self.sender_button_event.try_send(ButtonEvent::NewServer(id)){
                
            }
        }
    }

    pub fn new_client(&mut self) {
        if let Some(id) = self.node_id1{
            if let Err(e) = self.sender_button_event.try_send(ButtonEvent::NewClient(id)){
                
            }
        }
    }

    pub fn change_pdr(&mut self) {
        if let Some(id) = self.node_id1{
            self.hide_all_inputs();
            self.show_change_pdr_input = true;

            // Gestisci il parsing in modo sicuro invece di usare unwrap()
            let pdr_value = match self.input_pdr.parse::<f32>() {
                Ok(value) if value >= 0.0 && value <= 1.0 => value,
                Ok(_) => {
                    println!("PDR value must be between 0.0 and 1.0");
                    return; // Esci dalla funzione se il valore non è valido
                }
                Err(_) => {
                    // Se il parsing fallisce, usa un valore di default o gestisci l'errore
                    println!("Invalid PDR input, using default value 0.0");
                    0.0 // oppure potresti fare return; per non inviare nessun evento
                }
            };

            if let Err(_e) = self.sender_button_event.try_send(ButtonEvent::ChangePdr(id, pdr_value)){
                println!("Failed to send change PDR event");
            }

            self.input_pdr.clear();
        }
        else {
            println!("To change the PDR you have fisrt to select a drone");
        }
    }

    fn hide_all_inputs(&mut self) {
        self.show_pdr_input = false;
        self.show_change_pdr_input = false;
    }

    pub fn create_drone_with_pdr(&mut self, pdr: f32) {
        if let Some(first_connection) = self.node_id1{
            if let Err(e) = self.sender_button_event.send(NewDrone(first_connection, pdr)) {
                eprintln!("Something went wrong during the creation of the new drone");
            } else {
                println!("Drone with PDR: {} has been created", pdr);
            }
        }
        else{
            println!("First you have to select a node as the first connection");
        }
    }

    pub fn change_node_pdr(&mut self, pdr: f32) {
        if let Some(node) = self.node_id1 {
            if let Err(e) = self.sender_button_event.send(ChangePdr(node, pdr)) {
                eprintln!("Errore nel cambio PDR: {}", e);
            } else {
                println!("PDR del nodo {} cambiato a {}", node, pdr);
            }
        }
    }

    pub fn add_new_connection(&mut self){
        self.is_multiple_selection_allowed = true;
        if let Some(id1) = self.node_id1{
            if let Some(id2) = self.node_id2{
                if let Err(e) = self.sender_button_event.send(ButtonEvent::NewConnection(id1, id2)) {
                    eprintln!(" ");
                } else {
                    println!(" ");
                }
            }
        }
        //magari togliamo gli altri pulsanti finchè non usciamo o scegliamo l'altro nodo
    }

    pub fn remove_connection(&mut self){
        self.is_multiple_selection_allowed = true;
        if let Some(id1) = self.node_id1{
            if let Some(id2) = self.node_id2{
                if let Err(e) = self.sender_button_event.send(ButtonEvent::RemoveConection(id1, id2)) {
                    eprintln!(" ");
                } else {
                    println!(" ");
                }
            }
        }
    }

    pub fn let_drone_crash(&self){
        if let Some(node) = self.node_id1{
            if let Err(e) = self.sender_button_event.send(Crash(node)) {
                eprintln!("Errore nel crash del drone: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;
    use wg_2024::network::NodeId;
    use crate::utility::{ButtonEvent, ButtonsMessages};

    // Helper function per creare una ButtonWindow di test con receiver per i test
    fn create_test_button_window() -> (ButtonWindow, crossbeam_channel::Receiver<ButtonEvent>) {
        let (sender_button_event, receiver_button_event) = unbounded::<ButtonEvent>();
        let (sender_buttom_messages, _receiver_buttom_messages) = unbounded::<ButtonsMessages>();
        let (sender_node_clicked, receiver_node_clicked) = unbounded::<NodeId>();
        let (sender_edge_clicked, receiver_edge_clicked) = unbounded::<(NodeId, NodeId)>();

        let window = ButtonWindow {
            reciver_node_clicked: receiver_node_clicked,
            reciver_edge_clicked: receiver_edge_clicked,
            sender_button_event,
            sender_buttom_messages,
            node_id1: None,
            node_id2: None,
            is_multiple_selection_allowed: false,
            pdr_change: None,
            current_node_clicked: None,
            current_pdr: None,
            show_pdr_input: false,
            show_change_pdr_input: false,
            input_pdr: String::new(),
        };

        (window, receiver_button_event)
    }

    #[test]
    fn test_button_window_default_initialization() {
        let (window, _receiver) = create_test_button_window();

        assert_eq!(window.node_id1, None);
        assert_eq!(window.node_id2, None);
        assert_eq!(window.is_multiple_selection_allowed, false);
        assert_eq!(window.pdr_change, None);
        assert_eq!(window.current_node_clicked, None);
        assert_eq!(window.current_pdr, None);
        assert_eq!(window.show_pdr_input, false);
        assert_eq!(window.show_change_pdr_input, false);
        assert_eq!(window.input_pdr, String::new());
    }

    #[test]
    fn test_node_clicked_handler_single_selection() {
        let (mut window, _receiver) = create_test_button_window();
        let test_node_id = 42;

        // Test selezione singola (default)
        window.node_clicked_handler(test_node_id);

        assert_eq!(window.node_id1, Some(test_node_id));
        assert_eq!(window.node_id2, None);
        assert_eq!(window.is_multiple_selection_allowed, false);
    }

    #[test]
    fn test_node_clicked_handler_multiple_selection() {
        let (mut window, _receiver) = create_test_button_window();
        let first_node = 42;
        let second_node = 24;

        // Prima selezione
        window.node_clicked_handler(first_node);
        assert_eq!(window.node_id1, Some(first_node));

        // Abilita selezione multipla
        window.is_multiple_selection_allowed = true;

        // Seconda selezione
        window.node_clicked_handler(second_node);

        assert_eq!(window.node_id1, Some(first_node));
        assert_eq!(window.node_id2, Some(second_node));
    }

    #[test]
    fn test_new_drone_shows_pdr_input() {
        let (mut window, _receiver) = create_test_button_window();

        // Verifica stato iniziale
        assert_eq!(window.show_pdr_input, false);
        assert_eq!(window.show_change_pdr_input, false);

        // Chiama new_drone
        window.new_drone();

        // Verifica che l'input PDR sia visibile
        assert_eq!(window.show_pdr_input, true);
        assert_eq!(window.show_change_pdr_input, false);
        assert_eq!(window.input_pdr, "");
    }

    #[test]
    fn test_change_pdr_shows_input_with_selected_node() {
        let (mut window, receiver) = create_test_button_window();
        let test_node = 42;

        // Seleziona un nodo
        window.node_id1 = Some(test_node);

        // Verifica stato iniziale
        assert_eq!(window.show_change_pdr_input, false);

        // Chiama change_pdr - ora dovrebbe gestire l'input vuoto senza panic
        window.change_pdr();

        // Verifica che l'input per cambio PDR sia visibile
        assert_eq!(window.show_change_pdr_input, true);
        assert_eq!(window.show_pdr_input, false);
        assert_eq!(window.input_pdr, ""); // Dovrebbe essere vuoto dopo clear()

        // Verifica che sia stato inviato un evento con il valore di default (0.0)
        if let Ok(event) = receiver.try_recv() {
            match event {
                ButtonEvent::ChangePdr(node_id, pdr) => {
                    assert_eq!(node_id, test_node);
                    assert_eq!(pdr, 0.0); // Valore di default per input vuoto
                }
                _ => panic!("Event type mismatch"),
            }
        } else {
            panic!("No event was sent");
        }
    }

    #[test]
    fn test_change_pdr_with_valid_input() {
        let (mut window, receiver) = create_test_button_window();
        let test_node = 42;
        let test_pdr = 0.5;

        // Seleziona un nodo e imposta un PDR valido
        window.node_id1 = Some(test_node);
        window.input_pdr = test_pdr.to_string();

        // Chiama change_pdr
        window.change_pdr();

        // Verifica che sia stato inviato l'evento con il valore corretto
        if let Ok(event) = receiver.try_recv() {
            match event {
                ButtonEvent::ChangePdr(node_id, pdr) => {
                    assert_eq!(node_id, test_node);
                    assert_eq!(pdr, test_pdr);
                }
                _ => panic!("Event type mismatch"),
            }
        } else {
            panic!("No event was sent");
        }
    }

    #[test]
    fn test_change_pdr_with_invalid_input() {
        let (mut window, receiver) = create_test_button_window();
        let test_node = 42;

        // Seleziona un nodo e imposta un PDR non valido
        window.node_id1 = Some(test_node);
        window.input_pdr = "invalid_input".to_string();

        // Chiama change_pdr
        window.change_pdr();

        // Verifica che sia stato inviato un evento con il valore di default (0.0)
        if let Ok(event) = receiver.try_recv() {
            match event {
                ButtonEvent::ChangePdr(node_id, pdr) => {
                    assert_eq!(node_id, test_node);
                    assert_eq!(pdr, 0.0); // Valore di default per input non valido
                }
                _ => panic!("Event type mismatch"),
            }
        } else {
            panic!("No event was sent");
        }
    }

    #[test]
    fn test_change_pdr_without_selected_node() {
        let (mut window, _receiver) = create_test_button_window();

        // Nessun nodo selezionato
        assert_eq!(window.node_id1, None);

        // Chiama change_pdr
        window.change_pdr();

        // L'input non dovrebbe essere visibile
        assert_eq!(window.show_change_pdr_input, false);
        assert_eq!(window.show_pdr_input, false);
    }

    #[test]
    fn test_hide_all_inputs() {
        let (mut window, _receiver) = create_test_button_window();

        // Imposta entrambi gli input come visibili
        window.show_pdr_input = true;
        window.show_change_pdr_input = true;

        // Nascondi tutti gli input
        window.hide_all_inputs();

        // Verifica che siano nascosti
        assert_eq!(window.show_pdr_input, false);
        assert_eq!(window.show_change_pdr_input, false);
    }

    #[test]
    fn test_create_drone_with_pdr_valid_input() {
        let (mut window, receiver) = create_test_button_window();
        let test_node = 42;
        let test_pdr = 0.5_f32;

        // Seleziona un nodo come prima connessione
        window.node_id1 = Some(test_node);

        // Crea drone con PDR valido
        window.create_drone_with_pdr(test_pdr);

        // Verifica che il messaggio sia stato inviato (controllando il receiver)
        if let Ok(event) = receiver.try_recv() {
            match event {
                ButtonEvent::NewDrone(node_id, pdr) => {
                    assert_eq!(node_id, test_node);
                    assert_eq!(pdr, test_pdr);
                }
                _ => panic!("Event type mismatch"),
            }
        } else {
            panic!("No event was sent");
        }
    }

    #[test]
    fn test_create_drone_without_selected_node() {
        let (mut window, receiver) = create_test_button_window();
        let test_pdr = 0.5_f32;

        // Nessun nodo selezionato
        assert_eq!(window.node_id1, None);

        // Prova a creare un drone
        window.create_drone_with_pdr(test_pdr);

        // Non dovrebbe essere inviato nessun evento
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn test_change_node_pdr_with_selected_node() {
        let (mut window, receiver) = create_test_button_window();
        let test_node = 42;
        let new_pdr = 0.8_f32;

        // Seleziona un nodo
        window.node_id1 = Some(test_node);

        // Cambia PDR
        window.change_node_pdr(new_pdr);

        // Verifica che il messaggio sia stato inviato
        if let Ok(event) = receiver.try_recv() {
            match event {
                ButtonEvent::ChangePdr(node_id, pdr) => {
                    assert_eq!(node_id, test_node);
                    assert_eq!(pdr, new_pdr);
                }
                _ => panic!("Event type mismatch"),
            }
        } else {
            panic!("No event was sent");
        }
    }

    #[test]
    fn test_add_new_connection_with_both_nodes() {
        let (mut window, receiver) = create_test_button_window();
        let node1 = 42;
        let node2 = 24;

        // Seleziona entrambi i nodi
        window.node_id1 = Some(node1);
        window.node_id2 = Some(node2);

        // Aggiungi connessione
        window.add_new_connection();

        // Verifica che la selezione multipla sia abilitata
        assert_eq!(window.is_multiple_selection_allowed, true);

        // Verifica che il messaggio sia stato inviato
        if let Ok(event) = receiver.try_recv() {
            match event {
                ButtonEvent::NewConnection(id1, id2) => {
                    assert_eq!(id1, node1);
                    assert_eq!(id2, node2);
                }
                _ => panic!("Event type mismatch"),
            }
        } else {
            panic!("No event was sent");
        }
    }

    #[test]
    fn test_let_drone_crash_with_selected_node() {
        let (mut window, receiver) = create_test_button_window();
        let test_node = 42;

        // Seleziona un nodo
        window.node_id1 = Some(test_node);

        // Fai crashare il drone
        window.let_drone_crash();

        // Verifica che il messaggio sia stato inviato
        if let Ok(event) = receiver.try_recv() {
            match event {
                ButtonEvent::Crash(node_id) => {
                    assert_eq!(node_id, test_node);
                }
                _ => panic!("Event type mismatch"),
            }
        } else {
            panic!("No event was sent");
        }
    }

    #[test]
    fn test_pdr_parsing_validation() {
        // Test di parsing per valori PDR validi
        let valid_inputs = vec!["0.0", "0.5", "1.0", "0.25", "0.75"];

        for input in valid_inputs {
            let parsed: Result<f32, _> = input.parse();
            assert!(parsed.is_ok());
            let value = parsed.unwrap();
            assert!(value >= 0.0 && value <= 1.0);
        }

        // Test di parsing per valori PDR non validi
        let invalid_inputs = vec!["abc", "-0.5", "1.5", "", "not_a_number"];

        for input in invalid_inputs {
            let parsed: Result<f32, _> = input.parse();
            if parsed.is_ok() {
                let value = parsed.unwrap();
                // Se il parsing riesce, il valore deve comunque essere fuori range
                assert!(value < 0.0 || value > 1.0);
            }
            // Se il parsing fallisce, va bene così
        }
    }

    #[test]
    fn test_sequential_operations() {
        let (mut window, receiver) = create_test_button_window();

        // Sequenza: Seleziona nodo -> Nuovo drone -> Input PDR -> Crea drone
        let test_node = 42;
        let test_pdr = 0.7_f32;

        // 1. Seleziona nodo
        window.node_clicked_handler(test_node);
        assert_eq!(window.node_id1, Some(test_node));

        // 2. Richiedi nuovo drone (dovrebbe mostrare input)
        window.new_drone();
        assert_eq!(window.show_pdr_input, true);

        // 3. Simula input PDR
        window.input_pdr = test_pdr.to_string();

        // 4. Crea drone con PDR
        window.create_drone_with_pdr(test_pdr);

        // Verifica evento inviato
        if let Ok(event) = receiver.try_recv() {
            match event {
                ButtonEvent::NewDrone(node_id, pdr) => {
                    assert_eq!(node_id, test_node);
                    assert_eq!(pdr, test_pdr);
                }
                _ => panic!("Event type mismatch"),
            }
        } else {
            panic!("No event was sent");
        }
    }
}