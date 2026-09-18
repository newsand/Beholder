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
    Tick,
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
            Message::Tick => {
                self.do_sample();
            }
        }
    }

    fn do_sample(&mut self) {
        let mut pids_to_sample = Vec::new();
        for pid in self.targets.alive_pids() {
            match self.targets.check_pid_status(pid) {
                beholder_core::PidStatus::Dead => {
                    self.targets.mark_dead(pid);
                }
                beholder_core::PidStatus::Reused => {
                    self.targets.mark_reused(pid);
                }
                beholder_core::PidStatus::Alive => {
                    pids_to_sample.push(pid);
                }
            }
        }

        let result = self.sampler.sample_pids(&pids_to_sample, self.nvml.as_ref());

        for pid in result.not_found {
            self.targets.mark_dead(pid);
        }

        for sample in result.ram_samples {
            self.buffer.push_ram(sample);
        }
        for sample in result.vram_samples {
            self.buffer.push_vram(sample);
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
        let mut lines = vec!["rss_ts_ms,cpu_ts_ms,pid,name,rss_bytes,cpu_pct,vram_bytes".to_string()];

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

            let mut combined: Vec<(u64, u64, u64, f32, Option<u64>)> = Vec::new();

            for (rss_ts, rss, cpu_ts, cpu) in &ram_points {
                combined.push((*rss_ts, *cpu_ts, *rss, *cpu, None));
            }

            for (ts, vram) in &vram_points {
                if let Some(entry) = combined.iter_mut().find(|(rss_ts, _, _, _, _)| *rss_ts == *ts) {
                    entry.4 = Some(*vram);
                } else {
                    combined.push((*ts, *ts, 0, 0.0, Some(*vram)));
                }
            }

            combined.sort_by_key(|(rss_ts, _, _, _, _)| *rss_ts);

            for (rss_ts, cpu_ts, rss, cpu, vram) in combined {
                let vram_str = vram.map(|v| v.to_string()).unwrap_or_default();
                lines.push(format!(
                    "{},{},{},{},{},{:.2},{}",
                    rss_ts, cpu_ts, target.pid, target.name, rss, cpu, vram_str
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
                .map(|(rss_ts, rss, cpu_ts, cpu)| {
                    serde_json::json!({
                        "rss_ts_ms": rss_ts,
                        "rss_bytes": rss,
                        "cpu_ts_ms": cpu_ts,
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
                            let color = Self::color_for_index(idx);
                            ui.colored_label(color, "●");

                            let status = if target.alive { "" } else { " (dead)" };
                            ui.label(format!("[{}] {}{}", target.pid, target.name, status));

                            if ui.small_button("✕").clicked() {
                                to_remove = Some(target.pid);
                            }
                        });

                        if let Some(series) = self.model.buffer.get_ram_series(target.pid) {
                            if let Some(sample) = series.latest_raw() {
                                ui.horizontal(|ui| {
                                    ui.label(format!("  RSS: {}", format_bytes(sample.rss_bytes)));
                                    ui.label(format!("CPU: {:.1}%", sample.cpu_pct));
                                });
                            }
                            if let Some((_, peak)) = series.peak_rss() {
                                ui.label(format!("  Peak: {}", format_bytes(peak)));
                            }
                        }

                        if let Some(series) = self.model.buffer.get_vram_series(target.pid) {
                            if let Some(sample) = series.latest_raw() {
                                ui.label(format!("  VRAM: {}", format_bytes(sample.used_bytes)));
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

            let available = ui.available_size();
            let plot_height = (available.y - 20.0) / 2.0;

            ui.heading("RSS (Resident Set Size)");
            let rss_plot = Plot::new("rss_plot")
                .height(plot_height)
                .x_axis_label("Time (s)")
                .y_axis_label("Bytes")
                .legend(egui_plot::Legend::default());

            rss_plot.show(ui, |plot_ui| {
                let targets: Vec<_> = self.model.targets.list().cloned().collect();
                for (idx, target) in targets.iter().enumerate() {
                    if let Some(series) = self.model.buffer.get_ram_series(target.pid) {
                        let points: PlotPoints = series
                            .get_points()
                            .iter()
                            .map(|(rss_ts, rss, _, _)| [*rss_ts as f64 / 1000.0, *rss as f64])
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
                .y_axis_label("Bytes")
                .legend(egui_plot::Legend::default());

            vram_plot.show(ui, |plot_ui| {
                let targets: Vec<_> = self.model.targets.list().cloned().collect();
                for (idx, target) in targets.iter().enumerate() {
                    if let Some(series) = self.model.buffer.get_vram_series(target.pid) {
                        let points: PlotPoints = series
                            .get_points()
                            .iter()
                            .map(|(ts, vram)| [*ts as f64 / 1000.0, *vram as f64])
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
        });
    }
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
