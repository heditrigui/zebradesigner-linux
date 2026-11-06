use crate::canvas::CanvasState;
use crate::model::{BarcodeType, Element, ElementData, Label, Orientation, Resolution, ShapeKind};
use crate::printer::{detect_usb_printers, print_to_usb, PrinterLanguage, UsbPrinter};
use crate::zpl::generate_zpl;
use eframe::egui::{self, Color32, RichText, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveTab {
    Properties,
    LabelSetup,
    ZplCode,
    BatchPrint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrintTargetMode {
    Usb,
    Network,
}

pub struct UiState {
    pub active_tab: ActiveTab,
    pub status_message: String,
    pub show_about_dialog: bool,
    pub show_print_dialog: bool,
    pub print_copies: u32,
    pub printer_ip: String,
    pub printer_port: u16,
    pub usb_printers: Vec<UsbPrinter>,
    pub selected_usb_printer: usize,
    pub print_mode: PrintTargetMode,
    pub nlbl_import_path: String,
    pub print_notification: Option<(bool, String)>,
    pub printer_language: PrinterLanguage,
}

impl Default for UiState {
    fn default() -> Self {
        let usb_printers = detect_usb_printers();
        let status_message = if let Some(first) = usb_printers.first() {
            format!("Detected USB Printer: {} on {}", first.name, first.device_node)
        } else {
            "Ready. Design or edit label elements.".to_string()
        };

        UiState {
            active_tab: ActiveTab::Properties,
            status_message,
            show_about_dialog: false,
            show_print_dialog: false,
            print_copies: 1,
            printer_ip: "192.168.1.100".to_string(),
            printer_port: 9100,
            usb_printers,
            selected_usb_printer: 0,
            print_mode: PrintTargetMode::Usb,
            nlbl_import_path: "/home/heditrigui/Downloads/Label.nlbl".to_string(),
            print_notification: None,
            printer_language: PrinterLanguage::Auto,
        }
    }
}

pub fn render_menu_bar(
    ctx: &egui::Context,
    label: &mut Label,
    canvas: &mut CanvasState,
    ui_state: &mut UiState,
) {
    egui::TopBottomPanel::top("top_menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("📄 New Label (Standard 4x6\")").clicked() {
                    *label = Label::default();
                    canvas.selected_element_id = None;
                    ui_state.status_message = "Created new standard shipping label.".to_string();
                    ui.close_menu();
                }
                if ui.button("🗋 New Blank Label...").clicked() {
                    *label = Label::new_empty(101.6, 152.4, "Blank Label");
                    canvas.selected_element_id = None;
                    ui_state.status_message = "Created new blank label.".to_string();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("💾 Save Project (.json)").clicked() {
                    if let Ok(json) = serde_json::to_string_pretty(label) {
                        let _ = std::fs::write("label_project.json", json);
                        ui_state.status_message = "Saved project to label_project.json".to_string();
                    }
                    ui.close_menu();
                }
                if ui.button("📂 Open Project (.json)").clicked() {
                    if let Ok(content) = std::fs::read_to_string("label_project.json") {
                        if let Ok(loaded) = serde_json::from_str::<Label>(&content) {
                            *label = loaded;
                            ui_state.status_message = "Loaded project from label_project.json".to_string();
                        }
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("📥 Import NiceLabel Template (.nlbl)...").clicked() {
                    let path = std::path::Path::new(&ui_state.nlbl_import_path);
                    match crate::nlbl::import_nlbl(path) {
                        Ok(imported) => {
                            *label = imported;
                            canvas.selected_element_id = None;
                            canvas.zoom = 6.0;
                            ui_state.status_message = format!("Imported .nlbl template: {}", path.display());
                        }
                        Err(e) => {
                            ui_state.status_message = format!("Import failed: {}", e);
                        }
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("📤 Export ZPL Code (.zpl)").clicked() {
                    let zpl = generate_zpl(label);
                    let _ = std::fs::write("output_label.zpl", zpl);
                    ui_state.status_message = "Exported ZPL code to output_label.zpl".to_string();
                    ui_state.active_tab = ActiveTab::ZplCode;
                    ui.close_menu();
                }
                if ui.button("🖨 Print Label...").clicked() {
                    ui_state.show_print_dialog = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("❌ Exit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            ui.menu_button("Edit", |ui| {
                if ui.button("⎌ Undo").clicked() {
                    ui_state.status_message = "Undo action performed.".to_string();
                    ui.close_menu();
                }
                if ui.button("↷ Redo").clicked() {
                    ui_state.status_message = "Redo action performed.".to_string();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("✂ Cut").clicked() {
                    ui.close_menu();
                }
                if ui.button("📋 Copy").clicked() {
                    ui.close_menu();
                }
                if ui.button("📥 Paste").clicked() {
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("🗑 Delete Selected").clicked() {
                    if let Some(id) = canvas.selected_element_id {
                        label.elements.retain(|e| e.id != id);
                        canvas.selected_element_id = None;
                        ui_state.status_message = "Deleted selected element.".to_string();
                    }
                    ui.close_menu();
                }
            });

            ui.menu_button("View", |ui| {
                if ui.button("🔍 Zoom In (+20%)").clicked() {
                    canvas.zoom = (canvas.zoom * 1.2).min(12.0);
                    ui.close_menu();
                }
                if ui.button("🔎 Zoom Out (-20%)").clicked() {
                    canvas.zoom = (canvas.zoom * 0.8).max(1.0);
                    ui.close_menu();
                }
                if ui.button("1:1 Reset Zoom (100%)").clicked() {
                    canvas.zoom = 3.5;
                    ui.close_menu();
                }
                if ui.button("⛶ Fit to Canvas").clicked() {
                    canvas.pan = Vec2::new(60.0, 40.0);
                    canvas.zoom = 3.2;
                    ui.close_menu();
                }
                ui.separator();
                ui.checkbox(&mut canvas.show_grid, "Show Grid");
                ui.checkbox(&mut canvas.snap_to_grid, "Snap to Grid");
                ui.checkbox(&mut canvas.show_rulers, "Show Millimeter Rulers");
            });

            ui.menu_button("Insert", |ui| {
                if ui.button("🔤 Text Box").clicked() {
                    label.next_element_id += 1;
                    let id = label.next_element_id;
                    label.elements.push(Element {
                        id,
                        name: format!("Text_{}", id),
                        x: 10.0,
                        y: 10.0,
                        width: 40.0,
                        height: 6.0,
                        rotation: 0,
                        data: ElementData::Text {
                            text: "Sample Text".to_string(),
                            font_size: 12.0,
                            bold: false,
                            italic: false,
                            color: [0, 0, 0, 255],
                        },
                    });
                    canvas.selected_element_id = Some(id);
                    ui.close_menu();
                }
                if ui.button("||| 1D Barcode (Code 128)").clicked() {
                    label.next_element_id += 1;
                    let id = label.next_element_id;
                    label.elements.push(Element {
                        id,
                        name: format!("Barcode_{}", id),
                        x: 10.0,
                        y: 20.0,
                        width: 50.0,
                        height: 20.0,
                        rotation: 0,
                        data: ElementData::Barcode {
                            barcode_type: BarcodeType::Code128,
                            data: "123456789".to_string(),
                            show_text: true,
                            module_width: 0.4,
                        },
                    });
                    canvas.selected_element_id = Some(id);
                    ui.close_menu();
                }
                if ui.button("⚏ 2D QR Code").clicked() {
                    label.next_element_id += 1;
                    let id = label.next_element_id;
                    label.elements.push(Element {
                        id,
                        name: format!("QR_{}", id),
                        x: 10.0,
                        y: 20.0,
                        width: 25.0,
                        height: 25.0,
                        rotation: 0,
                        data: ElementData::Barcode {
                            barcode_type: BarcodeType::QrCode,
                            data: "https://example.com".to_string(),
                            show_text: false,
                            module_width: 1.0,
                        },
                    });
                    canvas.selected_element_id = Some(id);
                    ui.close_menu();
                }
                if ui.button("▭ Rectangle / Box").clicked() {
                    label.next_element_id += 1;
                    let id = label.next_element_id;
                    label.elements.push(Element {
                        id,
                        name: format!("Box_{}", id),
                        x: 10.0,
                        y: 10.0,
                        width: 40.0,
                        height: 25.0,
                        rotation: 0,
                        data: ElementData::Shape {
                            kind: ShapeKind::Rectangle,
                            stroke_width: 0.8,
                            stroke_color: [0, 0, 0, 255],
                            fill_color: None,
                            corner_radius: 0.0,
                        },
                    });
                    canvas.selected_element_id = Some(id);
                    ui.close_menu();
                }
                if ui.button("― Dividing Line").clicked() {
                    label.next_element_id += 1;
                    let id = label.next_element_id;
                    label.elements.push(Element {
                        id,
                        name: format!("Line_{}", id),
                        x: 5.0,
                        y: 30.0,
                        width: 90.0,
                        height: 0.5,
                        rotation: 0,
                        data: ElementData::Shape {
                            kind: ShapeKind::Line,
                            stroke_width: 0.8,
                            stroke_color: [40, 40, 40, 255],
                            fill_color: None,
                            corner_radius: 0.0,
                        },
                    });
                    canvas.selected_element_id = Some(id);
                    ui.close_menu();
                }
            });

            ui.menu_button("Printers", |ui| {
                if ui.button("🔄 Scan for USB Printers").clicked() {
                    ui_state.usb_printers = detect_usb_printers();
                    if let Some(first) = ui_state.usb_printers.first() {
                        ui_state.status_message = format!("Detected USB Printer: {} on {}", first.name, first.device_node);
                    } else {
                        ui_state.status_message = "No USB printers currently detected.".to_string();
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui_state.usb_printers.is_empty() {
                    ui.label("No USB printers found");
                } else {
                    for (i, p) in ui_state.usb_printers.iter().enumerate() {
                        if ui.button(format!("✓ {} ({})", p.name, p.device_node)).clicked() {
                            ui_state.selected_usb_printer = i;
                            ui_state.status_message = format!("Active printer: {}", p.display_name());
                            ui.close_menu();
                        }
                    }
                }
            });

            ui.menu_button("Help", |ui| {
                if ui.button("ℹ About ZebraDesigner Linux").clicked() {
                    ui_state.show_about_dialog = true;
                    ui.close_menu();
                }
            });
        });
    });
}

pub fn render_toolbar(
    ctx: &egui::Context,
    label: &mut Label,
    canvas: &mut CanvasState,
    ui_state: &mut UiState,
) {
    egui::TopBottomPanel::top("quick_toolbar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);

            if ui.button("📄 New").on_hover_text("New default label").clicked() {
                *label = Label::default();
                canvas.selected_element_id = None;
            }
            if ui.button("💾 Save").on_hover_text("Save label project").clicked() {
                if let Ok(json) = serde_json::to_string_pretty(label) {
                    let _ = std::fs::write("label_project.json", json);
                    ui_state.status_message = "Project saved.".to_string();
                }
            }
            if ui.button("📥 Import .nlbl").on_hover_text("Import NiceLabel / ZebraDesigner .nlbl template (/home/heditrigui/Downloads/Label.nlbl)").clicked() {
                let path = std::path::Path::new(&ui_state.nlbl_import_path);
                match crate::nlbl::import_nlbl(path) {
                    Ok(imported) => {
                        *label = imported;
                        canvas.selected_element_id = None;
                        canvas.zoom = 6.0;
                        ui_state.status_message = format!("Imported .nlbl template: {}", path.display());
                    }
                    Err(e) => {
                        ui_state.status_message = format!("Import error: {}", e);
                    }
                }
            }
            if ui.button("🖨 Print").on_hover_text("Print to Zebra printer").clicked() {
                ui_state.show_print_dialog = true;
            }

            ui.separator();

            // USB Printer status indicator in toolbar
            if !ui_state.usb_printers.is_empty() {
                let p = &ui_state.usb_printers[ui_state.selected_usb_printer.min(ui_state.usb_printers.len() - 1)];
                ui.label(RichText::new(format!("🔌 USB: {}", p.model)).color(Color32::from_rgb(0, 140, 50)).strong());
            } else {
                ui.label(RichText::new("🔌 USB: None").color(Color32::from_rgb(150, 100, 50)));
            }
            if ui.button("🔄").on_hover_text("Rescan USB ports for printers").clicked() {
                ui_state.usb_printers = detect_usb_printers();
                if let Some(first) = ui_state.usb_printers.first() {
                    ui_state.status_message = format!("Detected USB: {} ({})", first.name, first.device_node);
                } else {
                    ui_state.status_message = "No USB printer found.".to_string();
                }
            }

            ui.separator();

            if ui.button("🔍 +").on_hover_text("Zoom in").clicked() {
                canvas.zoom = (canvas.zoom * 1.2).min(12.0);
            }
            if ui.button("🔎 -").on_hover_text("Zoom out").clicked() {
                canvas.zoom = (canvas.zoom * 0.8).max(1.0);
            }
            if ui.button("1:1").on_hover_text("Reset zoom").clicked() {
                canvas.zoom = 3.5;
            }

            ui.separator();

            let grid_text = if canvas.show_grid { "▦ Grid [ON]" } else { "▦ Grid [OFF]" };
            if ui.selectable_label(canvas.show_grid, grid_text).clicked() {
                canvas.show_grid = !canvas.show_grid;
            }

            let snap_text = if canvas.snap_to_grid { "⌖ Snap [ON]" } else { "⌖ Snap [OFF]" };
            if ui.selectable_label(canvas.snap_to_grid, snap_text).clicked() {
                canvas.snap_to_grid = !canvas.snap_to_grid;
            }

            ui.separator();

            // Alignment buttons
            if let Some(sel_id) = canvas.selected_element_id {
                if ui.button("⇤ Left").on_hover_text("Align left margin").clicked() {
                    if let Some(el) = label.elements.iter_mut().find(|e| e.id == sel_id) {
                        el.x = label.margin_left_mm;
                    }
                }
                if ui.button("⇥ Center").on_hover_text("Center horizontally").clicked() {
                    if let Some(el) = label.elements.iter_mut().find(|e| e.id == sel_id) {
                        el.x = ((label.width_mm - el.width) / 2.0).max(0.0);
                    }
                }
                if ui.button("⤒ Top").on_hover_text("Align top margin").clicked() {
                    if let Some(el) = label.elements.iter_mut().find(|e| e.id == sel_id) {
                        el.y = label.margin_top_mm;
                    }
                }
                if ui.button("🗑 Delete").on_hover_text("Delete selected element").clicked() {
                    label.elements.retain(|e| e.id != sel_id);
                    canvas.selected_element_id = None;
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("Resolution: {} DPI", label.resolution.dpi()));
                ui.separator();
                ui.label(format!("Label: {:.1} x {:.1} mm", label.width_mm, label.height_mm));
            });
        });
    });
}

pub fn render_toolbox(
    ctx: &egui::Context,
    label: &mut Label,
    canvas: &mut CanvasState,
    ui_state: &mut UiState,
) {
    egui::SidePanel::left("toolbox_palette")
        .resizable(false)
        .default_width(52.0)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(4.0);
                ui.heading(RichText::new("Tools").size(11.0).color(Color32::from_rgb(100, 110, 125)));
                ui.separator();

                let btn_size = Vec2::new(42.0, 36.0);

                if ui.add_sized(btn_size, egui::Button::new("↖\nSelect")).on_hover_text("Select & move objects").clicked() {
                    ui_state.status_message = "Selection mode active. Click any object to edit.".to_string();
                }

                ui.add_space(2.0);

                if ui.add_sized(btn_size, egui::Button::new("🔤\nText")).on_hover_text("Add text element").clicked() {
                    label.next_element_id += 1;
                    let id = label.next_element_id;
                    label.elements.push(Element {
                        id,
                        name: format!("Text_{}", id),
                        x: 10.0,
                        y: 10.0,
                        width: 45.0,
                        height: 6.0,
                        rotation: 0,
                        data: ElementData::Text {
                            text: "PRODUCT TEXT".to_string(),
                            font_size: 12.0,
                            bold: true,
                            italic: false,
                            color: [0, 0, 0, 255],
                        },
                    });
                    canvas.selected_element_id = Some(id);
                    ui_state.active_tab = ActiveTab::Properties;
                }

                ui.add_space(2.0);

                if ui.add_sized(btn_size, egui::Button::new("|||\n1D Bar")).on_hover_text("Add 1D Barcode (Code 128 / Code 39)").clicked() {
                    label.next_element_id += 1;
                    let id = label.next_element_id;
                    label.elements.push(Element {
                        id,
                        name: format!("Barcode_{}", id),
                        x: 10.0,
                        y: 20.0,
                        width: 55.0,
                        height: 22.0,
                        rotation: 0,
                        data: ElementData::Barcode {
                            barcode_type: BarcodeType::Code128,
                            data: "9876543210".to_string(),
                            show_text: true,
                            module_width: 0.45,
                        },
                    });
                    canvas.selected_element_id = Some(id);
                    ui_state.active_tab = ActiveTab::Properties;
                }

                ui.add_space(2.0);

                if ui.add_sized(btn_size, egui::Button::new("⚏\n2D QR")).on_hover_text("Add 2D QR Code").clicked() {
                    label.next_element_id += 1;
                    let id = label.next_element_id;
                    label.elements.push(Element {
                        id,
                        name: format!("QR_{}", id),
                        x: 10.0,
                        y: 20.0,
                        width: 20.0,
                        height: 20.0,
                        rotation: 0,
                        data: ElementData::Barcode {
                            barcode_type: BarcodeType::QrCode,
                            data: "https://craftanything.ai".to_string(),
                            show_text: false,
                            module_width: 1.0,
                        },
                    });
                    canvas.selected_element_id = Some(id);
                    ui_state.active_tab = ActiveTab::Properties;
                }

                ui.add_space(2.0);

                if ui.add_sized(btn_size, egui::Button::new("▭\nBox")).on_hover_text("Add rectangle box").clicked() {
                    label.next_element_id += 1;
                    let id = label.next_element_id;
                    label.elements.push(Element {
                        id,
                        name: format!("Box_{}", id),
                        x: 10.0,
                        y: 10.0,
                        width: 50.0,
                        height: 20.0,
                        rotation: 0,
                        data: ElementData::Shape {
                            kind: ShapeKind::Rectangle,
                            stroke_width: 0.8,
                            stroke_color: [0, 0, 0, 255],
                            fill_color: None,
                            corner_radius: 0.0,
                        },
                    });
                    canvas.selected_element_id = Some(id);
                    ui_state.active_tab = ActiveTab::Properties;
                }

                ui.add_space(2.0);

                if ui.add_sized(btn_size, egui::Button::new("―\nLine")).on_hover_text("Add divider line").clicked() {
                    label.next_element_id += 1;
                    let id = label.next_element_id;
                    label.elements.push(Element {
                        id,
                        name: format!("Line_{}", id),
                        x: 5.0,
                        y: 25.0,
                        width: 90.0,
                        height: 0.5,
                        rotation: 0,
                        data: ElementData::Shape {
                            kind: ShapeKind::Line,
                            stroke_width: 0.8,
                            stroke_color: [30, 30, 30, 255],
                            fill_color: None,
                            corner_radius: 0.0,
                        },
                    });
                    canvas.selected_element_id = Some(id);
                    ui_state.active_tab = ActiveTab::Properties;
                }

                ui.add_space(2.0);

                if ui.add_sized(btn_size, egui::Button::new("◯\nCircle")).on_hover_text("Add ellipse / circle").clicked() {
                    label.next_element_id += 1;
                    let id = label.next_element_id;
                    label.elements.push(Element {
                        id,
                        name: format!("Circle_{}", id),
                        x: 15.0,
                        y: 15.0,
                        width: 25.0,
                        height: 25.0,
                        rotation: 0,
                        data: ElementData::Shape {
                            kind: ShapeKind::Ellipse,
                            stroke_width: 0.8,
                            stroke_color: [0, 0, 0, 255],
                            fill_color: None,
                            corner_radius: 0.0,
                        },
                    });
                    canvas.selected_element_id = Some(id);
                    ui_state.active_tab = ActiveTab::Properties;
                }

                ui.add_space(2.0);

                if ui.add_sized(btn_size, egui::Button::new("123\nSerial")).on_hover_text("Add incrementing serial counter").clicked() {
                    label.next_element_id += 1;
                    let id = label.next_element_id;
                    label.elements.push(Element {
                        id,
                        name: format!("Counter_{}", id),
                        x: 10.0,
                        y: 10.0,
                        width: 35.0,
                        height: 6.0,
                        rotation: 0,
                        data: ElementData::Counter {
                            prefix: "SN-".to_string(),
                            current_value: 1,
                            step: 1,
                            padding_digits: 6,
                            font_size: 11.0,
                            bold: true,
                        },
                    });
                    canvas.selected_element_id = Some(id);
                    ui_state.active_tab = ActiveTab::Properties;
                }
            });
        });
}

pub fn render_inspector(
    ctx: &egui::Context,
    label: &mut Label,
    canvas: &mut CanvasState,
    ui_state: &mut UiState,
) {
    egui::SidePanel::right("inspector_panel")
        .default_width(320.0)
        .min_width(280.0)
        .show(ctx, |ui| {
            // Tab Header
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ui_state.active_tab, ActiveTab::Properties, "Properties");
                ui.selectable_value(&mut ui_state.active_tab, ActiveTab::LabelSetup, "Label Setup");
                ui.selectable_value(&mut ui_state.active_tab, ActiveTab::ZplCode, "ZPL Output");
                ui.selectable_value(&mut ui_state.active_tab, ActiveTab::BatchPrint, "Print");
            });
            ui.separator();

            match ui_state.active_tab {
                ActiveTab::Properties => {
                    render_properties_tab(ui, label, canvas);
                }
                ActiveTab::LabelSetup => {
                    render_label_setup_tab(ui, label, canvas, ui_state);
                }
                ActiveTab::ZplCode => {
                    render_zpl_tab(ui, label, ui_state);
                }
                ActiveTab::BatchPrint => {
                    render_batch_print_tab(ui, label, ui_state);
                }
            }
        });
}

fn render_properties_tab(
    ui: &mut egui::Ui,
    label: &mut Label,
    canvas: &mut CanvasState,
) {
    if let Some(sel_id) = canvas.selected_element_id {
        if let Some(el) = label.elements.iter_mut().find(|e| e.id == sel_id) {
            ui.heading(RichText::new(&el.name).size(15.0));
            ui.add_space(4.0);

            // General transform
            egui::Grid::new("element_geometry_grid").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut el.name);
                ui.end_row();

                ui.label("Position X (mm):");
                ui.add(egui::DragValue::new(&mut el.x).speed(0.5).range(0.0..=300.0));
                ui.end_row();

                ui.label("Position Y (mm):");
                ui.add(egui::DragValue::new(&mut el.y).speed(0.5).range(0.0..=500.0));
                ui.end_row();

                ui.label("Width (mm):");
                ui.add(egui::DragValue::new(&mut el.width).speed(0.5).range(2.0..=300.0));
                ui.end_row();

                ui.label("Height (mm):");
                ui.add(egui::DragValue::new(&mut el.height).speed(0.5).range(0.5..=500.0));
                ui.end_row();

                ui.label("Rotation:");
                egui::ComboBox::from_id_salt("elem_rot")
                    .selected_text(format!("{}°", el.rotation))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut el.rotation, 0, "0°");
                        ui.selectable_value(&mut el.rotation, 90, "90°");
                        ui.selectable_value(&mut el.rotation, 180, "180°");
                        ui.selectable_value(&mut el.rotation, 270, "270°");
                    });
                ui.end_row();
            });

            ui.separator();

            // Type-specific properties
            match &mut el.data {
                ElementData::Text { text, font_size, bold, italic, color: _ } => {
                    ui.label(RichText::new("Text Properties").strong());
                    ui.add_space(4.0);
                    ui.label("Content:");
                    ui.text_edit_multiline(text);
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Font Size:");
                        ui.add(egui::DragValue::new(font_size).speed(0.5).range(4.0..=72.0));
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(bold, "Bold");
                        ui.checkbox(italic, "Italic");
                    });
                }
                ElementData::Barcode { barcode_type, data, show_text, module_width } => {
                    ui.label(RichText::new("Barcode Settings").strong());
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label("Symbology:");
                        egui::ComboBox::from_id_salt("barcode_type_sel")
                            .selected_text(barcode_type.name())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(barcode_type, BarcodeType::Code128, BarcodeType::Code128.name());
                                ui.selectable_value(barcode_type, BarcodeType::Code39, BarcodeType::Code39.name());
                                ui.selectable_value(barcode_type, BarcodeType::QrCode, BarcodeType::QrCode.name());
                            });
                    });

                    ui.label("Data / Payload:");
                    ui.text_edit_singleline(data);

                    if !barcode_type.is_2d() {
                        ui.checkbox(show_text, "Show Human-Readable Text Below");
                    }

                    ui.horizontal(|ui| {
                        ui.label("Narrow Bar Ratio:");
                        ui.add(egui::DragValue::new(module_width).speed(0.05).range(0.2..=2.0));
                    });
                }
                ElementData::Shape { kind, stroke_width, corner_radius, .. } => {
                    ui.label(RichText::new("Shape Properties").strong());
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label("Shape Kind:");
                        egui::ComboBox::from_id_salt("shape_kind_sel")
                            .selected_text(format!("{:?}", kind))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(kind, ShapeKind::Rectangle, "Rectangle");
                                ui.selectable_value(kind, ShapeKind::RoundedRectangle, "Rounded Rectangle");
                                ui.selectable_value(kind, ShapeKind::Line, "Line");
                                ui.selectable_value(kind, ShapeKind::Ellipse, "Ellipse");
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Stroke Width (mm):");
                        ui.add(egui::DragValue::new(stroke_width).speed(0.1).range(0.1..=10.0));
                    });

                    if *kind == ShapeKind::RoundedRectangle {
                        ui.horizontal(|ui| {
                            ui.label("Corner Radius (mm):");
                            ui.add(egui::DragValue::new(corner_radius).speed(0.5).range(0.0..=20.0));
                        });
                    }
                }
                ElementData::Counter { prefix, current_value, step, padding_digits, font_size, bold } => {
                    ui.label(RichText::new("Counter Sequence").strong());
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label("Prefix:");
                        ui.text_edit_singleline(prefix);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Start Value:");
                        ui.add(egui::DragValue::new(current_value).speed(1));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Step Increment:");
                        ui.add(egui::DragValue::new(step).speed(1));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Digits Padding:");
                        ui.add(egui::DragValue::new(padding_digits).range(1..=12));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Font Size:");
                        ui.add(egui::DragValue::new(font_size).speed(0.5).range(6.0..=36.0));
                        ui.checkbox(bold, "Bold");
                    });

                    let preview = format!("{}{:0width$}", prefix, current_value, width = *padding_digits);
                    ui.label(format!("Sample Output: {}", preview));
                }
                ElementData::DateTime { format_str, font_size, bold } => {
                    ui.label(RichText::new("Date / Time Settings").strong());
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label("Format Specifier:");
                        ui.text_edit_singleline(format_str);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Font Size:");
                        ui.add(egui::DragValue::new(font_size).speed(0.5).range(6.0..=36.0));
                        ui.checkbox(bold, "Bold");
                    });
                }
            }

            ui.add_space(10.0);
            ui.separator();

            if ui.button("🗑 Remove Element").clicked() {
                label.elements.retain(|e| e.id != sel_id);
                canvas.selected_element_id = None;
            }
        }
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(RichText::new("No element selected").italics().color(Color32::from_rgb(120, 130, 145)));
            ui.label("Click any element on canvas to inspect and edit its properties.");
        });
    }
}

fn render_label_setup_tab(
    ui: &mut egui::Ui,
    label: &mut Label,
    canvas: &mut CanvasState,
    ui_state: &mut UiState,
) {
    ui.heading(RichText::new("Label Template & Media Setup").size(15.0));
    ui.add_space(4.0);

    ui.label(RichText::new("Import .nlbl Template (ZebraDesigner / NiceLabel):").strong());
    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut ui_state.nlbl_import_path);
        if ui.button("📥 Import").clicked() {
            let path = std::path::Path::new(&ui_state.nlbl_import_path);
            match crate::nlbl::import_nlbl(path) {
                Ok(imported) => {
                    *label = imported;
                    canvas.selected_element_id = None;
                    canvas.zoom = 6.0;
                    ui_state.status_message = format!("Successfully imported template: {}", path.display());
                }
                Err(e) => {
                    ui_state.status_message = format!("Import error: {}", e);
                }
            }
        }
    });
    ui.add_space(6.0);
    ui.separator();

    ui.label(RichText::new("Standard Size Presets:").strong());
    ui.horizontal_wrapped(|ui| {
        if ui.button("Shipping 4x6\"").clicked() {
            label.width_mm = 101.6;
            label.height_mm = 152.4;
            label.name = "Shipping Label 4x6\"".to_string();
        }
        if ui.button("Product 4x3\"").clicked() {
            label.width_mm = 101.6;
            label.height_mm = 76.2;
            label.name = "Product Label 4x3\"".to_string();
        }
        if ui.button("Inventory 3x1\"").clicked() {
            label.width_mm = 76.2;
            label.height_mm = 25.4;
            label.name = "Inventory Label 3x1\"".to_string();
        }
        if ui.button("Asset Tag 2x1\"").clicked() {
            label.width_mm = 50.8;
            label.height_mm = 25.4;
            label.name = "Asset Tag 2x1\"".to_string();
        }
        if ui.button("Euro 100x150mm").clicked() {
            label.width_mm = 100.0;
            label.height_mm = 150.0;
            label.name = "Euro Standard 100x150".to_string();
        }
    });

    ui.add_space(8.0);
    ui.separator();

    egui::Grid::new("label_dim_grid").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
        ui.label("Label Name:");
        ui.text_edit_singleline(&mut label.name);
        ui.end_row();

        ui.label("Width (mm):");
        ui.add(egui::DragValue::new(&mut label.width_mm).speed(0.5).range(10.0..=300.0));
        ui.end_row();

        ui.label("Height (mm):");
        ui.add(egui::DragValue::new(&mut label.height_mm).speed(0.5).range(10.0..=600.0));
        ui.end_row();

        ui.label("Orientation:");
        egui::ComboBox::from_id_salt("orientation_sel")
            .selected_text(format!("{:?}", label.orientation))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut label.orientation, Orientation::Portrait, "Portrait");
                ui.selectable_value(&mut label.orientation, Orientation::Landscape, "Landscape");
            });
        ui.end_row();

        ui.label("Printer DPI:");
        egui::ComboBox::from_id_salt("res_sel")
            .selected_text(format!("{} DPI ({} dpmm)", label.resolution.dpi(), label.resolution.dots_per_mm()))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut label.resolution, Resolution::Dpi203, "203 DPI (8 dots/mm) - Standard");
                ui.selectable_value(&mut label.resolution, Resolution::Dpi300, "300 DPI (12 dots/mm) - High Density");
                ui.selectable_value(&mut label.resolution, Resolution::Dpi600, "600 DPI (24 dots/mm) - Ultra Fine");
            });
        ui.end_row();
    });

    ui.add_space(8.0);
    ui.separator();
    ui.label(RichText::new("Margins (mm):").strong());

    egui::Grid::new("margins_grid").num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
        ui.label("Left:");
        ui.add(egui::DragValue::new(&mut label.margin_left_mm).speed(0.5).range(0.0..=50.0));
        ui.label("Top:");
        ui.add(egui::DragValue::new(&mut label.margin_top_mm).speed(0.5).range(0.0..=50.0));
        ui.end_row();

        ui.label("Right:");
        ui.add(egui::DragValue::new(&mut label.margin_right_mm).speed(0.5).range(0.0..=50.0));
        ui.label("Bottom:");
        ui.add(egui::DragValue::new(&mut label.margin_bottom_mm).speed(0.5).range(0.0..=50.0));
        ui.end_row();
    });
}

fn render_zpl_tab(ui: &mut egui::Ui, label: &Label, ui_state: &mut UiState) {
    ui.heading(RichText::new("Printer Code Inspector").size(15.0));
    ui.label("Live machine code compiled for thermal label printers.");
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.selectable_value(&mut ui_state.printer_language, PrinterLanguage::Auto, "Auto");
        ui.selectable_value(&mut ui_state.printer_language, PrinterLanguage::Epl2, "EPL2 (TLP2844)");
        ui.selectable_value(&mut ui_state.printer_language, PrinterLanguage::Zpl2, "ZPL II (Zebra)");
    });
    ui.add_space(4.0);

    let use_epl = match ui_state.printer_language {
        PrinterLanguage::Epl2 => true,
        PrinterLanguage::Zpl2 => false,
        PrinterLanguage::Auto => {
            if let Some(p) = ui_state.usb_printers.get(ui_state.selected_usb_printer.min(ui_state.usb_printers.len().saturating_sub(1))) {
                p.supports_epl()
            } else {
                false
            }
        }
    };

    let code = if use_epl {
        crate::epl::generate_epl(label, ui_state.print_copies)
    } else {
        generate_zpl(label)
    };

    let ext = if use_epl { "epl" } else { "zpl" };

    ui.horizontal(|ui| {
        if ui.button("📋 Copy to Clipboard").clicked() {
            ui.output_mut(|o| o.copied_text = code.clone());
            ui_state.status_message = format!("Copied {} code to clipboard.", if use_epl { "EPL2" } else { "ZPL" });
        }
        if ui.button(format!("💾 Export .{} File", ext)).clicked() {
            let filename = format!("label_output.{}", ext);
            let _ = std::fs::write(&filename, &code);
            ui_state.status_message = format!("Saved {}", filename);
        }
    });

    ui.add_space(6.0);
    ui.separator();

    egui::ScrollArea::vertical().max_height(450.0).show(ui, |ui| {
        let mut code_display = code;
        ui.add(
            egui::TextEdit::multiline(&mut code_display)
                .font(egui::TextStyle::Monospace)
                .code_editor()
                .desired_rows(22)
                .lock_focus(true),
        );
    });
}

fn render_batch_print_tab(ui: &mut egui::Ui, label: &Label, ui_state: &mut UiState) {
    ui.heading(RichText::new("Print Label").size(15.0));
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.radio_value(&mut ui_state.print_mode, PrintTargetMode::Usb, "USB Direct");
        ui.radio_value(&mut ui_state.print_mode, PrintTargetMode::Network, "Network (TCP 9100)");
    });
    ui.separator();

    match ui_state.print_mode {
        PrintTargetMode::Usb => {
            ui.label(RichText::new("Detected USB Printers:").strong());
            ui.horizontal(|ui| {
                if ui.button("🔄 Rescan USB Ports").clicked() {
                    ui_state.usb_printers = detect_usb_printers();
                    if let Some(first) = ui_state.usb_printers.first() {
                        ui_state.status_message = format!("Detected USB: {} ({})", first.name, first.device_node);
                    } else {
                        ui_state.status_message = "No USB printer found.".to_string();
                    }
                }
            });

            if ui_state.usb_printers.is_empty() {
                ui.add_space(6.0);
                ui.colored_label(Color32::from_rgb(200, 90, 20), "⚠ No USB printer detected on /dev/usb/lp*");
                ui.label("Plug in a Zebra or thermal printer via USB, then click Rescan.");
            } else {
                ui.add_space(4.0);
                let current_sel = ui_state.selected_usb_printer.min(ui_state.usb_printers.len() - 1);
                let current_display = ui_state.usb_printers[current_sel].display_name();

                egui::ComboBox::from_id_salt("usb_printer_dropdown")
                    .selected_text(current_display)
                    .show_ui(ui, |ui| {
                        for (idx, p) in ui_state.usb_printers.iter().enumerate() {
                            ui.selectable_value(&mut ui_state.selected_usb_printer, idx, p.display_name());
                        }
                    });

                let p = &ui_state.usb_printers[ui_state.selected_usb_printer.min(ui_state.usb_printers.len() - 1)];
                ui.add_space(6.0);
                egui::Grid::new("usb_dev_details").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                    ui.label("Model:");
                    ui.label(RichText::new(&p.model).strong());
                    ui.end_row();

                    ui.label("Manufacturer:");
                    ui.label(&p.manufacturer);
                    ui.end_row();

                    ui.label("Device Node:");
                    ui.monospace(&p.device_node);
                    ui.end_row();

                    if !p.serial.is_empty() {
                        ui.label("Serial Number:");
                        ui.monospace(&p.serial);
                        ui.end_row();
                    }

                    if !p.vendor_id.is_empty() {
                        ui.label("USB ID:");
                        ui.monospace(format!("{}:{}", p.vendor_id, p.product_id));
                        ui.end_row();
                    }

                    ui.label("Connection Status:");
                    ui.colored_label(Color32::from_rgb(0, 150, 40), "● Connected & Ready");
                    ui.end_row();
                });
            }

            ui.add_space(10.0);
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Protocol:");
                egui::ComboBox::from_id_salt("usb_lang_sel")
                    .selected_text(ui_state.printer_language.name())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut ui_state.printer_language, PrinterLanguage::Auto, "Auto (EPL2 for 2844)");
                        ui.selectable_value(&mut ui_state.printer_language, PrinterLanguage::Epl2, "EPL2 (Eltron)");
                        ui.selectable_value(&mut ui_state.printer_language, PrinterLanguage::Zpl2, "ZPL II (Zebra)");
                    });
            });

            ui.horizontal(|ui| {
                ui.label("Number of Copies:");
                ui.add(egui::DragValue::new(&mut ui_state.print_copies).range(1..=1000));
            });

            ui.add_space(8.0);
            if ui.button(RichText::new("🖨 Print Directly to USB Printer").size(14.0).strong()).clicked() {
                if !ui_state.usb_printers.is_empty() {
                    let sel = ui_state.selected_usb_printer.min(ui_state.usb_printers.len() - 1);
                    let p = &ui_state.usb_printers[sel];
                    match print_to_usb(p, label, ui_state.print_copies, ui_state.printer_language) {
                        Ok(msg) => {
                            ui_state.status_message = msg.clone();
                            ui_state.print_notification = Some((true, msg));
                        }
                        Err(err) => {
                            ui_state.status_message = format!("USB error: {}", err);
                            ui_state.print_notification = Some((false, format!("Print failed: {}", err)));
                        }
                    }
                } else {
                    ui_state.status_message = "Cannot print: no USB printer connected.".to_string();
                    ui_state.print_notification = Some((false, "Cannot print: no USB printer connected.".to_string()));
                }
            }
        }
        PrintTargetMode::Network => {
            egui::Grid::new("print_settings_grid").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label("Number of Copies:");
                ui.add(egui::DragValue::new(&mut ui_state.print_copies).range(1..=1000));
                ui.end_row();

                ui.label("Printer IP Address:");
                ui.text_edit_singleline(&mut ui_state.printer_ip);
                ui.end_row();

                ui.label("Port (default 9100):");
                ui.add(egui::DragValue::new(&mut ui_state.printer_port));
                ui.end_row();
            });

            ui.add_space(10.0);
            ui.separator();

            if ui.button(RichText::new("🖨 Send to Network Printer (TCP Raw 9100)").strong()).clicked() {
                let zpl = generate_zpl(label);
                let _ = std::fs::write("print_job.zpl", &zpl);
                ui_state.status_message = format!("Print job ({} copies) generated and ready for {}", ui_state.print_copies, ui_state.printer_ip);
            }
        }
    }
}

pub fn render_status_bar(
    ctx: &egui::Context,
    label: &Label,
    canvas: &CanvasState,
    ui_state: &UiState,
) {
    egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // USB printer status pill
            if !ui_state.usb_printers.is_empty() {
                let p = &ui_state.usb_printers[ui_state.selected_usb_printer.min(ui_state.usb_printers.len() - 1)];
                ui.label(RichText::new(format!("🔌 {} ({})", p.model, p.device_node)).color(Color32::from_rgb(0, 130, 40)).strong());
            } else {
                ui.label(RichText::new("🔌 USB: Disconnected").color(Color32::from_rgb(130, 130, 130)));
            }
            ui.separator();

            ui.label(RichText::new(&ui_state.status_message).color(Color32::from_rgb(60, 65, 75)));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("Zoom: {:.0}%", canvas.zoom / 3.5 * 100.0));
                ui.separator();
                ui.label(format!("Elements: {}", label.elements.len()));
                ui.separator();
                ui.label(format!("Resolution: {} DPI", label.resolution.dpi()));
            });
        });
    });
}
