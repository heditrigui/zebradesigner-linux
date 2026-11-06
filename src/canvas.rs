use crate::barcode::{bar_dots, bar_height_dots, generate_code128, generate_code39, generate_qr_matrix, qr_scale};
use crate::model::{along_across, counter_text, format_datetime, BarcodeType, ElementData, Label, ShapeKind};
use eframe::egui::{self, epaint::TextShape, Color32, Pos2, Rect, Shape, Stroke, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragMode {
    None,
    MoveElement(u64),
    ResizeHandle(u64, ResizeCorner),
    PanCanvas,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Top,
    Bottom,
    Left,
    Right,
}

/// Maps a position in an element's reading direction (u along, v across, in
/// points) to the screen, starting from the corner the printer starts from.
fn local_to_screen(rect: Rect, rotation: i32, u: f32, v: f32) -> Pos2 {
    match rotation {
        90 => Pos2::new(rect.max.x - v, rect.min.y + u),
        180 => Pos2::new(rect.max.x - u, rect.max.y - v),
        270 => Pos2::new(rect.min.x + v, rect.max.y - u),
        _ => Pos2::new(rect.min.x + u, rect.min.y + v),
    }
}

fn paint_text(painter: &egui::Painter, rect: Rect, rotation: i32, text: &str, font: egui::FontId, color: Color32, bold: bool) {
    let galley = painter.layout_no_wrap(text.replace('\n', " "), font, color);
    let pos = local_to_screen(rect, rotation, 0.0, 0.0);
    let angle = (rotation as f32).to_radians();
    painter.add(TextShape::new(pos, galley.clone(), color).with_angle(angle));
    if bold {
        // The bundled fonts have no bold face, so match the printer's double strike
        let shifted = pos + Vec2::new(0.7, 0.0);
        painter.add(TextShape::new(shifted, galley, color).with_angle(angle));
    }
}

pub struct CanvasState {
    pub zoom: f32, // scale factor, default ~3.5 px per mm
    pub pan: Vec2,
    pub selected_element_id: Option<u64>,
    pub drag_mode: DragMode,
    pub drag_start_mouse: Pos2,
    pub drag_start_elem_rect: Rect, // x, y, width, height in mm
    pub show_grid: bool,
    pub snap_to_grid: bool,
    pub grid_step_mm: f32,
    pub show_rulers: bool,
}

impl Default for CanvasState {
    fn default() -> Self {
        CanvasState {
            zoom: 3.5, // 100 mm label takes ~350 pixels
            pan: Vec2::new(60.0, 40.0),
            selected_element_id: Some(14), // Select the primary barcode initially
            drag_mode: DragMode::None,
            drag_start_mouse: Pos2::ZERO,
            drag_start_elem_rect: Rect::ZERO,
            show_grid: true,
            snap_to_grid: true,
            grid_step_mm: 5.0,
            show_rulers: true,
        }
    }
}

impl CanvasState {
    pub fn mm_to_screen(&self, canvas_origin: Pos2, mm_x: f32, mm_y: f32) -> Pos2 {
        Pos2::new(
            canvas_origin.x + self.pan.x + mm_x * self.zoom,
            canvas_origin.y + self.pan.y + mm_y * self.zoom,
        )
    }

    pub fn screen_to_mm(&self, canvas_origin: Pos2, screen_pos: Pos2) -> (f32, f32) {
        let x = (screen_pos.x - canvas_origin.x - self.pan.x) / self.zoom;
        let y = (screen_pos.y - canvas_origin.y - self.pan.y) / self.zoom;
        (x, y)
    }

    pub fn draw_canvas(
        &mut self,
        ui: &mut egui::Ui,
        label: &mut Label,
    ) {
        let (response, painter) = ui.allocate_painter(
            ui.available_size_before_wrap(),
            egui::Sense::click_and_drag(),
        );

        let canvas_rect = response.rect;
        let canvas_origin = canvas_rect.min;

        // Clip painter to canvas bounds
        let painter = painter.with_clip_rect(canvas_rect);

        // Canvas backdrop fill (engineering blueprint / dark slate workbench)
        painter.rect_filled(canvas_rect, 0.0, Color32::from_rgb(238, 240, 244));

        // Pan and Zoom interaction
        let pointer_pos = ui.input(|i| i.pointer.hover_pos().unwrap_or(Pos2::ZERO));
        let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
        if response.hovered() && scroll_delta != 0.0 {
            let old_zoom = self.zoom;
            let zoom_factor = if scroll_delta > 0.0 { 1.1 } else { 0.9 };
            self.zoom = (self.zoom * zoom_factor).clamp(1.0, 12.0);

            // Zoom centered on pointer
            let mouse_offset = pointer_pos - (canvas_origin + self.pan);
            let scale_change = self.zoom / old_zoom;
            self.pan = pointer_pos - canvas_origin - mouse_offset * scale_change;
        }

        // Calculate Label screen bounding box
        let label_tl = self.mm_to_screen(canvas_origin, 0.0, 0.0);
        let label_br = self.mm_to_screen(canvas_origin, label.width_mm, label.height_mm);
        let label_rect = Rect::from_two_pos(label_tl, label_br);

        // 1. Label shadow (gives depth and tactile feel of physical label sheet)
        let shadow_rect = label_rect.translate(Vec2::new(4.0, 4.0));
        painter.rect_filled(shadow_rect, 4.0, Color32::from_black_alpha(35));

        // 2. Physical Label paper body (pristine pure white)
        painter.rect_filled(label_rect, 3.0, Color32::WHITE);
        painter.rect_stroke(label_rect, 3.0, Stroke::new(1.0_f32, Color32::from_rgb(180, 185, 195)));

        // 3. Grid dots/lines if enabled
        if self.show_grid {
            let step = self.grid_step_mm * self.zoom;
            let mut gx = label_rect.min.x + step;
            while gx < label_rect.max.x {
                let mut gy = label_rect.min.y + step;
                while gy < label_rect.max.y {
                    painter.circle_filled(Pos2::new(gx, gy), 1.0, Color32::from_rgb(215, 220, 228));
                    gy += step;
                }
                gx += step;
            }
        }

        // 4. Margins guide line (dashed guide inside label)
        let m_tl = self.mm_to_screen(canvas_origin, label.margin_left_mm, label.margin_top_mm);
        let m_br = self.mm_to_screen(
            canvas_origin,
            label.width_mm - label.margin_right_mm,
            label.height_mm - label.margin_bottom_mm,
        );
        let margin_rect = Rect::from_two_pos(m_tl, m_br);
        painter.rect_stroke(
            margin_rect,
            1.0,
            Stroke::new(0.8_f32, Color32::from_rgba_unmultiplied(220, 80, 80, 140)),
        );

        // 5. Render Label Elements
        let mut clicked_element_id = None;

        for el in &label.elements {
            let el_tl = self.mm_to_screen(canvas_origin, el.x, el.y);
            let el_br = self.mm_to_screen(canvas_origin, el.x + el.width, el.y + el.height);
            let el_rect = Rect::from_two_pos(el_tl, el_br);

            // Element rendering
            match &el.data {
                ElementData::Shape { kind, stroke_width, stroke_color, fill_color, corner_radius } => {
                    let s_color = Color32::from_rgba_premultiplied(
                        stroke_color[0], stroke_color[1], stroke_color[2], stroke_color[3],
                    );
                    let s_width = (*stroke_width * self.zoom).max(0.8);
                    let stroke = Stroke::new(s_width, s_color);

                    let f_color = fill_color.map(|c| {
                        Color32::from_rgba_premultiplied(c[0], c[1], c[2], c[3])
                    }).unwrap_or(Color32::TRANSPARENT);

                    let c_rad = *corner_radius * self.zoom;

                    match kind {
                        ShapeKind::Rectangle => {
                            painter.rect(el_rect, 0.0, f_color, stroke);
                        }
                        ShapeKind::RoundedRectangle => {
                            painter.rect(el_rect, c_rad, f_color, stroke);
                        }
                        ShapeKind::Line => {
                            let segment = if el.rotation == 90 || el.rotation == 270 {
                                [Pos2::new(el_rect.center().x, el_rect.min.y),
                                 Pos2::new(el_rect.center().x, el_rect.max.y)]
                            } else {
                                [Pos2::new(el_rect.min.x, el_rect.center().y),
                                 Pos2::new(el_rect.max.x, el_rect.center().y)]
                            };
                            painter.line_segment(segment, stroke);
                        }
                        ShapeKind::Ellipse => {
                            let radius = el_rect.size() / 2.0;
                            painter.add(Shape::ellipse_filled(el_rect.center(), radius, f_color));
                            painter.add(Shape::ellipse_stroke(el_rect.center(), radius, stroke));
                        }
                    }
                }
                ElementData::Text { text, font_size, bold, italic: _, color } => {
                    let t_color = Color32::from_rgba_premultiplied(color[0], color[1], color[2], color[3]);
                    let scale_size = (*font_size * self.zoom * 0.35).max(8.0);
                    paint_text(&painter, el_rect, el.rotation, text, egui::FontId::proportional(scale_size), t_color, *bold);
                }
                ElementData::Barcode { barcode_type, data, show_text, .. } => {
                    let dpmm = label.resolution.dots_per_mm();
                    let w_dots = (el.width * dpmm).round().max(1.0) as u32;
                    let h_dots = (el.height * dpmm).round().max(1.0) as u32;
                    let dot = self.zoom / dpmm;

                    if barcode_type.is_2d() {
                        // Render QR Code at the size it prints
                        if let Some((grid_w, colors)) = generate_qr_matrix(data) {
                            let mod_size = qr_scale(data, w_dots.min(h_dots)) as f32 * dot;
                            for row in 0..grid_w {
                                for col in 0..grid_w {
                                    if colors[row * grid_w + col] {
                                        let b_x = el_rect.min.x + col as f32 * mod_size;
                                        let b_y = el_rect.min.y + row as f32 * mod_size;
                                        painter.rect_filled(
                                            Rect::from_min_size(Pos2::new(b_x, b_y), Vec2::splat(mod_size + 0.3)),
                                            0.0,
                                            Color32::BLACK,
                                        );
                                    }
                                }
                            }
                        }
                    } else {
                        // Render 1D Barcode (Code 128 / Code 39) at the size it prints
                        let pattern = match barcode_type {
                            BarcodeType::Code39 => generate_code39(data),
                            _ => generate_code128(data),
                        };

                        let (along, across) = along_across(el.rotation, w_dots, h_dots);
                        let mod_w = bar_dots(pattern.modules.len(), along) as f32 * dot;
                        let bar_h = bar_height_dots(across, *show_text) as f32 * dot;

                        for (idx, &is_bar) in pattern.modules.iter().enumerate() {
                            if is_bar {
                                let u = idx as f32 * mod_w;
                                painter.rect_filled(
                                    Rect::from_two_pos(
                                        local_to_screen(el_rect, el.rotation, u, 0.0),
                                        local_to_screen(el_rect, el.rotation, u + mod_w.max(1.0), bar_h),
                                    ),
                                    0.0,
                                    Color32::BLACK,
                                );
                            }
                        }

                        if *show_text {
                            let font = egui::FontId::monospace((self.zoom * 2.8).max(9.0));
                            let galley = painter.layout_no_wrap(data.clone(), font, Color32::BLACK);
                            let code_len = pattern.modules.len() as f32 * mod_w;
                            let u = ((code_len - galley.size().x) / 2.0).max(0.0);
                            let pos = local_to_screen(el_rect, el.rotation, u, bar_h + 2.0);
                            let angle = (el.rotation as f32).to_radians();
                            painter.add(TextShape::new(pos, galley, Color32::BLACK).with_angle(angle));
                        }
                    }
                }
                ElementData::Counter { prefix, current_value, padding_digits, font_size, bold, .. } => {
                    let formatted = counter_text(prefix, *current_value, *padding_digits);
                    let scale_size = (*font_size * self.zoom * 0.35).max(8.0);
                    paint_text(&painter, el_rect, el.rotation, &formatted, egui::FontId::monospace(scale_size), Color32::BLACK, *bold);
                }
                ElementData::DateTime { format_str, font_size, bold } => {
                    let date_str = format_datetime(format_str);
                    let scale_size = (*font_size * self.zoom * 0.35).max(8.0);
                    paint_text(&painter, el_rect, el.rotation, &date_str, egui::FontId::proportional(scale_size), Color32::from_rgb(40, 40, 40), *bold);
                }
            }

            // Selection indicator & handles
            if Some(el.id) == self.selected_element_id {
                // Highlight box
                painter.rect_stroke(
                    el_rect.expand(2.0),
                    2.0,
                    Stroke::new(1.5_f32, Color32::from_rgb(0, 120, 215)),
                );

                // 8 Resize handles
                let handle_size = 6.0;
                let handles = [
                    (el_rect.min, ResizeCorner::TopLeft),
                    (Pos2::new(el_rect.center().x, el_rect.min.y), ResizeCorner::Top),
                    (Pos2::new(el_rect.max.x, el_rect.min.y), ResizeCorner::TopRight),
                    (Pos2::new(el_rect.max.x, el_rect.center().y), ResizeCorner::Right),
                    (el_rect.max, ResizeCorner::BottomRight),
                    (Pos2::new(el_rect.center().x, el_rect.max.y), ResizeCorner::Bottom),
                    (Pos2::new(el_rect.min.x, el_rect.max.y), ResizeCorner::BottomLeft),
                    (Pos2::new(el_rect.min.x, el_rect.center().y), ResizeCorner::Left),
                ];

                for (pt, _) in &handles {
                    let h_rect = Rect::from_center_size(*pt, Vec2::splat(handle_size));
                    painter.rect_filled(h_rect, 1.0, Color32::WHITE);
                    painter.rect_stroke(h_rect, 1.0, Stroke::new(1.0_f32, Color32::from_rgb(0, 120, 215)));
                }
            }

            // Check element click
            if response.clicked() && el_rect.contains(pointer_pos) {
                clicked_element_id = Some(el.id);
            }
        }

        // Handle Click Selection
        if response.clicked() {
            if let Some(id) = clicked_element_id {
                self.selected_element_id = Some(id);
            } else if label_rect.contains(pointer_pos) {
                // Clicked canvas background
                self.selected_element_id = None;
            }
        }

        // Handle Drag / Move / Resize
        if response.drag_started() {
            self.drag_start_mouse = pointer_pos;
            if let Some(sel_id) = self.selected_element_id {
                if let Some(el) = label.elements.iter().find(|e| e.id == sel_id) {
                    let el_tl = self.mm_to_screen(canvas_origin, el.x, el.y);
                    let el_br = self.mm_to_screen(canvas_origin, el.x + el.width, el.y + el.height);
                    let el_rect = Rect::from_two_pos(el_tl, el_br);

                    // Check if dragging a handle
                    let handle_hit_size = 10.0;
                    let mut hit_handle = None;
                    let handles = [
                        (el_rect.min, ResizeCorner::TopLeft),
                        (Pos2::new(el_rect.center().x, el_rect.min.y), ResizeCorner::Top),
                        (Pos2::new(el_rect.max.x, el_rect.min.y), ResizeCorner::TopRight),
                        (Pos2::new(el_rect.max.x, el_rect.center().y), ResizeCorner::Right),
                        (el_rect.max, ResizeCorner::BottomRight),
                        (Pos2::new(el_rect.center().x, el_rect.max.y), ResizeCorner::Bottom),
                        (Pos2::new(el_rect.min.x, el_rect.max.y), ResizeCorner::BottomLeft),
                        (Pos2::new(el_rect.min.x, el_rect.center().y), ResizeCorner::Left),
                    ];
                    for (pt, corner) in handles {
                        if (pointer_pos - pt).length() <= handle_hit_size {
                            hit_handle = Some(corner);
                            break;
                        }
                    }

                    if let Some(corner) = hit_handle {
                        self.drag_mode = DragMode::ResizeHandle(sel_id, corner);
                        self.drag_start_elem_rect = Rect::from_min_size(Pos2::new(el.x, el.y), Vec2::new(el.width, el.height));
                    } else if el_rect.contains(pointer_pos) {
                        self.drag_mode = DragMode::MoveElement(sel_id);
                        self.drag_start_elem_rect = Rect::from_min_size(Pos2::new(el.x, el.y), Vec2::new(el.width, el.height));
                    } else {
                        self.drag_mode = DragMode::PanCanvas;
                    }
                }
            } else {
                self.drag_mode = DragMode::PanCanvas;
            }
        }

        if response.dragged() {
            let delta = response.drag_delta();
            match self.drag_mode {
                DragMode::PanCanvas => {
                    self.pan += delta;
                }
                DragMode::MoveElement(id) => {
                    let total_dx_mm = (pointer_pos.x - self.drag_start_mouse.x) / self.zoom;
                    let total_dy_mm = (pointer_pos.y - self.drag_start_mouse.y) / self.zoom;
                    if let Some(el) = label.elements.iter_mut().find(|e| e.id == id) {
                        let mut new_x = self.drag_start_elem_rect.min.x + total_dx_mm;
                        let mut new_y = self.drag_start_elem_rect.min.y + total_dy_mm;
                        if self.snap_to_grid {
                            new_x = (new_x / self.grid_step_mm).round() * self.grid_step_mm;
                            new_y = (new_y / self.grid_step_mm).round() * self.grid_step_mm;
                        }
                        el.x = new_x.max(0.0);
                        el.y = new_y.max(0.0);
                    }
                }
                DragMode::ResizeHandle(id, corner) => {
                    let total_dx_mm = (pointer_pos.x - self.drag_start_mouse.x) / self.zoom;
                    let total_dy_mm = (pointer_pos.y - self.drag_start_mouse.y) / self.zoom;
                    if let Some(el) = label.elements.iter_mut().find(|e| e.id == id) {
                        let init = self.drag_start_elem_rect;
                        match corner {
                            ResizeCorner::BottomRight => {
                                el.width = (init.width() + total_dx_mm).max(4.0);
                                el.height = (init.height() + total_dy_mm).max(4.0);
                            }
                            ResizeCorner::Right => {
                                el.width = (init.width() + total_dx_mm).max(4.0);
                            }
                            ResizeCorner::Bottom => {
                                el.height = (init.height() + total_dy_mm).max(4.0);
                            }
                            _ => {}
                        }
                    }
                }
                DragMode::None => {}
            }
        }

        if response.drag_stopped() {
            self.drag_mode = DragMode::None;
        }

        // 6. Rulers (Top and Left)
        if self.show_rulers {
            let ruler_thickness = 18.0;
            let ruler_color = Color32::from_rgb(245, 246, 248);
            let ruler_border = Color32::from_rgb(205, 210, 218);
            let tick_color = Color32::from_rgb(120, 125, 135);

            // Top Ruler
            let top_ruler_rect = Rect::from_min_size(
                Pos2::new(canvas_rect.min.x + ruler_thickness, canvas_rect.min.y),
                Vec2::new(canvas_rect.width() - ruler_thickness, ruler_thickness),
            );
            painter.rect_filled(top_ruler_rect, 0.0, ruler_color);
            painter.line_segment(
                [Pos2::new(top_ruler_rect.min.x, top_ruler_rect.max.y), top_ruler_rect.max],
                Stroke::new(1.0_f32, ruler_border),
            );

            // Left Ruler
            let left_ruler_rect = Rect::from_min_size(
                Pos2::new(canvas_rect.min.x, canvas_rect.min.y + ruler_thickness),
                Vec2::new(ruler_thickness, canvas_rect.height() - ruler_thickness),
            );
            painter.rect_filled(left_ruler_rect, 0.0, ruler_color);
            painter.line_segment(
                [Pos2::new(left_ruler_rect.max.x, left_ruler_rect.min.y), left_ruler_rect.max],
                Stroke::new(1.0_f32, ruler_border),
            );

            // Top-left Corner Square
            painter.rect_filled(
                Rect::from_min_size(canvas_rect.min, Vec2::splat(ruler_thickness)),
                0.0,
                Color32::from_rgb(230, 233, 238),
            );
            painter.text(
                canvas_rect.min + Vec2::new(2.0, 2.0),
                egui::Align2::LEFT_TOP,
                "mm",
                egui::FontId::proportional(10.0),
                Color32::from_rgb(90, 95, 105),
            );

            // Top Ruler Ticks (every 10mm and 50mm)
            let mut mm_x = 0.0;
            while mm_x <= label.width_mm + 20.0 {
                let sx = self.mm_to_screen(canvas_origin, mm_x, 0.0).x;
                if sx >= top_ruler_rect.min.x && sx <= top_ruler_rect.max.x {
                    let is_major = (mm_x as i32) % 50 == 0;
                    let is_mid = (mm_x as i32) % 10 == 0;
                    let tick_len = if is_major { 10.0 } else if is_mid { 6.0 } else { 3.0 };
                    painter.line_segment(
                        [Pos2::new(sx, top_ruler_rect.max.y - tick_len), Pos2::new(sx, top_ruler_rect.max.y)],
                        Stroke::new(1.0_f32, tick_color),
                    );
                    if is_major || (is_mid && self.zoom > 3.0) {
                        painter.text(
                            Pos2::new(sx + 2.0, top_ruler_rect.min.y + 2.0),
                            egui::Align2::LEFT_TOP,
                            format!("{}", mm_x as i32),
                            egui::FontId::proportional(9.0),
                            tick_color,
                        );
                    }
                }
                mm_x += 10.0;
            }

            // Left Ruler Ticks
            let mut mm_y = 0.0;
            while mm_y <= label.height_mm + 20.0 {
                let sy = self.mm_to_screen(canvas_origin, 0.0, mm_y).y;
                if sy >= left_ruler_rect.min.y && sy <= left_ruler_rect.max.y {
                    let is_major = (mm_y as i32) % 50 == 0;
                    let is_mid = (mm_y as i32) % 10 == 0;
                    let tick_len = if is_major { 10.0 } else if is_mid { 6.0 } else { 3.0 };
                    painter.line_segment(
                        [Pos2::new(left_ruler_rect.max.x - tick_len, sy), Pos2::new(left_ruler_rect.max.x, sy)],
                        Stroke::new(1.0_f32, tick_color),
                    );
                    if is_major || (is_mid && self.zoom > 3.0) {
                        painter.text(
                            Pos2::new(left_ruler_rect.min.x + 1.0, sy + 1.0),
                            egui::Align2::LEFT_TOP,
                            format!("{}", mm_y as i32),
                            egui::FontId::proportional(8.0),
                            tick_color,
                        );
                    }
                }
                mm_y += 10.0;
            }

            // Cursor position indicator line on rulers
            if canvas_rect.contains(pointer_pos) {
                if pointer_pos.x >= top_ruler_rect.min.x {
                    painter.line_segment(
                        [Pos2::new(pointer_pos.x, top_ruler_rect.min.y), Pos2::new(pointer_pos.x, top_ruler_rect.max.y)],
                        Stroke::new(1.0_f32, Color32::from_rgb(0, 120, 215)),
                    );
                }
                if pointer_pos.y >= left_ruler_rect.min.y {
                    painter.line_segment(
                        [Pos2::new(left_ruler_rect.min.x, pointer_pos.y), Pos2::new(left_ruler_rect.max.x, pointer_pos.y)],
                        Stroke::new(1.0_f32, Color32::from_rgb(0, 120, 215)),
                    );
                }
            }
        }
    }
}
