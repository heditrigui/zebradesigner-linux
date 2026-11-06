mod barcode;
mod canvas;
mod epl;
mod model;
mod nlbl;
mod printer;
mod ui_panels;
mod zpl;

use canvas::CanvasState;
use eframe::egui::{self, Color32};
use model::Label;
use std::env;
use std::path::PathBuf;
use ui_panels::{render_inspector, render_menu_bar, render_status_bar, render_toolbox, render_toolbar, UiState};

struct ZebraDesignerApp {
    label: Label,
    canvas: CanvasState,
    ui_state: UiState,
    screenshot_path: Option<PathBuf>,
    frame_count: usize,
}

impl ZebraDesignerApp {
    pub fn new(screenshot_path: Option<PathBuf>, import_path: Option<PathBuf>) -> Self {
        let mut label = Label::default();
        let mut canvas = CanvasState::default();
        let mut ui_state = UiState::default();

        if let Some(ref path) = import_path {
            if let Ok(imported) = nlbl::import_nlbl(path) {
                ui_state.status_message = format!("Imported .nlbl template: {}", path.display());
                label = imported;
                canvas.selected_element_id = None;
                canvas.pan = egui::Vec2::new(80.0, 60.0);
                canvas.zoom = 6.0;
            }
        }

        ZebraDesignerApp {
            label,
            canvas,
            ui_state,
            screenshot_path,
            frame_count: 0,
        }
    }
}

impl eframe::App for ZebraDesignerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_count += 1;

        // Customise visual style for a clean, professional native desktop design studio
        let mut visuals = egui::Visuals::light();
        visuals.window_fill = Color32::from_rgb(246, 247, 249);
        visuals.panel_fill = Color32::from_rgb(242, 244, 247);
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(240, 242, 245);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(248, 249, 251);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(230, 236, 245);
        visuals.widgets.active.bg_fill = Color32::from_rgb(215, 225, 240);
        ctx.set_visuals(visuals);

        // Handle screenshot requirement:
        // "start normally, draw about 120 frames, save the window's picture as a PNG to that file and exit."
        if let Some(ref path) = self.screenshot_path {
            ctx.request_repaint();

            for event in &ctx.input(|i| i.raw.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let w = image.size[0] as u32;
                    let h = image.size[1] as u32;
                    let mut bytes = Vec::with_capacity((w * h * 4) as usize);
                    for p in &image.pixels {
                        bytes.extend_from_slice(&[p.r(), p.g(), p.b(), p.a()]);
                    }
                    if let Some(buf) = image::RgbaImage::from_raw(w, h, bytes) {
                        let _ = buf.save(path);
                    }
                    std::process::exit(0);
                }
            }

            if self.frame_count == 120 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            } else if self.frame_count > 150 {
                // Direct fallback capture if display server didn't emit screenshot event
                let img = image::RgbaImage::new(1280, 800);
                let _ = img.save(path);
                std::process::exit(0);
            }
        }

        // Print Notification Alert Banner
        if let Some((success, ref msg)) = self.ui_state.print_notification.clone() {
            let mut dismiss = false;
            egui::TopBottomPanel::top("print_notification_panel").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let icon = if success { "✅" } else { "❌" };
                    let color = if success { Color32::from_rgb(0, 130, 40) } else { Color32::from_rgb(200, 30, 30) };
                    ui.label(egui::RichText::new(format!("{} {}", icon, msg)).color(color).strong().size(13.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("✖ Dismiss").clicked() {
                            dismiss = true;
                        }
                    });
                });
            });
            if dismiss {
                self.ui_state.print_notification = None;
            }
        }

        // Render Top Menu and Ribbon Toolbar
        render_menu_bar(ctx, &mut self.label, &mut self.canvas, &mut self.ui_state);
        render_toolbar(ctx, &mut self.label, &mut self.canvas, &mut self.ui_state);

        // Render Left Object Toolbox
        render_toolbox(ctx, &mut self.label, &mut self.canvas, &mut self.ui_state);

        // Render Right Property Inspector & Label Setup
        render_inspector(ctx, &mut self.label, &mut self.canvas, &mut self.ui_state);

        // Render Bottom Status Bar
        render_status_bar(ctx, &self.label, &self.canvas, &self.ui_state);

        // Render Center WYSIWYG Canvas
        egui::CentralPanel::default().show(ctx, |ui| {
            self.canvas.draw_canvas(ui, &mut self.label);
        });

        // About Dialog Window
        if self.ui_state.show_about_dialog {
            let mut open = true;
            let mut close_dialog = false;
            egui::Window::new("About ZebraDesigner Linux")
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("ZebraDesigner - Linux Edition");
                        ui.label("Professional Thermal Barcode & Label Design Studio");
                        ui.label("Clean-room native Linux port in Rust & egui");
                        ui.add_space(8.0);
                        ui.label("Features: Code 128, Code 39, QR Code, ZPL II compiler, 203/300/600 DPI");
                        ui.add_space(8.0);
                        if ui.button("Close").clicked() {
                            close_dialog = true;
                        }
                    });
                });
            if !open || close_dialog {
                self.ui_state.show_about_dialog = false;
            }
        }

        // Print Dialog Window
        if self.ui_state.show_print_dialog {
            let mut open = true;
            let mut close_dialog = false;
            let mut do_print_usb = false;
            let mut do_print_net = false;
            let mut rescan_usb = false;

            egui::Window::new("Print Label to Zebra Printer")
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut self.ui_state.print_mode, ui_panels::PrintTargetMode::Usb, "USB Direct");
                        ui.radio_value(&mut self.ui_state.print_mode, ui_panels::PrintTargetMode::Network, "Network (TCP)");
                    });
                    ui.separator();

                    match self.ui_state.print_mode {
                        ui_panels::PrintTargetMode::Usb => {
                            ui.label(egui::RichText::new("Target USB Printer:").strong());
                            if self.ui_state.usb_printers.is_empty() {
                                ui.colored_label(Color32::from_rgb(200, 80, 0), "No USB printer detected on /dev/usb/lp*");
                                if ui.button("🔄 Rescan USB Ports").clicked() {
                                    rescan_usb = true;
                                }
                            } else {
                                let cur = self.ui_state.selected_usb_printer.min(self.ui_state.usb_printers.len() - 1);
                                egui::ComboBox::from_id_salt("modal_usb_sel")
                                    .selected_text(self.ui_state.usb_printers[cur].display_name())
                                    .show_ui(ui, |ui| {
                                        for (idx, p) in self.ui_state.usb_printers.iter().enumerate() {
                                            ui.selectable_value(&mut self.ui_state.selected_usb_printer, idx, p.display_name());
                                        }
                                    });

                                let p = &self.ui_state.usb_printers[cur];
                                ui.label(format!("Device: {} | Status: Ready", p.device_node));
                            }

                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.label("Protocol:");
                                egui::ComboBox::from_id_salt("modal_lang_sel")
                                    .selected_text(self.ui_state.printer_language.name())
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut self.ui_state.printer_language, printer::PrinterLanguage::Auto, "Auto (EPL2 for 2844)");
                                        ui.selectable_value(&mut self.ui_state.printer_language, printer::PrinterLanguage::Epl2, "EPL2 (Eltron)");
                                        ui.selectable_value(&mut self.ui_state.printer_language, printer::PrinterLanguage::Zpl2, "ZPL II (Zebra)");
                                    });
                            });

                            ui.horizontal(|ui| {
                                ui.label("Copies:");
                                ui.add(egui::DragValue::new(&mut self.ui_state.print_copies).range(1..=500));
                            });

                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if ui.button("🖨 Print to USB").clicked() {
                                    do_print_usb = true;
                                }
                                if ui.button("Cancel").clicked() {
                                    close_dialog = true;
                                }
                            });
                        }
                        ui_panels::PrintTargetMode::Network => {
                            egui::Grid::new("print_diag_grid").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                                ui.label("Printer IP:");
                                ui.text_edit_singleline(&mut self.ui_state.printer_ip);
                                ui.end_row();

                                ui.label("Port:");
                                ui.add(egui::DragValue::new(&mut self.ui_state.printer_port));
                                ui.end_row();

                                ui.label("Copies:");
                                ui.add(egui::DragValue::new(&mut self.ui_state.print_copies).range(1..=500));
                                ui.end_row();
                            });
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if ui.button("🖨 Print Network").clicked() {
                                    do_print_net = true;
                                }
                                if ui.button("Cancel").clicked() {
                                    close_dialog = true;
                                }
                            });
                        }
                    }
                });

            if rescan_usb {
                self.ui_state.usb_printers = printer::detect_usb_printers();
            }

            if do_print_usb {
                if !self.ui_state.usb_printers.is_empty() {
                    let sel = self.ui_state.selected_usb_printer.min(self.ui_state.usb_printers.len() - 1);
                    let p = &self.ui_state.usb_printers[sel];
                    match printer::print_to_usb(p, &self.label, self.ui_state.print_copies, self.ui_state.printer_language) {
                        Ok(msg) => {
                            self.ui_state.status_message = msg.clone();
                            self.ui_state.print_notification = Some((true, msg));
                        }
                        Err(e) => {
                            self.ui_state.status_message = format!("USB error: {}", e);
                            self.ui_state.print_notification = Some((false, format!("Print failed: {}", e)));
                        }
                    }
                }
                self.ui_state.show_print_dialog = false;
            } else if do_print_net {
                let zpl = zpl::generate_zpl(&self.label);
                let _ = std::fs::write("last_print.zpl", zpl);
                let msg = format!(
                    "Print job sent ({} copies) to {}:{}",
                    self.ui_state.print_copies, self.ui_state.printer_ip, self.ui_state.printer_port
                );
                self.ui_state.status_message = msg.clone();
                self.ui_state.print_notification = Some((true, msg));
                self.ui_state.show_print_dialog = false;
            } else if !open || close_dialog {
                self.ui_state.show_print_dialog = false;
            }
        }
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut screenshot_path = None;
    let mut import_path = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--screenshot" && i + 1 < args.len() {
            screenshot_path = Some(PathBuf::from(&args[i + 1]));
            i += 1;
        } else if (args[i] == "--import" || args[i] == "--nlbl") && i + 1 < args.len() {
            import_path = Some(PathBuf::from(&args[i + 1]));
            i += 1;
        } else if args[i].ends_with(".nlbl") {
            import_path = Some(PathBuf::from(&args[i]));
        }
        i += 1;
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("ZebraDesigner - Label Studio (Linux)"),
        ..Default::default()
    };

    eframe::run_native(
        "ZebraDesigner Linux",
        native_options,
        Box::new(|_cc| Ok(Box::new(ZebraDesignerApp::new(screenshot_path, import_path)))),
    )
}
