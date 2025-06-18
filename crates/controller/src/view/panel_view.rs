// fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
//     egui::TopBottomPanel::bottom("message_panel").show(ctx, |ui| {
//         self.message_window.update(ui);
//     });
//
//     egui::SidePanel::right("button_panel").show(ctx, |ui| {
//         self.button_window.update(ui);
//     });
//
//     egui::CentralPanel::default().show(ctx, |ui| {
//         self.graph_window.update(ctx, ui);
//     });
// }