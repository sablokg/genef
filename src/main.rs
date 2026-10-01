/*
Gaurav Sablok
gsablok@proton.me
*/

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "CAFE gene family explorer",
        options,
        Box::new(|cc| Ok(Box::new(cafe_gui::app::App::new(cc)))),
    )
}
