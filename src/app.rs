use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use eframe::egui::{self, Color32, RichText, Vec2};
use crate::compressor::{format_size, process_image, ImageItem, ItemStatus, OutputFormat};

pub enum WorkerMsg {
    Progress { id: usize, status: ItemStatus },
    FinishedAll,
}

pub struct CompressorApp {
    // Files list
    pub items: Vec<ImageItem>,
    pub output_dir: Option<PathBuf>,
    pub format_pref: OutputFormat,

    // Batch target controls
    pub batch_target_mb: f32,

    // Processing state
    pub is_processing: bool,
    pub rx: Option<Receiver<WorkerMsg>>,
    pub status_message: String,
    pub completed_count: usize,
    pub total_saved_bytes: i64,
}

impl Default for CompressorApp {
    fn default() -> Self {
        // Default output dir: User's Pictures/Compressed or Desktop
        let default_out = dirs_next().or_else(|| std::env::current_dir().ok());

        Self {
            items: Vec::new(),
            output_dir: default_out,
            format_pref: OutputFormat::Auto,
            batch_target_mb: 2.0,
            is_processing: false,
            rx: None,
            status_message: "Siap digunakan. Tambahkan foto atau folder untuk memulai.".to_string(),
            completed_count: 0,
            total_saved_bytes: 0,
        }
    }
}

fn dirs_next() -> Option<PathBuf> {
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        let p = PathBuf::from(user_profile).join("Pictures").join("Compressed");
        let _ = std::fs::create_dir_all(&p);
        Some(p)
    } else {
        None
    }
}

impl CompressorApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    fn add_files(&mut self, paths: Vec<PathBuf>) {
        let mut added = 0;
        for path in paths {
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let ext_lower = ext.to_lowercase();
                    if ["jpg", "jpeg", "png", "webp", "bmp"].contains(&ext_lower.as_str()) {
                        let id = self.items.len();
                        if let Ok(item) = ImageItem::new(id, path) {
                            self.items.push(item);
                            added += 1;
                        }
                    }
                }
            }
        }
        if added > 0 {
            self.recalc_default_batch_target();
            self.status_message = format!("Berhasil menambahkan {} foto.", added);
        }
    }

    fn add_folder(&mut self, folder: PathBuf) {
        if let Ok(entries) = std::fs::read_dir(folder) {
            let mut paths = Vec::new();
            for entry in entries.flatten() {
                paths.push(entry.path());
            }
            self.add_files(paths);
        }
    }

    fn total_original_bytes(&self) -> u64 {
        self.items.iter().map(|i| i.original_bytes).sum()
    }

    fn total_target_bytes(&self) -> u64 {
        self.items.iter().map(|i| i.target_bytes).sum()
    }

    fn recalc_default_batch_target(&mut self) {
        let orig = self.total_original_bytes();
        let target = (orig as f64 * 0.5) / (1024.0 * 1024.0);
        self.batch_target_mb = (target as f32).max(0.1);
    }

    /// Terapkan target total MB secara proporsional ke semua file
    fn apply_batch_target(&mut self) {
        let total_target_bytes = (self.batch_target_mb as f64 * 1024.0 * 1024.0) as u64;
        let orig_total = self.total_original_bytes();

        if orig_total == 0 || self.items.is_empty() {
            return;
        }

        let ratio = (total_target_bytes as f64) / (orig_total as f64);

        for item in &mut self.items {
            let item_target = (item.original_bytes as f64 * ratio) as u64;
            // Batas minimal 10 KB per foto
            item.target_bytes = item_target.max(10 * 1024).min(item.original_bytes);
        }

        self.status_message = format!(
            "Target total {} diterapkan secara proporsional ke {} foto.",
            format_size(total_target_bytes),
            self.items.len()
        );
    }

    fn start_compression(&mut self) {
        if self.items.is_empty() {
            self.status_message = "Tidak ada foto dalam antrian.".to_string();
            return;
        }

        let out_dir = match &self.output_dir {
            Some(dir) => dir.clone(),
            None => {
                self.status_message = "Silakan tentukan folder output terlebih dahulu.".to_string();
                return;
            }
        };

        if let Err(e) = std::fs::create_dir_all(&out_dir) {
            self.status_message = format!("Gagal membuat folder output: {e}");
            return;
        }

        self.is_processing = true;
        self.completed_count = 0;
        self.total_saved_bytes = 0;
        for item in &mut self.items {
            item.status = ItemStatus::Pending;
        }

        let (tx, rx): (Sender<WorkerMsg>, Receiver<WorkerMsg>) = channel();
        self.rx = Some(rx);

        let items = self.items.clone();
        let format_pref = self.format_pref;

        thread::spawn(move || {
            for item in items {
                let _ = tx.send(WorkerMsg::Progress {
                    id: item.id,
                    status: ItemStatus::Processing,
                });

                match process_image(&item, &out_dir, format_pref) {
                    Ok((bytes, out_path)) => {
                        let _ = tx.send(WorkerMsg::Progress {
                            id: item.id,
                            status: ItemStatus::Done {
                                compressed_bytes: bytes,
                                out_path,
                            },
                        });
                    }
                    Err(err) => {
                        let _ = tx.send(WorkerMsg::Progress {
                            id: item.id,
                            status: ItemStatus::Error(err),
                        });
                    }
                }
            }

            let _ = tx.send(WorkerMsg::FinishedAll);
        });
    }

    fn poll_worker(&mut self) {
        let mut finished = false;
        if let Some(ref rx) = self.rx {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    WorkerMsg::Progress { id, status } => {
                        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
                            if let ItemStatus::Done { compressed_bytes, .. } = status {
                                self.completed_count += 1;
                                let saved = item.original_bytes as i64 - compressed_bytes as i64;
                                self.total_saved_bytes += saved;
                            }
                            item.status = status;
                        }
                    }
                    WorkerMsg::FinishedAll { .. } => {
                        finished = true;
                    }
                }
            }
        }

        if finished {
            self.is_processing = false;
            self.rx = None;
            let saved_mb = self.total_saved_bytes as f64 / (1024.0 * 1024.0);
            self.status_message = format!(
                "Kompresi selesai! Total ruang dihemat: {:.2} MB ({} foto selesai).",
                saved_mb.max(0.0),
                self.completed_count
            );
        }
    }
}

impl eframe::App for CompressorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_worker();

        if self.is_processing {
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);

            // Header Section
            ui.horizontal(|ui| {
                ui.heading(RichText::new("🦀 Rust Image Compressor").size(22.0).color(Color32::from_rgb(255, 140, 50)).strong());
                ui.label(RichText::new("Bulk & Single Image Optimizer with Custom Target Size").color(Color32::from_rgb(160, 170, 190)));
            });

            ui.separator();

            // Top Action Toolbar
            ui.horizontal_wrapped(|ui| {
                if ui.button(RichText::new("➕ Tambah Foto").strong()).clicked() {
                    if let Some(files) = rfd::FileDialog::new()
                        .add_filter("Images", &["jpg", "jpeg", "png", "webp", "bmp"])
                        .pick_files()
                    {
                        self.add_files(files);
                    }
                }

                if ui.button(RichText::new("📁 Tambah Folder").strong()).clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.add_folder(folder);
                    }
                }

                if ui.button("🗑 Bersihkan List").clicked() {
                    self.items.clear();
                    self.completed_count = 0;
                    self.total_saved_bytes = 0;
                    self.status_message = "Daftar antrian telah dibersihkan.".to_string();
                }

                ui.separator();

                // Folder Output Chooser
                ui.label(RichText::new("Lokasi Simpan:").strong());
                let out_str = self
                    .output_dir
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Belum dipilih".to_string());

                let display_out = if out_str.len() > 35 {
                    format!("...{}", &out_str[out_str.len() - 35..])
                } else {
                    out_str.clone()
                };

                ui.label(RichText::new(display_out).italics().color(Color32::LIGHT_BLUE));

                if ui.button("Ganti Folder").clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.output_dir = Some(folder);
                    }
                }

                if let Some(ref dir) = self.output_dir {
                    if ui.button("Buka Folder").clicked() {
                        #[cfg(target_os = "windows")]
                        let _ = std::process::Command::new("explorer").arg(dir).spawn();
                    }
                }
            });

            // Target Kontrol Global / Bulk Section
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("🎯 Target Total Kompresi Seluruh Foto:").strong());
                    
                    let orig_total = self.total_original_bytes();
                    let orig_mb = (orig_total as f64) / (1024.0 * 1024.0);
                    
                    ui.label(format!("(Total Asli: {:.2} MB)", orig_mb));

                    let max_slider = orig_mb.max(1.0) as f32;
                    let slider = egui::Slider::new(&mut self.batch_target_mb, 0.1..=max_slider)
                        .suffix(" MB")
                        .step_by(0.05);

                    ui.add(slider);

                    if ui.button(RichText::new("⚡ Terapkan ke Semua Foto").strong().color(Color32::WHITE)).clicked() {
                        self.apply_batch_target();
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Format Output:");
                    ui.radio_value(&mut self.format_pref, OutputFormat::Auto, "Auto (Ikuti Sumber)");
                    ui.radio_value(&mut self.format_pref, OutputFormat::Jpeg, "JPEG (.jpg)");
                    ui.radio_value(&mut self.format_pref, OutputFormat::WebP, "WebP (.webp)");

                    let current_target_total = self.total_target_bytes();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!(
                            "Estimasi Total: {}",
                            format_size(current_target_total)
                        )).color(Color32::LIGHT_GREEN).strong());
                    });
                });
            });

            // Progress Bar if running
            if self.is_processing {
                let progress = if !self.items.is_empty() {
                    self.completed_count as f32 / self.items.len() as f32
                } else {
                    0.0
                };
                ui.add(
                    egui::ProgressBar::new(progress)
                        .show_percentage()
                        .animate(true)
                        .text(format!("{}/{} Selesai", self.completed_count, self.items.len())),
                );
            }

            // Status strip
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.status_message).color(Color32::LIGHT_GRAY));
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let btn_text = if self.is_processing {
                        RichText::new("Sedang Memproses...").color(Color32::YELLOW)
                    } else {
                        RichText::new("🚀 Mulai Kompres Sekarang").size(15.0).strong().color(Color32::WHITE)
                    };

                    let btn = egui::Button::new(btn_text)
                        .min_size(Vec2::new(180.0, 32.0))
                        .fill(if self.is_processing { Color32::DARK_GRAY } else { Color32::from_rgb(30, 130, 60) });

                    if ui.add_enabled(!self.is_processing && !self.items.is_empty(), btn).clicked() {
                        self.start_compression();
                    }
                });
            });

            ui.separator();

            // Table Header & Scrollable List of Files
            ui.label(RichText::new(format!("Daftar File ({}) — Geser Slider Per-File untuk Atur Ukuran Target:", self.items.len())).strong());

            egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                if self.items.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(60.0);
                        ui.label(RichText::new("Belum ada foto yang dipilih.").size(16.0).color(Color32::GRAY));
                        ui.label("Klik tombol 'Tambah Foto' atau 'Tambah Folder' di atas untuk memulai.");
                        ui.add_space(60.0);
                    });
                } else {
                    for item in &mut self.items {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                // Status Indicator
                                let (status_str, status_color) = match &item.status {
                                    ItemStatus::Pending => ("⏳ Pending", Color32::GRAY),
                                    ItemStatus::Processing => ("🔄 Proses...", Color32::YELLOW),
                                    ItemStatus::Done { compressed_bytes, .. } => {
                                        let saved = (item.original_bytes as i64 - *compressed_bytes as i64).max(0);
                                        let pct = (saved as f64 / item.original_bytes as f64) * 100.0;
                                        ui.label(RichText::new(format!("✅ Selesai ({}, -{:.0}%)", format_size(*compressed_bytes), pct)).color(Color32::LIGHT_GREEN).strong());
                                        ("", Color32::LIGHT_GREEN)
                                    }
                                    ItemStatus::Error(e) => {
                                        ui.label(RichText::new(format!("❌ Gagal: {}", e)).color(Color32::LIGHT_RED));
                                        ("", Color32::LIGHT_RED)
                                    }
                                };

                                if !status_str.is_empty() {
                                    ui.label(RichText::new(status_str).color(status_color).strong());
                                }

                                // File Name & Details
                                ui.label(RichText::new(&item.file_name).strong());
                                ui.label(format!("({}x{})", item.dimensions.0, item.dimensions.1));
                                ui.label(RichText::new(format!("Asli: {}", format_size(item.original_bytes))).color(Color32::KHAKI));

                                ui.separator();

                                // Slider per file
                                let orig_kb = (item.original_bytes as f64 / 1024.0) as u32;
                                let mut target_kb = (item.target_bytes as f64 / 1024.0) as u32;

                                ui.label("Target:");
                                let slider = egui::Slider::new(&mut target_kb, 10..=orig_kb.max(20))
                                    .suffix(" KB")
                                    .logarithmic(false);

                                if ui.add_enabled(!self.is_processing, slider).changed() {
                                    item.target_bytes = (target_kb as u64) * 1024;
                                }

                                let pct = if orig_kb > 0 {
                                    ((orig_kb as i64 - target_kb as i64) as f64 / orig_kb as f64) * 100.0
                                } else {
                                    0.0
                                };
                                ui.label(RichText::new(format!("(Turun ~{:.0}%)", pct.max(0.0))).color(Color32::LIGHT_BLUE));
                            });
                        });
                    }
                }
            });
        });
    }
}
