use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use crate::api::{self, Build, Channel, FetchResult, InstallProgress};
use crate::launcher::{self, InstalledBuild};
use crate::logger;
use crate::models::{AppTab, AppSettings};
use crate::updater;

pub struct CmManagerApp {
    current_tab: AppTab,
    settings: AppSettings,

    // Versions Tab State
    selected_channel: Channel,
    selected_build_id: String,
    is_fetching_builds: bool,
    initial_fetch_attempted: bool, // Guard flag to prevent infinite loops on error
    builds: Vec<Build>,
    fetch_receiver: Option<Receiver<FetchResult>>,

    // Installed Builds State
    installed_builds: Vec<InstalledBuild>,

    // Installation State
    install_receiver: Option<Receiver<InstallProgress>>,
    install_status_msg: String,
    download_fraction: Option<f32>,

    // Store Tab State
    install_target: String,

    // Updater State
    update_status: String,
}

impl Default for CmManagerApp {
    fn default() -> Self {
        Self {
            current_tab: AppTab::Versions,
            settings: AppSettings::default(),
            selected_channel: Channel::Stable,
            selected_build_id: String::new(),
            is_fetching_builds: false,
            initial_fetch_attempted: false,
            builds: Vec::new(),
            fetch_receiver: None,
            installed_builds: Vec::new(),
            install_receiver: None,
            install_status_msg: String::new(),
            download_fraction: None,
            install_target: "Global (All Versions)".to_string(),
            update_status: String::new(),
        }
    }
}

impl CmManagerApp {
    fn refresh_installed_builds(&mut self) {
        logger::info("Refreshing list of installed builds...");
        self.installed_builds = launcher::scan_installed_builds(&self.settings.install_directory);
    }

    fn trigger_build_fetch(&mut self) {
        if self.is_fetching_builds {
            return;
        }
        logger::action("Initiating fetch for ChroMapper releases from API...");
        self.is_fetching_builds = true;
        let (tx, rx) = channel();
        self.fetch_receiver = Some(rx);
        api::fetch_builds_async(tx);
    }

    fn render_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.heading("CM Manager");
                ui.add_space(20.0);
                
                let prev_tab = self.current_tab;
                ui.selectable_value(&mut self.current_tab, AppTab::Versions, "📦 Installs");
                ui.selectable_value(&mut self.current_tab, AppTab::PluginStore, "🔌 Plugins");
                ui.selectable_value(&mut self.current_tab, AppTab::Migration, "🔄 Migrator");
                ui.selectable_value(&mut self.current_tab, AppTab::Settings, "⚙ Settings");

                if prev_tab != self.current_tab {
                    let tab_name = match self.current_tab {
                        AppTab::Versions => "Versions",
                        AppTab::PluginStore => "PluginStore",
                        AppTab::Migration => "Migration",
                        AppTab::Settings => "Settings",
                    };
                    logger::info(format!("Switched tab to {tab_name}"));
                }
            });
            ui.add_space(5.0);
        });
    }

    fn render_versions_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // --- 1. INSTALLED BUILDS & LAUNCHER SECTION ---
        ui.horizontal(|ui| {
            ui.heading("🚀 Installed Builds");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🔄 Refresh List").clicked() {
                    self.refresh_installed_builds();
                }
            });
        });
        ui.separator();
        ui.add_space(5.0);

        if self.installed_builds.is_empty() {
            ui.label(egui::RichText::new("No installed builds found in your versions folder.").italics());
        } else {
            egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                let mut build_to_delete: Option<InstalledBuild> = None;

                for build in &self.installed_builds {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(&build.name).strong());
                                ui.small(build.executable_path.to_string_lossy());
                            });

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("🗑").on_hover_text("Delete Build").clicked() {
                                    build_to_delete = Some(build.clone());
                                }

                                if ui.button("📂").on_hover_text("Open Folder").clicked() {
                                    logger::action(format!("Opening directory for build: '{}'", build.name));
                                    let _ = open::that(&build.path);
                                }

                                if ui.button("🚀 Launch").clicked() {
                                    if let Err(e) = launcher::launch_build(build, self.settings.auto_kill_chromapper) {
                                        self.install_status_msg = format!("Launch failed: {e}");
                                    } else {
                                        self.install_status_msg = format!("Launched {}", build.name);
                                    }
                                }
                            });
                        });
                    });
                }

                if let Some(to_delete) = build_to_delete {
                    if let Err(e) = launcher::delete_build(&to_delete) {
                        self.install_status_msg = format!("Deletion error: {e}");
                    } else {
                        self.refresh_installed_builds();
                    }
                }
            });
        }

        ui.add_space(20.0);

        // --- 2. DOWNLOAD NEW BUILD SECTION ---
        ui.horizontal(|ui| {
            ui.heading("⬇ Download New Build");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(!self.is_fetching_builds, egui::Button::new("🔄 Fetch Releases")).clicked() {
                    self.trigger_build_fetch();
                }
            });
        });
        ui.separator();
        ui.add_space(10.0);

        // Check for background build fetching results
        if let Some(rx) = &self.fetch_receiver {
            if let Ok(result) = rx.try_recv() {
                self.is_fetching_builds = false;
                match result {
                    FetchResult::Success(fetched) => {
                        logger::info(format!("Fetched {} release(s) successfully.", fetched.len()));
                        self.builds = fetched;
                        if let Some(first) = self.builds.iter().find(|b| b.channel == self.selected_channel) {
                            self.selected_build_id = first.id.clone();
                        }
                    }
                    FetchResult::Error(err) => {
                        logger::error(format!("Failed to fetch releases: {err}"));
                        self.install_status_msg = format!("Failed to load releases: {err}");
                    }
                }
                self.fetch_receiver = None;
            }
            ctx.request_repaint();
        }

        if self.is_fetching_builds {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Fetching latest ChroMapper releases from GitHub...");
            });
            ui.add_space(10.0);
        }

        // Selection Grid
        egui::Grid::new("version_grid").spacing([20.0, 10.0]).show(ui, |ui| {
            ui.label("Release Channel:");
            let prev_channel = self.selected_channel;
            egui::ComboBox::from_id_source("release_channel")
                .selected_text(format!("{}", self.selected_channel))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.selected_channel, Channel::Stable, "Stable");
                    ui.selectable_value(&mut self.selected_channel, Channel::Dev, "Dev / Pre-release");
                });

            if prev_channel != self.selected_channel {
                logger::info(format!("Selected channel changed to: {}", self.selected_channel));
                if let Some(first) = self.builds.iter().find(|b| b.channel == self.selected_channel) {
                    self.selected_build_id = first.id.clone();
                } else {
                    self.selected_build_id.clear();
                }
            }
            ui.end_row();

            ui.label("Select Build:");
            let filtered_builds: Vec<&Build> = self
                .builds
                .iter()
                .filter(|b| b.channel == self.selected_channel)
                .collect();

            let selected_text = self
                .builds
                .iter()
                .find(|b| b.id == self.selected_build_id)
                .map(|b| format!("{} ({})", b.version, b.release_date))
                .unwrap_or_else(|| "Select a build...".to_string());

            egui::ComboBox::from_id_source("build_version")
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    for build in filtered_builds {
                        let label = format!("{} ({})", build.version, build.release_date);
                        ui.selectable_value(&mut self.selected_build_id, build.id.clone(), label);
                    }
                });
            ui.end_row();
        });

        ui.add_space(15.0);

        // Download & Install Action
        let is_installing = self.install_receiver.is_some();
        let selected_build = self.builds.iter().find(|b| b.id == self.selected_build_id).cloned();

        ui.add_enabled_ui(!is_installing && selected_build.is_some(), |ui| {
            if ui.button("⬇ Download & Auto-Extract Selected Build").clicked() {
                if let Some(build) = selected_build {
                    logger::action(format!("Starting download for build '{}' ({})", build.version, build.download_url));
                    let (tx, rx) = channel();
                    self.install_receiver = Some(rx);
                    self.install_status_msg = "Starting install process...".to_string();
                    self.download_fraction = Some(0.0);

                    let target_dir = PathBuf::from(&self.settings.install_directory)
                        .join("versions")
                        .join(build.version.replace('/', "_"));

                    api::install_build_async(build.download_url, target_dir, tx);
                }
            }
        });

        // Background Installation Receiver
        if let Some(rx) = &self.install_receiver {
            if let Ok(progress) = rx.try_recv() {
                match progress {
                    InstallProgress::Started => {
                        self.install_status_msg = "Connecting to server...".to_string();
                    }
                    InstallProgress::Downloading { downloaded, total } => {
                        if let Some(t) = total {
                            let frac = downloaded as f32 / t as f32;
                            self.download_fraction = Some(frac);
                            self.install_status_msg = format!(
                                "Downloading... {:.1} MB / {:.1} MB ({:.0}%)",
                                downloaded as f64 / 1_048_576.0,
                                t as f64 / 1_048_576.0,
                                frac * 100.0
                            );
                        } else {
                            self.install_status_msg = format!(
                                "Downloading... {:.1} MB",
                                downloaded as f64 / 1_048_576.0
                            );
                        }
                    }
                    InstallProgress::Extracting => {
                        logger::info("Download completed; extracting archive...");
                        self.download_fraction = None;
                        self.install_status_msg = "📦 Unzipping and extracting files...".to_string();
                    }
                    InstallProgress::Finished(path) => {
                        logger::info(format!("Installation successfully completed at '{}'", path.display()));
                        self.download_fraction = None;
                        self.install_status_msg = format!("✅ Successfully installed to: {path:?}");
                        self.install_receiver = None;
                        self.refresh_installed_builds(); // Automatically refresh UI list!
                    }
                    InstallProgress::Failed(err) => {
                        logger::error(format!("Installation failed: {err}"));
                        self.download_fraction = None;
                        self.install_status_msg = format!("❌ Installation failed: {err}");
                        self.install_receiver = None;
                    }
                }
            }
            ctx.request_repaint();
        }

        if !self.install_status_msg.is_empty() {
            ui.add_space(10.0);
            if let Some(frac) = self.download_fraction {
                ui.add(egui::ProgressBar::new(frac).show_percentage());
            }
            ui.label(egui::RichText::new(&self.install_status_msg).strong());
        }
    }

    fn render_store_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("🔌 Plugin Store (BeatMods)");
        ui.horizontal(|ui| {
            ui.label("Install to:");
            egui::ComboBox::from_id_source("install_target")
                .selected_text(&self.install_target)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.install_target, "Global (All Versions)".to_string(), "Global (All Versions)");
                    ui.separator();
                    for installed in &self.installed_builds {
                        ui.selectable_value(&mut self.install_target, installed.name.clone(), &installed.name);
                    }
                });
        });
        ui.separator();

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("ChroMapper-LightID").strong());
                    ui.label("Pulls latest .dll from GitHub release");
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Install").clicked() {
                        logger::action("User requested install for plugin: ChroMapper-LightID");
                    }
                });
            });
        });
    }

    fn render_migration_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("🔄 Migrate Setup");
        ui.separator();

        if self.installed_builds.len() < 2 {
            ui.label("You need at least 2 installed builds to perform a migration.");
            return;
        }

        ui.horizontal(|ui| {
            ui.label("Source Install:");
            egui::ComboBox::from_id_source("source_box")
                .selected_text("Select Source...")
                .show_ui(ui, |ui| {
                    for build in &self.installed_builds {
                        ui.label(&build.name);
                    }
                });
        });

        ui.horizontal(|ui| {
            ui.label("Target Install: ");
            egui::ComboBox::from_id_source("target_box")
                .selected_text("Select Target...")
                .show_ui(ui, |ui| {
                    for build in &self.installed_builds {
                        ui.label(&build.name);
                    }
                });
        });

        ui.add_space(10.0);
        ui.checkbox(&mut true, "Copy Plugins folder");
        ui.checkbox(&mut true, "Copy Settings.json");
        ui.add_space(10.0);
        if ui.button("🚀 Start Migration").clicked() {
            logger::action("User initiated migration workflow.");
        }
    }

    fn render_settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("⚙ Settings");
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Base Install Directory:");
            if ui.text_edit_singleline(&mut self.settings.install_directory).changed() {
                logger::info(format!("Install directory updated to: {}", self.settings.install_directory));
                self.refresh_installed_builds();
            }
        });
        ui.small("Where ChroMapper builds and global plugins are saved.");

        ui.add_space(15.0);

        if ui.button("📂 Open Global Plugins Folder").clicked() {
            let global_path = format!("{}/global_plugins", self.settings.install_directory);
            logger::action(format!("Opening global plugins directory at: {global_path}"));
            std::fs::create_dir_all(&global_path).unwrap_or_default();
            let _ = open::that(&global_path);
        }

        ui.add_space(15.0);
        ui.checkbox(&mut self.settings.auto_kill_chromapper, "Auto-kill ChroMapper before launching/updating");

        let mut is_dark = self.settings.dark_mode;
        if ui.checkbox(&mut is_dark, "Use Dark Theme").changed() {
            self.settings.dark_mode = is_dark;
            if is_dark {
                ui.ctx().set_visuals(egui::Visuals::dark());
            } else {
                ui.ctx().set_visuals(egui::Visuals::light());
            }
        }

        ui.add_space(20.0);
        ui.separator();
        ui.add_space(10.0);

        if ui.button("🔄 Check for App Updates").clicked() {
            logger::action("Checking for application self-updates...");
            self.update_status = "Checking GitHub...".to_string();
            match updater::check_for_updates() {
                Ok(msg) => {
                    logger::info(&msg);
                    self.update_status = msg;
                }
                Err(e) => {
                    let err_msg = format!("Update failed: {e}");
                    logger::error(&err_msg);
                    self.update_status = err_msg;
                }
            }
        }
        if !self.update_status.is_empty() {
            ui.label(&self.update_status);
        }
    }
}

impl eframe::App for CmManagerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Initial setup on boot (runs only once, protecting against infinite fetch retry loops)
        if !self.initial_fetch_attempted && !self.is_fetching_builds && self.fetch_receiver.is_none() {
            self.initial_fetch_attempted = true;
            logger::info("App initialized; performing initial scan and fetch.");
            self.refresh_installed_builds();
            self.trigger_build_fetch();
        }

        if self.settings.dark_mode {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }

        self.render_top_bar(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.current_tab {
                AppTab::Versions => self.render_versions_tab(ui, ctx),
                AppTab::PluginStore => self.render_store_tab(ui),
                AppTab::Migration => self.render_migration_tab(ui),
                AppTab::Settings => self.render_settings_tab(ui),
            }
        });
    }
}