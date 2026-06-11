fn main() -> eframe::Result<()> {
    let app = mole_devtool::ParityLedgerApp::load_workspace_root()
        .expect("failed to load parity ledger workspace data");
    let options = eframe::NativeOptions::default();

    eframe::run_native(app.title(), options, Box::new(move |_cc| Ok(Box::new(app))))
}
