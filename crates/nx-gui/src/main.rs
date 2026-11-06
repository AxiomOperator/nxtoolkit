//! `nxtoolkit-gui`: graphical frontend (egui).

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::Duration,
};

use clap::Parser;
use crossbeam_channel::Receiver;
use eframe::egui::{self, Color32, RichText};
use nx_core::{
    ActionKind, Elevation, Event, Family, InitOptions, Mode, Plan, Toolkit, catalog::InstallMethod,
};

#[derive(Parser)]
#[command(name = "nxtoolkit-gui", version, about = "Graphical nxtoolkit")]
struct Cli {
    /// Use catalog + scripts from DIR instead of the built-in copy.
    #[arg(long, value_name = "DIR")]
    catalog: Option<PathBuf>,
    /// Pretend to be this distro family (runs disabled).
    #[arg(long, value_name = "FAMILY")]
    distro: Option<Family>,
}

fn main() -> eframe::Result {
    let cli = Cli::parse();
    let opts = InitOptions {
        catalog_dir: cli.catalog,
        family_override: cli.distro,
    };
    let simulated = opts.family_override.is_some();
    let tk = match Toolkit::init(&opts) {
        Ok(tk) => tk,
        Err(e) => {
            eprintln!("nxtoolkit: {e:#}");
            std::process::exit(1);
        }
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("nxtoolkit")
            .with_app_id("nxtoolkit")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "nxtoolkit",
        options,
        Box::new(move |_cc| Ok(Box::new(GuiApp::new(tk, simulated)))),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Install,
    Tweaks,
    Maintenance,
    Setup,
}

impl Tab {
    const ALL: [Tab; 4] = [Tab::Install, Tab::Tweaks, Tab::Maintenance, Tab::Setup];
    fn title(self) -> &'static str {
        match self {
            Tab::Install => "Install",
            Tab::Tweaks => "Tweaks",
            Tab::Maintenance => "Maintenance",
            Tab::Setup => "Setup",
        }
    }
    fn kind(self) -> Option<ActionKind> {
        match self {
            Tab::Install => None,
            Tab::Tweaks => Some(ActionKind::Tweak),
            Tab::Maintenance => Some(ActionKind::Maintenance),
            Tab::Setup => Some(ActionKind::Setup),
        }
    }
}

struct Item {
    id: String,
    name: String,
    category: String,
    tooltip: String,
    has_check: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StepState {
    Pending,
    Running,
    Ok,
    Failed,
}

struct RunState {
    labels: Vec<String>,
    states: Vec<StepState>,
    output: String,
    rx: Receiver<Event>,
    done: Option<(usize, Option<PathBuf>)>,
}

struct GuiApp {
    tk: Toolkit,
    simulated: bool,
    tab: Tab,
    items: HashMap<&'static str, Vec<Item>>,
    selected: HashSet<String>,
    search: String,
    confirm: Option<(Plan, Mode)>,
    run: Option<RunState>,
    show_output: bool,
    applied: HashMap<String, bool>,
    checks: Option<Receiver<(String, bool)>>,
    message: Option<String>,
}

impl GuiApp {
    fn new(tk: Toolkit, simulated: bool) -> Self {
        let family = tk.distro.family;
        let has_aur = tk.aur.is_some();
        let mut items = HashMap::new();
        items.insert(
            Tab::Install.title(),
            tk.catalog
                .apps
                .iter()
                .map(|a| {
                    let how = match a.install_method(family, has_aur) {
                        Some(InstallMethod::Native(p)) => {
                            format!("{}: {}", nx_core::pkg::native(family).name(), p.join(" "))
                        }
                        Some(InstallMethod::Aur(p)) => format!("AUR: {}", p.join(" ")),
                        Some(InstallMethod::Flatpak(p)) => format!("Flathub: {}", p.join(" ")),
                        None => String::new(),
                    };
                    Item {
                        id: a.id.clone(),
                        name: a.name.clone(),
                        category: a.category.clone(),
                        tooltip: format!("{}\n\n{how}", a.description),
                        has_check: false,
                    }
                })
                .collect::<Vec<_>>(),
        );
        for tab in &Tab::ALL[1..] {
            let kind = tab.kind().unwrap();
            items.insert(
                tab.title(),
                tk.catalog
                    .actions_of(kind)
                    .map(|a| {
                        let mut tip = a.description.clone();
                        tip.push_str(&format!(
                            "\n\nRuns as {}{}",
                            if a.root { "root" } else { "your user" },
                            if a.revert.is_some() {
                                " · revertible"
                            } else {
                                ""
                            }
                        ));
                        if let Some(c) = &a.confirm {
                            tip.push_str(&format!("\n⚠ {c}"));
                        }
                        Item {
                            id: a.id.clone(),
                            name: a.name.clone(),
                            category: a.category.clone(),
                            tooltip: tip,
                            has_check: a.check.is_some(),
                        }
                    })
                    .collect(),
            );
        }
        let checks = Some(tk.spawn_checks());
        GuiApp {
            tk,
            simulated,
            tab: Tab::Install,
            items,
            selected: HashSet::new(),
            search: String::new(),
            confirm: None,
            run: None,
            show_output: false,
            applied: HashMap::new(),
            checks,
            message: None,
        }
    }

    fn poll(&mut self) -> bool {
        let mut busy = false;
        if let Some(rx) = &self.checks {
            for (id, ok) in rx.try_iter() {
                self.applied.insert(id, ok);
            }
            busy = true;
            if rx.is_empty()
                && self.applied.len()
                    >= self
                        .tk
                        .catalog
                        .actions
                        .iter()
                        .filter(|a| a.check.is_some())
                        .count()
            {
                busy = false;
            }
        }
        let mut finished = false;
        if let Some(run) = &mut self.run {
            for ev in run.rx.try_iter() {
                match ev {
                    Event::StepStarted { index } => {
                        run.states[index] = StepState::Running;
                        run.output.push_str(&format!("==> {}\n", run.labels[index]));
                    }
                    Event::Output { line } => {
                        run.output.push_str(&line);
                        run.output.push('\n');
                    }
                    Event::StepFinished { index, success } => {
                        run.states[index] = if success {
                            StepState::Ok
                        } else {
                            StepState::Failed
                        };
                    }
                    Event::Finished { failed, log } => {
                        run.done = Some((failed.len(), log));
                        finished = true;
                    }
                }
            }
            const MAX: usize = 400_000;
            if run.output.len() > MAX {
                let cut = run.output.len() - MAX;
                let cut = run.output[cut..].find('\n').map_or(cut, |n| cut + n + 1);
                run.output.drain(..cut);
            }
            busy |= run.done.is_none();
        }
        if finished {
            if self
                .run
                .as_ref()
                .is_some_and(|r| r.done.as_ref().is_some_and(|d| d.0 == 0))
            {
                self.selected.clear();
            }
            self.applied.clear();
            self.checks = Some(self.tk.spawn_checks());
        }
        busy
    }

    fn running(&self) -> bool {
        self.run.as_ref().is_some_and(|r| r.done.is_none())
    }

    fn prepare(&mut self, mode: Mode) {
        let ids: Vec<String> = self.selected.iter().cloned().collect();
        if ids.is_empty() {
            self.message = Some("Select something first.".into());
            return;
        }
        match self.tk.plan(&ids, mode) {
            Ok(p) if p.is_empty() => {
                self.message = Some(
                    p.warnings
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "Nothing to do.".into()),
                )
            }
            Ok(p) => self.confirm = Some((p, mode)),
            Err(e) => self.message = Some(e.to_string()),
        }
    }

    fn start(&mut self, plan: Plan) {
        if self.simulated {
            self.message = Some("Simulated distro (--distro): runs are disabled.".into());
            return;
        }
        let labels: Vec<String> = plan.steps.iter().map(|s| s.label.clone()).collect();
        let n = labels.len();
        let rx = self.tk.spawn_run(plan, Elevation::for_frontend(true));
        self.run = Some(RunState {
            labels,
            states: vec![StepState::Pending; n],
            output: String::new(),
            rx,
            done: None,
        });
        self.show_output = true;
        self.message = None;
    }

    fn ui_header(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.heading(RichText::new("nxtoolkit").strong());
            ui.label(RichText::new(&self.tk.distro.name).weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("🔍 Search")
                        .desired_width(220.0),
                );
            });
        });
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            for tab in Tab::ALL {
                let n = self.items[tab.title()]
                    .iter()
                    .filter(|i| self.selected.contains(&i.id))
                    .count();
                let label = if n > 0 {
                    format!("{} ({n})", tab.title())
                } else {
                    tab.title().to_string()
                };
                if ui
                    .selectable_label(self.tab == tab, RichText::new(label).size(15.0))
                    .clicked()
                {
                    self.tab = tab;
                }
            }
        });
        ui.add_space(4.0);
    }

    fn ui_footer(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let running = self.running();
            let (apply, revert) = match self.tab {
                Tab::Install => ("Install selected", "Uninstall selected"),
                _ => ("Apply selected", "Revert selected"),
            };
            ui.add_enabled_ui(!running && !self.selected.is_empty(), |ui| {
                if ui.button(RichText::new(apply).strong()).clicked() {
                    self.prepare(Mode::Apply);
                }
                if ui.button(revert).clicked() {
                    self.prepare(Mode::Revert);
                }
            });
            if ui
                .add_enabled(
                    !self.selected.is_empty(),
                    egui::Button::new("Clear selection"),
                )
                .clicked()
            {
                self.selected.clear();
            }
            if self.run.is_some() {
                ui.toggle_value(&mut self.show_output, "Output");
            }
            ui.separator();
            if running {
                ui.spinner();
                ui.label("Running…");
            } else if let Some(m) = &self.message {
                ui.label(RichText::new(m).color(Color32::from_rgb(230, 180, 60)));
            } else {
                ui.label(RichText::new(format!("{} selected", self.selected.len())).weak());
            }
        });
        ui.add_space(4.0);
    }

    fn ui_items(&mut self, ui: &mut egui::Ui) {
        let q = self.search.to_lowercase();
        let items = &self.items[self.tab.title()];
        let mut categories: Vec<&str> = Vec::new();
        for it in items {
            if !categories.contains(&it.category.as_str()) {
                categories.push(&it.category);
            }
        }
        let col_w = 270.0;
        let cols = ((ui.available_width() / col_w).floor() as usize).max(1);
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                let mut any = false;
                for cat in categories {
                    let visible: Vec<&Item> = items
                        .iter()
                        .filter(|it| it.category == cat)
                        .filter(|it| {
                            q.is_empty()
                                || it.name.to_lowercase().contains(&q)
                                || it.id.contains(&q)
                                || it.tooltip.to_lowercase().contains(&q)
                        })
                        .collect();
                    if visible.is_empty() {
                        continue;
                    }
                    any = true;
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(cat)
                            .strong()
                            .size(16.0)
                            .color(Color32::from_rgb(90, 170, 220)),
                    );
                    ui.separator();
                    egui::Grid::new(cat)
                        .num_columns(cols)
                        .min_col_width(col_w - 10.0)
                        .spacing([10.0, 6.0])
                        .show(ui, |ui| {
                            for (n, it) in visible.iter().enumerate() {
                                let mut on = self.selected.contains(&it.id);
                                ui.horizontal(|ui| {
                                    let resp =
                                        ui.checkbox(&mut on, &it.name).on_hover_text(&it.tooltip);
                                    if resp.changed() {
                                        if on {
                                            self.selected.insert(it.id.clone());
                                        } else {
                                            self.selected.remove(&it.id);
                                        }
                                    }
                                    match self.applied.get(&it.id) {
                                        Some(true) => {
                                            ui.label(
                                                RichText::new("✔ on")
                                                    .small()
                                                    .color(Color32::from_rgb(90, 190, 110)),
                                            );
                                        }
                                        Some(false) => {
                                            ui.label(RichText::new("off").small().weak());
                                        }
                                        None if it.has_check => {
                                            ui.label(RichText::new("…").small().weak());
                                        }
                                        None => {}
                                    }
                                });
                                if (n + 1) % cols == 0 {
                                    ui.end_row();
                                }
                            }
                        });
                }
                if !any {
                    ui.label("No items match.");
                }
            });
    }

    fn ui_output(&mut self, ui: &mut egui::Ui) {
        let Some(run) = &mut self.run else { return };
        ui.add_space(4.0);
        ui.columns(2, |cols| {
            let ui = &mut cols[0];
            ui.label(RichText::new("Steps").strong());
            egui::ScrollArea::vertical()
                .id_salt("steps")
                .show(ui, |ui| {
                    for (label, state) in run.labels.iter().zip(&run.states) {
                        let (icon, color) = match state {
                            StepState::Pending => ("○", Color32::GRAY),
                            StepState::Running => ("▶", Color32::from_rgb(230, 180, 60)),
                            StepState::Ok => ("✔", Color32::from_rgb(90, 190, 110)),
                            StepState::Failed => ("✖", Color32::from_rgb(220, 80, 80)),
                        };
                        ui.label(RichText::new(format!("{icon} {label}")).color(color));
                    }
                    if let Some((failed, log)) = &run.done {
                        ui.add_space(6.0);
                        if *failed == 0 {
                            ui.label(
                                RichText::new("All steps succeeded.")
                                    .strong()
                                    .color(Color32::from_rgb(90, 190, 110)),
                            );
                        } else {
                            ui.label(
                                RichText::new(format!("{failed} step(s) failed."))
                                    .strong()
                                    .color(Color32::from_rgb(220, 80, 80)),
                            );
                        }
                        if let Some(log) = log {
                            ui.label(
                                RichText::new(format!("Log: {}", log.display()))
                                    .small()
                                    .weak(),
                            );
                        }
                    }
                });
            let ui = &mut cols[1];
            ui.label(RichText::new("Output").strong());
            egui::ScrollArea::both()
                .id_salt("output")
                .stick_to_bottom(true)
                .auto_shrink(false)
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(RichText::new(run.output.as_str()).monospace().size(12.0))
                            .extend(),
                    );
                });
        });
    }

    fn ui_confirm(&mut self, ctx: &egui::Context) {
        let Some((plan, mode)) = &self.confirm else {
            return;
        };
        let mut decision = None;
        let title = match mode {
            Mode::Apply => "Confirm",
            Mode::Revert => "Confirm revert / uninstall",
        };
        egui::Modal::new(egui::Id::unique("nx-confirm")).show(ctx, |ui| {
            ui.set_width(640.0);
            ui.heading(title);
            ui.add_space(4.0);
            for c in &plan.confirmations {
                ui.label(RichText::new(format!("⚠ {c}")).color(Color32::from_rgb(230, 180, 60)));
            }
            for w in &plan.warnings {
                ui.label(
                    RichText::new(format!("warning: {w}")).color(Color32::from_rgb(230, 180, 60)),
                );
            }
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .max_height(380.0)
                .show(ui, |ui| {
                    for (i, s) in plan.steps.iter().enumerate() {
                        ui.label(
                            RichText::new(format!(
                                "{}. {}  ({})",
                                i + 1,
                                s.label,
                                if s.root { "root" } else { "user" }
                            ))
                            .strong(),
                        );
                        ui.label(RichText::new(&s.command).monospace().size(12.0).weak());
                        ui.add_space(4.0);
                    }
                });
            if plan.needs_root() && !nx_core::privilege::is_root() {
                ui.label(RichText::new("You will be asked for your password once.").weak());
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(RichText::new("Run").strong()).clicked() {
                    decision = Some(true);
                }
                if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    decision = Some(false);
                }
            });
        });
        match decision {
            Some(true) => {
                let (plan, _) = self.confirm.take().unwrap();
                self.start(plan);
            }
            Some(false) => self.confirm = None,
            None => {}
        }
    }
}

impl eframe::App for GuiApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.poll() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        egui::Panel::top("header").show(ui, |ui| self.ui_header(ui));
        egui::Panel::bottom("footer").show(ui, |ui| self.ui_footer(ui));
        if self.show_output && self.run.is_some() {
            egui::Panel::bottom("output")
                .resizable(true)
                .default_size(260.0)
                .min_size(120.0)
                .show(ui, |ui| self.ui_output(ui));
        }
        egui::CentralPanel::default().show(ui, |ui| self.ui_items(ui));
        self.ui_confirm(&ctx);
    }
}
