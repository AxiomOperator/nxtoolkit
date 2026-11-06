//! Interactive terminal UI.

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::Duration,
};

use crossbeam_channel::Receiver;
use nx_core::{ActionKind, Elevation, Event, Mode, Plan, Toolkit, catalog::InstallMethod};
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event as CEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Cell, Clear, Paragraph, Row, Table, TableState, Tabs, Wrap},
};

const TABS: [&str; 4] = ["Install", "Tweaks", "Maintenance", "Setup"];
const MAX_OUTPUT: usize = 5000;

struct Item {
    id: String,
    name: String,
    category: String,
    description: String,
    /// Extra detail lines (install method, privileges, ...).
    details: Vec<String>,
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
    output: Vec<String>,
    rx: Receiver<Event>,
    /// Lines scrolled up from the bottom (0 = follow).
    scroll_back: usize,
    done: Option<(usize, Option<PathBuf>)>,
}

enum Screen {
    Browse,
    Confirm { plan: Plan, mode: Mode, scroll: u16 },
    Running,
    Help,
}

struct App {
    tk: Toolkit,
    simulated: bool,
    tab: usize,
    items: [Vec<Item>; 4],
    table: [TableState; 4],
    selected: HashSet<String>,
    filter: String,
    filtering: bool,
    screen: Screen,
    run: Option<RunState>,
    applied: HashMap<String, bool>,
    checks: Option<Receiver<(String, bool)>>,
    message: Option<String>,
    quit: bool,
}

pub fn run(tk: Toolkit, simulated: bool) -> anyhow::Result<()> {
    let mut app = App::new(tk, simulated);
    let mut terminal = ratatui::init();
    let result = app.event_loop(&mut terminal);
    ratatui::restore();
    result
}

impl App {
    fn new(tk: Toolkit, simulated: bool) -> Self {
        let family = tk.distro.family;
        let has_aur = tk.aur.is_some();
        let apps = tk
            .catalog
            .apps
            .iter()
            .map(|a| {
                let how = match a.install_method(family, has_aur) {
                    Some(InstallMethod::Native(p)) => {
                        format!(
                            "Installs via {}: {}",
                            nx_core::pkg::native(family).name(),
                            p.join(" ")
                        )
                    }
                    Some(InstallMethod::Aur(p)) => format!("Installs from AUR: {}", p.join(" ")),
                    Some(InstallMethod::Flatpak(p)) => {
                        format!("Installs from Flathub: {}", p.join(" "))
                    }
                    None => "Not available".into(),
                };
                Item {
                    id: a.id.clone(),
                    name: a.name.clone(),
                    category: a.category.clone(),
                    description: a.description.clone(),
                    details: vec![how],
                    has_check: false,
                }
            })
            .collect();
        let actions = |kind: ActionKind| -> Vec<Item> {
            tk.catalog
                .actions_of(kind)
                .map(|a| {
                    let mut details = vec![format!(
                        "Runs as {}{}",
                        if a.root { "root" } else { "your user" },
                        if a.revert.is_some() {
                            " · revertible (r)"
                        } else {
                            ""
                        }
                    )];
                    if let Some(c) = &a.confirm {
                        details.push(format!("⚠ {c}"));
                    }
                    Item {
                        id: a.id.clone(),
                        name: a.name.clone(),
                        category: a.category.clone(),
                        description: a.description.clone(),
                        details,
                        has_check: a.check.is_some(),
                    }
                })
                .collect()
        };
        let items = [
            apps,
            actions(ActionKind::Tweak),
            actions(ActionKind::Maintenance),
            actions(ActionKind::Setup),
        ];
        let table = std::array::from_fn(|i| {
            let mut s = TableState::default();
            if !items[i].is_empty() {
                s.select(Some(0));
            }
            s
        });
        let checks = Some(tk.spawn_checks());
        App {
            tk,
            simulated,
            tab: 0,
            items,
            table,
            selected: HashSet::new(),
            filter: String::new(),
            filtering: false,
            screen: Screen::Browse,
            run: None,
            applied: HashMap::new(),
            checks,
            message: None,
            quit: false,
        }
    }

    fn event_loop(&mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        while !self.quit {
            self.drain_channels();
            terminal.draw(|f| self.draw(f))?;
            if event::poll(Duration::from_millis(50))?
                && let CEvent::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                self.on_key(key, terminal)?;
            }
        }
        Ok(())
    }

    fn drain_channels(&mut self) {
        if let Some(rx) = &self.checks {
            for (id, ok) in rx.try_iter() {
                self.applied.insert(id, ok);
            }
        }
        let mut finished = false;
        if let Some(run) = &mut self.run {
            for ev in run.rx.try_iter() {
                match ev {
                    Event::StepStarted { index } => {
                        run.states[index] = StepState::Running;
                        run.output.push(format!("==> {}", run.labels[index]));
                    }
                    Event::Output { line } => run.output.push(line),
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
            if run.output.len() > MAX_OUTPUT {
                let excess = run.output.len() - MAX_OUTPUT;
                run.output.drain(..excess);
            }
        }
        if finished {
            // Re-run checks so tweak status reflects the changes.
            self.applied.clear();
            self.checks = Some(self.tk.spawn_checks());
        }
    }

    // ── input ───────────────────────────────────────────────────────────────

    fn on_key(&mut self, key: KeyEvent, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            let running = self.run.as_ref().is_some_and(|r| r.done.is_none());
            if !running {
                self.quit = true;
            }
            return Ok(());
        }
        match &mut self.screen {
            Screen::Help => self.screen = Screen::Browse,
            Screen::Confirm { scroll, .. } => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => self.start_run(terminal)?,
                KeyCode::Char('n') | KeyCode::Esc | KeyCode::Char('q') => {
                    self.screen = Screen::Browse
                }
                KeyCode::Down | KeyCode::Char('j') => *scroll = scroll.saturating_add(1),
                KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
                _ => {}
            },
            Screen::Running => {
                let Some(run) = &mut self.run else {
                    self.screen = Screen::Browse;
                    return Ok(());
                };
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => run.scroll_back += 1,
                    KeyCode::Down | KeyCode::Char('j') => {
                        run.scroll_back = run.scroll_back.saturating_sub(1)
                    }
                    KeyCode::PageUp => run.scroll_back += 20,
                    KeyCode::PageDown => run.scroll_back = run.scroll_back.saturating_sub(20),
                    KeyCode::End | KeyCode::Char('G') => run.scroll_back = 0,
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') if run.done.is_some() => {
                        if run.done.as_ref().is_some_and(|(f, _)| *f == 0) {
                            self.selected.clear();
                        }
                        self.screen = Screen::Browse;
                    }
                    _ => {}
                }
            }
            Screen::Browse if self.filtering => match key.code {
                KeyCode::Esc => {
                    self.filtering = false;
                    self.filter.clear();
                    self.clamp_cursor();
                }
                KeyCode::Enter => self.filtering = false,
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.clamp_cursor();
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                    self.clamp_cursor();
                }
                KeyCode::Up | KeyCode::Down => {
                    self.move_cursor(if key.code == KeyCode::Up { -1 } else { 1 })
                }
                _ => {}
            },
            Screen::Browse => {
                self.message = None;
                match key.code {
                    KeyCode::Char('q') => self.quit = true,
                    KeyCode::Char('?') => self.screen = Screen::Help,
                    KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
                        self.set_tab((self.tab + 1) % TABS.len())
                    }
                    KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
                        self.set_tab((self.tab + TABS.len() - 1) % TABS.len())
                    }
                    KeyCode::Char(c @ '1'..='4') => self.set_tab(c as usize - '1' as usize),
                    KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
                    KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
                    KeyCode::PageDown => self.move_cursor(10),
                    KeyCode::PageUp => self.move_cursor(-10),
                    KeyCode::Home | KeyCode::Char('g') => self.move_cursor(i32::MIN / 2),
                    KeyCode::End | KeyCode::Char('G') => self.move_cursor(i32::MAX / 2),
                    KeyCode::Char(' ') => self.toggle_current(),
                    KeyCode::Char('a') => self.toggle_all_visible(),
                    KeyCode::Char('c') => self.selected.clear(),
                    KeyCode::Char('/') => self.filtering = true,
                    KeyCode::Esc => {
                        self.filter.clear();
                        self.clamp_cursor();
                    }
                    KeyCode::Enter | KeyCode::Char('i') => self.prepare(Mode::Apply),
                    KeyCode::Char('r') | KeyCode::Char('u') => self.prepare(Mode::Revert),
                    KeyCode::Char('o') if self.run.is_some() => self.screen = Screen::Running,
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn visible(&self, tab: usize) -> Vec<usize> {
        let f = self.filter.to_lowercase();
        self.items[tab]
            .iter()
            .enumerate()
            .filter(|(_, it)| {
                f.is_empty()
                    || [&it.name, &it.id, &it.category, &it.description]
                        .iter()
                        .any(|s| s.to_lowercase().contains(&f))
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn current(&self) -> Option<&Item> {
        let vis = self.visible(self.tab);
        let row = self.table[self.tab].selected()?;
        vis.get(row).map(|&i| &self.items[self.tab][i])
    }

    fn set_tab(&mut self, t: usize) {
        self.tab = t;
        self.clamp_cursor();
    }

    fn clamp_cursor(&mut self) {
        let n = self.visible(self.tab).len();
        let st = &mut self.table[self.tab];
        st.select(match (n, st.selected()) {
            (0, _) => None,
            (n, Some(s)) => Some(s.min(n - 1)),
            (_, None) => Some(0),
        });
    }

    fn move_cursor(&mut self, delta: i32) {
        let n = self.visible(self.tab).len() as i64;
        if n == 0 {
            return;
        }
        let st = &mut self.table[self.tab];
        let cur = st.selected().unwrap_or(0) as i64;
        st.select(Some((cur + delta as i64).clamp(0, n - 1) as usize));
    }

    fn toggle_current(&mut self) {
        if let Some(id) = self.current().map(|i| i.id.clone()) {
            if !self.selected.remove(&id) {
                self.selected.insert(id);
            }
            self.move_cursor(1);
        }
    }

    fn toggle_all_visible(&mut self) {
        let ids: Vec<String> = self
            .visible(self.tab)
            .into_iter()
            .map(|i| self.items[self.tab][i].id.clone())
            .collect();
        if ids.iter().all(|id| self.selected.contains(id)) {
            for id in &ids {
                self.selected.remove(id);
            }
        } else {
            self.selected.extend(ids);
        }
    }

    fn prepare(&mut self, mode: Mode) {
        let mut ids: Vec<String> = self.selected.iter().cloned().collect();
        if ids.is_empty() {
            // Act on the highlighted item when nothing is selected.
            match self.current() {
                Some(it) => ids.push(it.id.clone()),
                None => return,
            }
        }
        match self.tk.plan(&ids, mode) {
            Ok(plan) if plan.is_empty() => {
                self.message = Some(
                    plan.warnings
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "nothing to do".into()),
                );
            }
            Ok(plan) => {
                self.screen = Screen::Confirm {
                    plan,
                    mode,
                    scroll: 0,
                }
            }
            Err(e) => self.message = Some(e.to_string()),
        }
    }

    fn start_run(&mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        let Screen::Confirm { plan, .. } = std::mem::replace(&mut self.screen, Screen::Browse)
        else {
            return Ok(());
        };
        if self.simulated {
            self.message = Some("simulated distro (--distro): runs are disabled".into());
            return Ok(());
        }
        let elevation = Elevation::for_frontend(false);
        if plan.needs_root() && elevation == Elevation::Sudo {
            // Leave the alternate screen so sudo can prompt normally.
            ratatui::restore();
            println!("nxtoolkit needs administrator rights for this run.");
            let ok = elevation.prepare().unwrap_or(false);
            *terminal = ratatui::init();
            terminal.clear()?;
            if !ok {
                self.message = Some("sudo authentication failed".into());
                return Ok(());
            }
        }
        let labels: Vec<String> = plan.steps.iter().map(|s| s.label.clone()).collect();
        let n = labels.len();
        let rx = self.tk.spawn_run(plan, elevation);
        self.run = Some(RunState {
            labels,
            states: vec![StepState::Pending; n],
            output: Vec::new(),
            rx,
            scroll_back: 0,
            done: None,
        });
        self.screen = Screen::Running;
        Ok(())
    }

    // ── drawing ─────────────────────────────────────────────────────────────

    fn draw(&mut self, f: &mut Frame) {
        let [header, body, footer] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .areas(f.area());
        self.draw_header(f, header);
        match self.screen {
            Screen::Running => self.draw_run(f, body),
            _ => self.draw_browse(f, body),
        }
        self.draw_footer(f, footer);
        match &self.screen {
            Screen::Confirm { plan, mode, scroll } => draw_confirm(f, body, plan, *mode, *scroll),
            Screen::Help => draw_help(f, body),
            _ => {}
        }
    }

    fn draw_header(&self, f: &mut Frame, area: Rect) {
        let d = &self.tk.distro;
        let title = Line::from(vec![
            Span::styled(
                " nxtoolkit ",
                Style::new().bold().fg(Color::Black).bg(Color::Cyan),
            ),
            Span::raw(format!(" {} ", d.name)),
            Span::styled(format!("[{}]", d.family), Style::new().fg(Color::DarkGray)),
        ]);
        let titles = TABS.iter().enumerate().map(|(i, t)| {
            let n = self.items[i]
                .iter()
                .filter(|it| self.selected.contains(&it.id))
                .count();
            if n > 0 {
                format!("{} ({n})", t)
            } else {
                t.to_string()
            }
        });
        let tabs = Tabs::new(titles)
            .select(self.tab)
            .highlight_style(
                Style::new()
                    .bold()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::REVERSED),
            )
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .title(title),
            );
        f.render_widget(tabs, area);
    }

    fn draw_browse(&mut self, f: &mut Frame, area: Rect) {
        let [list_area, detail_area] =
            Layout::horizontal([Constraint::Percentage(62), Constraint::Percentage(38)])
                .areas(area);
        let vis = self.visible(self.tab);
        let rows: Vec<Row> = vis
            .iter()
            .map(|&i| {
                let it = &self.items[self.tab][i];
                let mark = if self.selected.contains(&it.id) {
                    "[x]"
                } else {
                    "[ ]"
                };
                let status = match self.applied.get(&it.id) {
                    Some(true) => Span::styled("✓ on", Style::new().fg(Color::Green)),
                    Some(false) => Span::styled("off", Style::new().fg(Color::DarkGray)),
                    None if it.has_check => Span::styled("…", Style::new().fg(Color::DarkGray)),
                    None => Span::raw(""),
                };
                let style = if self.selected.contains(&it.id) {
                    Style::new().fg(Color::Yellow)
                } else {
                    Style::new()
                };
                Row::new(vec![
                    Cell::from(mark),
                    Cell::from(it.name.clone()),
                    Cell::from(Span::styled(
                        it.category.clone(),
                        Style::new().fg(Color::DarkGray),
                    )),
                    Cell::from(status),
                ])
                .style(style)
            })
            .collect();
        let title = if self.filtering || !self.filter.is_empty() {
            format!(
                " {} — filter: {}{} ",
                TABS[self.tab],
                self.filter,
                if self.filtering { "▏" } else { "" }
            )
        } else {
            format!(" {} ", TABS[self.tab])
        };
        let table = Table::new(
            rows,
            [
                Constraint::Length(3),
                Constraint::Fill(3),
                Constraint::Fill(2),
                Constraint::Length(5),
            ],
        )
        .block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .title(title),
        )
        .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol("› ");
        f.render_stateful_widget(table, list_area, &mut self.table[self.tab]);

        let detail =
            match self.current() {
                Some(it) => {
                    let mut lines = vec![
                        Line::from(it.name.clone().bold()),
                        Line::from(Span::styled(
                            format!("{} · {}", it.category, it.id),
                            Style::new().fg(Color::DarkGray),
                        )),
                        Line::raw(""),
                        Line::raw(it.description.clone()),
                        Line::raw(""),
                    ];
                    lines.extend(it.details.iter().map(|d| {
                        Line::from(Span::styled(d.clone(), Style::new().fg(Color::Cyan)))
                    }));
                    if let Some(ok) = self.applied.get(&it.id) {
                        lines.push(Line::raw(if *ok {
                            "Status: applied"
                        } else {
                            "Status: not applied"
                        }));
                    }
                    Text::from(lines)
                }
                None => Text::raw("No items match."),
            };
        f.render_widget(
            Paragraph::new(detail).wrap(Wrap { trim: false }).block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .title(" Details "),
            ),
            detail_area,
        );
    }

    fn draw_run(&self, f: &mut Frame, area: Rect) {
        let Some(run) = &self.run else { return };
        let [steps_area, out_area] =
            Layout::horizontal([Constraint::Percentage(32), Constraint::Percentage(68)])
                .areas(area);
        let lines: Vec<Line> = run
            .labels
            .iter()
            .zip(&run.states)
            .map(|(l, s)| {
                let (icon, style) = match s {
                    StepState::Pending => ("·", Style::new().fg(Color::DarkGray)),
                    StepState::Running => ("▶", Style::new().fg(Color::Yellow).bold()),
                    StepState::Ok => ("✓", Style::new().fg(Color::Green)),
                    StepState::Failed => ("✗", Style::new().fg(Color::Red).bold()),
                };
                Line::from(vec![
                    Span::styled(format!("{icon} "), style),
                    Span::styled(l.clone(), style),
                ])
            })
            .collect();
        let mut step_text = Text::from(lines);
        if let Some((failed, log)) = &run.done {
            step_text.push_line(Line::raw(""));
            step_text.push_line(if *failed == 0 {
                Line::styled("All steps succeeded.", Style::new().fg(Color::Green).bold())
            } else {
                Line::styled(
                    format!("{failed} step(s) failed."),
                    Style::new().fg(Color::Red).bold(),
                )
            });
            if let Some(log) = log {
                step_text.push_line(Line::styled(
                    format!("Log: {}", log.display()),
                    Style::new().fg(Color::DarkGray),
                ));
            }
            step_text.push_line(Line::styled(
                "Press Enter to return.",
                Style::new().fg(Color::Cyan),
            ));
        }
        f.render_widget(
            Paragraph::new(step_text).wrap(Wrap { trim: false }).block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .title(" Steps "),
            ),
            steps_area,
        );

        let height = out_area.height.saturating_sub(2) as usize;
        let end = run
            .output
            .len()
            .saturating_sub(run.scroll_back.min(run.output.len()));
        let start = end.saturating_sub(height);
        let out: Vec<Line> = run.output[start..end]
            .iter()
            .map(|l| {
                if l.starts_with("==> ") {
                    Line::styled(l.clone(), Style::new().bold().fg(Color::Cyan))
                } else {
                    Line::raw(l.clone())
                }
            })
            .collect();
        let title = if run.scroll_back > 0 {
            " Output (scrolled — End to follow) "
        } else {
            " Output "
        };
        f.render_widget(
            Paragraph::new(out).block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .title(title),
            ),
            out_area,
        );
    }

    fn draw_footer(&self, f: &mut Frame, area: Rect) {
        let hint = match &self.screen {
            Screen::Browse if self.filtering => {
                "type to filter · Enter keep · Esc clear".to_string()
            }
            Screen::Browse => format!(
                "space select · a all · / filter · Enter apply · r revert/uninstall · ? help · q quit   {} selected",
                self.selected.len()
            ),
            Screen::Confirm { .. } => "y/Enter run · n/Esc cancel · ↑↓ scroll".into(),
            Screen::Running => {
                "↑↓/PgUp/PgDn scroll · End follow · Enter return (when finished)".into()
            }
            Screen::Help => "any key to close".into(),
        };
        let line = match &self.message {
            Some(m) => Line::styled(format!(" {m}"), Style::new().fg(Color::Yellow).bold()),
            None => Line::styled(format!(" {hint}"), Style::new().fg(Color::DarkGray)),
        };
        f.render_widget(Paragraph::new(line), area);
    }
}

fn popup(area: Rect, w_pct: u16, h_pct: u16) -> Rect {
    let [_, v, _] = Layout::vertical([
        Constraint::Percentage((100 - h_pct) / 2),
        Constraint::Percentage(h_pct),
        Constraint::Fill(1),
    ])
    .areas(area);
    let [_, h, _] = Layout::horizontal([
        Constraint::Percentage((100 - w_pct) / 2),
        Constraint::Percentage(w_pct),
        Constraint::Fill(1),
    ])
    .areas(v);
    h
}

fn draw_confirm(f: &mut Frame, area: Rect, plan: &Plan, mode: Mode, scroll: u16) {
    let rect = popup(area, 86, 86);
    f.render_widget(Clear, rect);
    let mut lines: Vec<Line> = Vec::new();
    for c in &plan.confirmations {
        lines.push(Line::styled(
            format!("⚠ {c}"),
            Style::new().fg(Color::Yellow),
        ));
    }
    for w in &plan.warnings {
        lines.push(Line::styled(
            format!("warning: {w}"),
            Style::new().fg(Color::Yellow),
        ));
    }
    if !lines.is_empty() {
        lines.push(Line::raw(""));
    }
    for (i, s) in plan.steps.iter().enumerate() {
        lines.push(Line::from(vec![
            Span::styled(format!("{}. {}", i + 1, s.label), Style::new().bold()),
            Span::styled(
                if s.root { "  (root)" } else { "  (user)" },
                Style::new().fg(Color::DarkGray),
            ),
        ]));
        for c in s.command.lines() {
            lines.push(Line::styled(
                format!("   $ {c}"),
                Style::new().fg(Color::DarkGray),
            ));
        }
    }
    let title = match mode {
        Mode::Apply => " Confirm: apply / install ",
        Mode::Revert => " Confirm: revert / uninstall ",
    };
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0))
            .block(
                Block::bordered()
                    .border_type(BorderType::Double)
                    .border_style(Style::new().fg(Color::Cyan))
                    .title(title)
                    .title_bottom(Line::from(" [y] run   [n] cancel ").centered()),
            ),
        rect,
    );
}

fn draw_help(f: &mut Frame, area: Rect) {
    let rect = popup(area, 60, 70);
    f.render_widget(Clear, rect);
    let rows = [
        ("Tab / ← →  / 1-4", "switch tab"),
        ("↑ ↓ / j k", "move"),
        ("Space", "select / deselect"),
        ("a", "select all visible (toggle)"),
        ("c", "clear selection"),
        ("/", "filter by name, id, category"),
        ("Enter / i", "install / apply selection (or highlighted)"),
        ("r / u", "uninstall / revert selection"),
        ("o", "show last run output"),
        ("q", "quit"),
    ];
    let lines: Vec<Line> = rows
        .iter()
        .map(|(k, v)| {
            Line::from(vec![
                Span::styled(format!("{k:<20}"), Style::new().fg(Color::Cyan)),
                Span::raw(*v),
            ])
        })
        .collect();
    f.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .title(" Keys "),
        ),
        rect,
    );
}
