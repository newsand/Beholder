use beholder_core::buffer::BufferManager;
use beholder_core::model::{AddMode, VramBoardSample, WindowConfig};
use beholder_core::nvml::NvmlWrapper;
use beholder_core::sampler::Sampler;
use beholder_core::targets::TargetManager;
use chrono::Local;
use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};
use std::time::{Duration, Instant};

const COLORS: &[(u8, u8, u8)] = &[
    (66, 133, 244),
    (234, 67, 53),
    (251, 188, 5),
    (52, 168, 83),
    (155, 89, 182),
    (230, 126, 34),
    (26, 188, 156),
    (241, 196, 15),
];

pub enum Message {
    AddTargetPid(u32),
    AddTargetName(String, AddMode),
    RemoveTarget(u32),
    SetInterval(u32),
    SetWindow(u32),
    ClearBuffer,
    ExportCsv,
    ExportJson,
    ToggleViewMode,
    Tick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Line,
    Gauge,
}

pub struct Model {
    targets: TargetManager,
    buffer: BufferManager,
    sampler: Sampler,
    nvml: Option<NvmlWrapper>,
    last_vram_board: Option<VramBoardSample>,
    config: WindowConfig,
    error_message: Option<String>,
    add_input: String,
    add_mode: AddMode,
    view_mode: ViewMode,
}

impl Model {
    fn new(nvml: Option<NvmlWrapper>) -> Self {
        Self {
            targets: TargetManager::new(),
            buffer: BufferManager::default(),
            sampler: Sampler::new(),
            nvml,
            last_vram_board: None,
            config: WindowConfig::default(),
            error_message: None,
            add_input: String::new(),
            add_mode: AddMode::Pid,
            view_mode: ViewMode::Line,
        }
    }

    fn update(&mut self, msg: Message) {
        self.error_message = None;

        match msg {
            Message::AddTargetPid(pid) => {
                if let Err(e) = self.targets.add_by_pid(pid) {
                    self.error_message = Some(e.to_string());
                }
            }
            Message::AddTargetName(name, mode) => {
                if let Err(e) = self.targets.add_by_name(&name, mode) {
                    self.error_message = Some(e.to_string());
                }
            }
            Message::RemoveTarget(pid) => {
                self.buffer.remove_target(pid);
                let _ = self.targets.remove(pid);
            }
            Message::SetInterval(ms) => {
                if let Some(new_config) = WindowConfig::new(self.config.window_secs, ms) {
                    self.config = new_config;
                    self.buffer.set_config(new_config);
                }
            }
            Message::SetWindow(secs) => {
                if let Some(new_config) = WindowConfig::new(secs, self.config.interval_ms) {
                    self.config = new_config;
                    self.buffer.set_config(new_config);
                }
            }
            Message::ClearBuffer => {
                self.buffer.clear();
            }
            Message::ExportCsv => {
                self.export("csv");
            }
            Message::ExportJson => {
                self.export("json");
            }
            Message::ToggleViewMode => {
                self.view_mode = match self.view_mode {
                    ViewMode::Line => ViewMode::Gauge,
                    ViewMode::Gauge => ViewMode::Line,
                };
            }
            Message::Tick => {
                self.do_sample();
            }
        }
    }

    fn do_sample(&mut self) {
        let (ram_samples, vram_samples) =
            self.sampler.sample_targets(&mut self.targets, self.nvml.as_ref());

        let track_history = self.view_mode == ViewMode::Line;
        for sample in ram_samples {
            self.buffer.push_ram(sample, track_history);
        }
        for sample in vram_samples {
            self.buffer.push_vram(sample, track_history);
        }

        if let Some(ref nvml) = self.nvml {
            if let Ok(board) = nvml.get_board_vram() {
                self.last_vram_board = Some(board);
            }
        }
    }

    fn export(&self, format: &str) {
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let default_name = format!("beholder_export_{}.{}", timestamp, format);

        let filter = if format == "csv" {
            rfd::FileDialog::new().add_filter("CSV", &["csv"])
        } else {
            rfd::FileDialog::new().add_filter("JSON", &["json"])
        };

        if let Some(path) = filter.set_file_name(&default_name).save_file() {
            let content = if format == "csv" {
                self.generate_csv()
            } else {
                self.generate_json()
            };

            if let Err(e) = std::fs::write(&path, content) {
                eprintln!("Export error: {}", e);
            }
        }
    }

    fn generate_csv(&self) -> String {
        let mut lines = vec!["ts_ms,pid,name,rss_bytes,cpu_pct,vram_bytes".to_string()];

        for target in self.targets.list() {
            let ram_points = self
                .buffer
                .get_ram_series(target.pid)
                .map(|s| s.get_points())
                .unwrap_or_default();

            let vram_points = self
                .buffer
                .get_vram_series(target.pid)
                .map(|s| s.get_points())
                .unwrap_or_default();

            let mut combined: Vec<(u64, Option<(u64, f32)>, Option<u64>)> = Vec::new();

            for (ts, rss, cpu) in &ram_points {
                combined.push((*ts, Some((*rss, *cpu)), None));
            }

            for (ts, vram) in &vram_points {
                if let Some(entry) = combined.iter_mut().find(|(t, _, _)| *t == *ts) {
                    entry.2 = Some(*vram);
                } else {
                    combined.push((*ts, None, Some(*vram)));
                }
            }

            combined.sort_by_key(|(ts, _, _)| *ts);

            for (ts, ram, vram) in combined {
                let (rss, cpu) = ram.unwrap_or((0, 0.0));
                let vram_str = vram.map(|v| v.to_string()).unwrap_or_default();
                lines.push(format!(
                    "{},{},{},{},{:.2},{}",
                    ts, target.pid, target.name, rss, cpu, vram_str
                ));
            }
        }

        lines.join("\n")
    }

    fn generate_json(&self) -> String {
        let mut data = Vec::new();

        for target in self.targets.list() {
            let ram_points = self
                .buffer
                .get_ram_series(target.pid)
                .map(|s| s.get_points())
                .unwrap_or_default();

            let vram_points = self
                .buffer
                .get_vram_series(target.pid)
                .map(|s| s.get_points())
                .unwrap_or_default();

            let ram_samples: Vec<serde_json::Value> = ram_points
                .into_iter()
                .map(|(ts, rss, cpu)| {
                    serde_json::json!({
                        "ts_ms": ts,
                        "rss_bytes": rss,
                        "cpu_pct": cpu
                    })
                })
                .collect();

            let vram_samples: Vec<serde_json::Value> = vram_points
                .into_iter()
                .map(|(ts, vram)| {
                    serde_json::json!({
                        "ts_ms": ts,
                        "used_bytes": vram
                    })
                })
                .collect();

            data.push(serde_json::json!({
                "pid": target.pid,
                "name": target.name,
                "ram_samples": ram_samples,
                "vram_samples": vram_samples
            }));
        }

        serde_json::to_string_pretty(&serde_json::json!({ "data": data })).unwrap_or_default()
    }
}

pub struct BeholderApp {
    model: Model,
    last_sample: Instant,
}

impl BeholderApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, nvml: Option<NvmlWrapper>) -> Self {
        Self {
            model: Model::new(nvml),
            last_sample: Instant::now(),
        }
    }

    fn color_for_index(idx: usize) -> egui::Color32 {
        let (r, g, b) = COLORS[idx % COLORS.len()];
        egui::Color32::from_rgb(r, g, b)
    }
}

impl eframe::App for BeholderApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let interval = Duration::from_millis(self.model.config.interval_ms as u64);
        if self.last_sample.elapsed() >= interval {
            self.model.update(Message::Tick);
            self.last_sample = Instant::now();
        }

        ctx.request_repaint_after(Duration::from_millis(50));

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Beholder");
                ui.separator();

                ui.label("Interval:");
                for &ms in &WindowConfig::VALID_INTERVALS {
                    if ui
                        .selectable_label(self.model.config.interval_ms == ms, format!("{}ms", ms))
                        .clicked()
                    {
                        self.model.update(Message::SetInterval(ms));
                    }
                }

                ui.separator();

                ui.label("Window:");
                for &secs in &WindowConfig::VALID_WINDOWS {
                    let label = if secs < 60 {
                        format!("{}s", secs)
                    } else if secs < 3600 {
                        format!("{}m", secs / 60)
                    } else {
                        format!("{}h", secs / 3600)
                    };
                    if ui
                        .selectable_label(self.model.config.window_secs == secs, label)
                        .clicked()
                    {
                        self.model.update(Message::SetWindow(secs));
                    }
                }

                ui.separator();

                if ui.button("Clear").clicked() {
                    self.model.update(Message::ClearBuffer);
                }

                ui.menu_button("Export", |ui| {
                    if ui.button("CSV").clicked() {
                        self.model.update(Message::ExportCsv);
                        ui.close_menu();
                    }
                    if ui.button("JSON").clicked() {
                        self.model.update(Message::ExportJson);
                        ui.close_menu();
                    }
                });

                ui.separator();

                let gauge_label = match self.model.view_mode {
                    ViewMode::Line => "📈 Line",
                    ViewMode::Gauge => "🔲 Gauge",
                };
                if ui.button(gauge_label).clicked() {
                    self.model.update(Message::ToggleViewMode);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let nvml_status = if self.model.nvml.is_some() {
                        egui::RichText::new("GPU: OK").color(egui::Color32::GREEN)
                    } else {
                        egui::RichText::new("GPU: N/A").color(egui::Color32::GRAY)
                    };
                    ui.label(nvml_status);
                });
            });
        });

        egui::SidePanel::left("targets_panel")
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading("Add Target");

                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.model.add_mode, AddMode::Pid, "PID");
                    ui.radio_value(&mut self.model.add_mode, AddMode::Exact, "Exact");
                    ui.radio_value(&mut self.model.add_mode, AddMode::Substring, "Substring");
                });

                ui.horizontal(|ui| {
                    let hint = match self.model.add_mode {
                        AddMode::Pid => "Enter PID...",
                        AddMode::Exact => "comm name...",
                        AddMode::Substring => "cmdline pattern...",
                    };
                    let te = egui::TextEdit::singleline(&mut self.model.add_input).hint_text(hint);
                    ui.add(te);

                    if ui.button("Add").clicked() {
                        let input = self.model.add_input.trim().to_string();
                        if !input.is_empty() {
                            match self.model.add_mode {
                                AddMode::Pid => {
                                    if let Ok(pid) = input.parse::<u32>() {
                                        self.model.update(Message::AddTargetPid(pid));
                                    } else {
                                        self.model.error_message = Some("Invalid PID".to_string());
                                    }
                                }
                                mode => {
                                    self.model.update(Message::AddTargetName(input, mode));
                                }
                            }
                            self.model.add_input.clear();
                        }
                    }
                });

                if let Some(ref err) = self.model.error_message {
                    ui.colored_label(egui::Color32::RED, err);
                }

                ui.separator();
                ui.heading("Targets");

                let targets: Vec<_> = self.model.targets.list().cloned().collect();
                let mut to_remove = None;

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (idx, target) in targets.iter().enumerate() {
                        ui.horizontal(|ui| {
                            if ui.small_button("✕").clicked() {
                                to_remove = Some(target.pid);
                            }

                            let color = Self::color_for_index(idx);
                            ui.colored_label(color, "●");

                            let status = if target.alive { "" } else { " (dead)" };
                            ui.label(format!("[{}] {}{}", target.pid, target.name, status));
                        });

                        if let Some(series) = self.model.buffer.get_ram_series(target.pid) {
                            if let Some((_, rss, cpu)) = series.latest() {
                                ui.horizontal(|ui| {
                                    ui.label(format!("  Current (RSS): {}", format_bytes(rss)));
                                    ui.label(format!("CPU: {:.1}%", cpu));
                                });
                            }
                            if let Some((_, peak)) = series.peak_rss() {
                                ui.label(format!("  Peak: {}", format_bytes(peak)));
                            }
                        }

                        if let Some(series) = self.model.buffer.get_vram_series(target.pid) {
                            if let Some((_, vram)) = series.latest() {
                                ui.label(format!("  VRAM: {}", format_bytes(vram)));
                            }
                            if let Some((_, peak)) = series.peak() {
                                ui.label(format!("  VRAM Peak: {}", format_bytes(peak)));
                            }
                        }

                        ui.add_space(4.0);
                    }
                });

                if let Some(pid) = to_remove {
                    self.model.update(Message::RemoveTarget(pid));
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(ref board) = self.model.last_vram_board {
                ui.horizontal(|ui| {
                    ui.heading("GPU 0 VRAM");
                    ui.label(format!(
                        "Total: {} | Used: {} | Free: {}",
                        format_bytes(board.total_bytes),
                        format_bytes(board.used_bytes),
                        format_bytes(board.free_bytes)
                    ));

                    let usage_pct = board.used_bytes as f32 / board.total_bytes as f32;
                    ui.add(
                        egui::ProgressBar::new(usage_pct)
                            .text(format!("{:.1}%", usage_pct * 100.0))
                            .desired_width(150.0),
                    );
                });
                ui.separator();
            }

            match self.model.view_mode {
                ViewMode::Line => {
                    let available = ui.available_size();
                    let plot_height = (available.y - 20.0) / 2.0;

                    ui.heading("Current (RSS)");
                    let rss_plot = Plot::new("rss_plot")
                        .height(plot_height)
                        .x_axis_label("Time (s)")
                        .y_axis_label("MB")
                        .y_axis_formatter(|mark, _range| format!("{:.2}", mark.value))
                        .legend(egui_plot::Legend::default());

                    rss_plot.show(ui, |plot_ui| {
                        let targets: Vec<_> = self.model.targets.list().cloned().collect();
                        for (idx, target) in targets.iter().enumerate() {
                            if let Some(series) = self.model.buffer.get_ram_series(target.pid) {
                                let points: PlotPoints = series
                                    .get_points()
                                    .iter()
                                    .map(|(ts, rss, _)| [*ts as f64 / 1000.0, bytes_to_mb(*rss)])
                                    .collect();

                                let color = Self::color_for_index(idx);
                                let line = Line::new(points)
                                    .name(format!("{} ({})", target.name, target.pid))
                                    .color(color)
                                    .width(2.0_f32);
                                plot_ui.line(line);
                            }
                        }
                    });

                    ui.add_space(10.0);
                    ui.heading("VRAM per Process");
                    let vram_plot = Plot::new("vram_plot")
                        .height(plot_height)
                        .x_axis_label("Time (s)")
                        .y_axis_label("MB")
                        .y_axis_formatter(|mark, _range| format!("{:.2}", mark.value))
                        .legend(egui_plot::Legend::default());

                    vram_plot.show(ui, |plot_ui| {
                        let targets: Vec<_> = self.model.targets.list().cloned().collect();
                        for (idx, target) in targets.iter().enumerate() {
                            if let Some(series) = self.model.buffer.get_vram_series(target.pid) {
                                let points: PlotPoints = series
                                    .get_points()
                                    .iter()
                                    .map(|(ts, vram)| [*ts as f64 / 1000.0, bytes_to_mb(*vram)])
                                    .collect();

                                let color = Self::color_for_index(idx);
                                let line = Line::new(points)
                                    .name(format!("{} ({})", target.name, target.pid))
                                    .color(color)
                                    .width(2.0_f32);
                                plot_ui.line(line);
                            }
                        }
                    });
                }
                ViewMode::Gauge => {
                    ui.heading("Current (RSS) — Gauge");
                    ui.label(
                        egui::RichText::new(
                            "Live snapshot only — history isn't retained while this mode is active.",
                        )
                        .color(egui::Color32::GRAY)
                        .small(),
                    );
                    ui.add_space(6.0);

                    egui::ScrollArea::vertical()
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                        // horizontal_wrapped can't be trusted to wrap here: the tiles
                        // are drawn via egui::Frame, whose size isn't known to the wrap
                        // layouter until after it's placed. Compute columns explicitly
                        // from the available width instead, and lay out row by row —
                        // this recalculates every frame, so resizing the window
                        // reflows the grid immediately.
                        const TILE_WIDTH: f32 = 190.0;
                        let spacing = ui.spacing().item_spacing.x;
                        let columns = (((ui.available_width() + spacing) / (TILE_WIDTH + spacing))
                            .floor() as usize)
                            .max(1);

                        let targets: Vec<_> = self.model.targets.list().cloned().collect();
                        for row in targets.chunks(columns) {
                            ui.horizontal(|ui| {
                                for target in row {
                                    let idx = self
                                        .model
                                        .targets
                                        .list()
                                        .position(|t| t.pid == target.pid)
                                        .unwrap_or(0);
                                    let ram_gauge = self
                                        .model
                                        .buffer
                                        .get_ram_series(target.pid)
                                        .and_then(|s| s.gauge());
                                    let vram_gauge = self
                                        .model
                                        .buffer
                                        .get_vram_series(target.pid)
                                        .and_then(|s| s.gauge());

                                    Self::draw_gauge_tile(
                                        ui,
                                        &format!("{} ({})", target.name, target.pid),
                                        Self::color_for_index(idx),
                                        ram_gauge,
                                        vram_gauge,
                                    );
                                }
                            });
                        }
                    });
                }
            }
        });
    }
}

impl BeholderApp {
    fn draw_gauge_tile(
        ui: &mut egui::Ui,
        label: &str,
        color: egui::Color32,
        ram_gauge: Option<(u64, u64, u64)>,
        vram_gauge: Option<(u64, u64, u64)>,
    ) {
        egui::Frame::none()
            .fill(egui::Color32::from_gray(30))
            .stroke(egui::Stroke::new(1.0_f32, color))
            .inner_margin(egui::Margin::same(10.0))
            .rounding(6.0)
            .show(ui, |ui| {
                ui.set_width(190.0);
                ui.vertical_centered(|ui| {
                    ui.colored_label(color, label);
                    ui.separator();

                    if let Some((current, min, max)) = ram_gauge {
                        ui.label("RAM");
                        Self::paint_dial(ui, current, min, max);
                        ui.heading(format!("{:.2} MB", bytes_to_mb(current)));
                        ui.label(format!(
                            "min {:.2} · max {:.2}",
                            bytes_to_mb(min),
                            bytes_to_mb(max)
                        ));
                    } else {
                        ui.label("RAM: —");
                    }

                    if let Some((current, min, max)) = vram_gauge {
                        ui.add_space(6.0);
                        ui.label("VRAM");
                        Self::paint_dial(ui, current, min, max);
                        ui.heading(format!("{:.2} MB", bytes_to_mb(current)));
                        ui.label(format!(
                            "min {:.2} · max {:.2}",
                            bytes_to_mb(min),
                            bytes_to_mb(max)
                        ));
                    }
                });
            });
    }

    /// Speedometer-style dial: a semicircular arc sweeping from `min` to
    /// `max`, with a needle pointing at `current`.
    fn paint_dial(ui: &mut egui::Ui, current: u64, min: u64, max: u64) {
        let size = egui::vec2(160.0, 90.0);
        let (rect, _response) = ui.allocate_exact_size(size, egui::Sense::hover());
        let painter = ui.painter_at(rect);

        let center = rect.center_bottom() - egui::vec2(0.0, 6.0);
        let radius = (rect.width() / 2.0).min(rect.height()) - 6.0;

        const START_ANGLE: f32 = std::f32::consts::PI;
        const END_ANGLE: f32 = 0.0;
        const STEPS: usize = 48;

        let arc_point = |t: f32| {
            let angle = START_ANGLE + (END_ANGLE - START_ANGLE) * t;
            center + egui::vec2(angle.cos(), -angle.sin()) * radius
        };

        let track: Vec<egui::Pos2> = (0..=STEPS).map(|i| arc_point(i as f32 / STEPS as f32)).collect();
        painter.add(egui::Shape::line(
            track,
            egui::Stroke::new(6.0_f32, egui::Color32::from_gray(60)),
        ));

        let range = max.saturating_sub(min).max(1);
        let frac = (current.saturating_sub(min) as f32 / range as f32).clamp(0.0, 1.0);
        let filled_steps = ((STEPS as f32) * frac).ceil() as usize;

        // Blue (low) -> red (high), drawn as a per-segment gradient so the
        // arc itself shows the climb, not just the needle.
        for i in 0..filled_steps {
            let t0 = i as f32 / STEPS as f32;
            let t1 = ((i + 1) as f32 / STEPS as f32).min(frac);
            painter.line_segment(
                [arc_point(t0), arc_point(t1)],
                egui::Stroke::new(6.0_f32, Self::heat_color(t1)),
            );
        }

        let needle_color = Self::heat_color(frac);
        let needle_angle = START_ANGLE + (END_ANGLE - START_ANGLE) * frac;
        let needle_end = center + egui::vec2(needle_angle.cos(), -needle_angle.sin()) * (radius - 10.0);
        painter.line_segment([center, needle_end], egui::Stroke::new(2.5_f32, needle_color));
        painter.circle_filled(center, 4.0, needle_color);
    }

    /// Interpolates blue (cold, low usage) to red (hot, high usage) by `t` in [0, 1].
    fn heat_color(t: f32) -> egui::Color32 {
        let t = t.clamp(0.0, 1.0);
        const LOW: (f32, f32, f32) = (60.0, 130.0, 246.0); // blue
        const HIGH: (f32, f32, f32) = (230.0, 50.0, 50.0); // red
        let r = LOW.0 + (HIGH.0 - LOW.0) * t;
        let g = LOW.1 + (HIGH.1 - LOW.1) * t;
        let b = LOW.2 + (HIGH.2 - LOW.2) * t;
        egui::Color32::from_rgb(r as u8, g as u8, b as u8)
    }
}

fn bytes_to_mb(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
