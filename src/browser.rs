//! Keyboard-driven inspection of saved artifacts. This view never executes mutations.
use std::{
    collections::BTreeSet,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use clap::ValueEnum;
use crossterm::{
    cursor::Show,
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind,
        KeyModifiers,
    },
    execute,
    style::ResetColor,
    terminal::{LeaveAlternateScreen, disable_raw_mode},
};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::{
    cancellation::Cancellation,
    mutation::Operator,
    report::{ExecutionState, Outcome},
    report_reader::{LoadedReport, SavedExecution, clean_text},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResultFilter {
    All,
    Missed,
    Killed,
    Timeout,
    BuildFailed,
    Error,
    Pending,
    Interrupted,
}

impl ResultFilter {
    const ALL: [Self; 8] = [
        Self::All,
        Self::Missed,
        Self::Killed,
        Self::Timeout,
        Self::BuildFailed,
        Self::Error,
        Self::Pending,
        Self::Interrupted,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Missed => "missed",
            Self::Killed => "killed",
            Self::Timeout => "timeout",
            Self::BuildFailed => "build failed",
            Self::Error => "error",
            Self::Pending => "pending",
            Self::Interrupted => "interrupted",
        }
    }
    fn matches(self, execution: &SavedExecution) -> bool {
        self == Self::All
            || match self {
                Self::Missed => execution.outcome == Some(Outcome::Missed),
                Self::Killed => execution.outcome == Some(Outcome::Killed),
                Self::Timeout => execution.outcome == Some(Outcome::Timeout),
                Self::BuildFailed => execution.outcome == Some(Outcome::BuildFailed),
                Self::Error => execution.outcome == Some(Outcome::Error),
                Self::Pending => execution.state == ExecutionState::Pending,
                Self::Interrupted => execution.state == ExecutionState::Interrupted,
                Self::All => true,
            }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReviewFilter {
    All,
    Undismissed,
    Dismissed,
}

impl ReviewFilter {
    fn next(self) -> Self {
        match self {
            Self::All => Self::Undismissed,
            Self::Undismissed => Self::Dismissed,
            Self::Dismissed => Self::All,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Undismissed => "undismissed",
            Self::Dismissed => "dismissed",
        }
    }
    fn matches(self, dismissed: bool) -> bool {
        match self {
            Self::All => true,
            Self::Undismissed => !dismissed,
            Self::Dismissed => dismissed,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    Diff,
    Log,
    Baseline,
    Discovery,
    Command,
    Help,
    Dismissals,
    Score,
    Operators,
}

impl View {
    fn label(self) -> &'static str {
        match self {
            Self::Diff => "Diff",
            Self::Log => "Mutation log",
            Self::Baseline => "Baseline log",
            Self::Discovery => "Discovery log",
            Self::Command => "Rerun command",
            Self::Help => "Help",
            Self::Dismissals => "Saved dismissal review",
            Self::Score => "Mutation score",
            Self::Operators => "Operator breakdown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    List,
    Detail,
}

struct Browser {
    report: LoadedReport,
    module: Option<PathBuf>,
    visible: Vec<usize>,
    list: ListState,
    marked: BTreeSet<usize>,
    result: ResultFilter,
    review: ReviewFilter,
    operator: Option<Operator>,
    file_filter: String,
    draft: Option<String>,
    focus: Focus,
    view: View,
    detail_key: Option<(View, Option<usize>)>,
    detail: Vec<String>,
    scroll: usize,
    horizontal: u16,
    page_rows: usize,
    export: Option<String>,
}

impl Browser {
    fn new(report: LoadedReport, module: Option<PathBuf>) -> Self {
        let view = if report.mutations.is_empty() {
            View::Baseline
        } else {
            View::Diff
        };
        let mut app = Self {
            report,
            module,
            visible: Vec::new(),
            list: ListState::default(),
            marked: BTreeSet::new(),
            result: ResultFilter::All,
            review: ReviewFilter::All,
            operator: None,
            file_filter: String::new(),
            draft: None,
            focus: Focus::List,
            view,
            detail_key: None,
            detail: Vec::new(),
            scroll: 0,
            horizontal: 0,
            page_rows: 10,
            export: None,
        };
        app.refilter();
        app
    }

    fn selected(&self) -> Option<usize> {
        self.list
            .selected()
            .and_then(|index| self.visible.get(index))
            .copied()
    }

    fn refilter(&mut self) {
        let selected = self.selected();
        let query = self.file_filter.to_lowercase();
        self.visible = self
            .report
            .mutations
            .iter()
            .enumerate()
            .filter(|(index, mutation)| {
                self.result.matches(&self.report.executions[*index])
                    && self
                        .review
                        .matches(self.report.dismissal(mutation.id).is_some())
                    && self
                        .operator
                        .is_none_or(|operator| operator == mutation.operator)
                    && mutation
                        .file
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&query)
            })
            .map(|(index, _)| index)
            .collect();
        self.list.select(
            selected
                .and_then(|id| self.visible.iter().position(|value| *value == id))
                .or_else(|| (!self.visible.is_empty()).then_some(0)),
        );
        if self.view == View::Command {
            self.detail_key = None;
        }
    }

    fn command(&self) -> Result<String> {
        self.report.run.validate_rerun_scope()?;
        ensure!(
            self.report.complete,
            "This run has no complete mutation inventory to rerun."
        );
        let ids = if self.marked.is_empty() {
            vec![
                self.report.mutations[self
                    .selected()
                    .context("Select or mark a mutation first.")?]
                .id,
            ]
        } else {
            self.marked.iter().copied().collect()
        };
        let module = self.module.as_deref().or(self.report.run.module_root.as_deref())
            .context("No module path was recorded. Reopen with --module PATH to generate a rerun command.")?;
        Ok(format!(
            "mutie {}{} --rerun {} --id {}",
            quote_path(module)?,
            match self.report.run.language() {
                crate::cli::Lang::Rust => " --lang rust",
                crate::cli::Lang::Vitest => " --lang vitest",
                crate::cli::Lang::Node => " --lang node",
                _ => "",
            },
            quote_path(&self.report.directory)?,
            ids.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ))
    }

    // File access happens only when the selected artifact changes, never inside render.
    fn load_detail(&mut self) {
        let key = (
            self.view,
            if matches!(self.view, View::Diff | View::Log | View::Command) {
                self.selected()
            } else {
                None
            },
        );
        if self.detail_key == Some(key) {
            return;
        }
        self.detail_key = Some(key);
        self.scroll = 0;
        self.horizontal = 0;
        let text = if self.view == View::Help {
            "Keyboard shortcuts\n\nq / Esc        Close browser\nCtrl-C          Interrupt and restore terminal\nTab             Switch list/detail focus\nUp/Down or j/k  Move through rows or detail lines\nPgUp/PgDn       Move one page\nHome/End        First/last row or detail line\nLeft/Right      Scroll detail horizontally\n\no               Cycle outcome/state filter\np               Cycle operator filter\ns               Cycle all/undismissed/dismissed\n/               Edit file substring filter\nEnter / Esc     Apply/cancel file edit\nx               Clear filters\n\nSpace           Mark/unmark current mutation\na               Mark/unmark visible mutations\nc               Clear all marks, including hidden\n\nd               Show saved diff\nl               Show mutation log\nb / g           Show baseline/discovery log\nv               Show saved dismissal review\nm               Show mutation score and threshold\nt               Show operator counts and command time\nr               Show focused rerun command\ne               Close and print rerun command\n?               Show help\n\n[D] marks dismissed mutations; outcomes are unchanged.\nDismissals reflect the saved run, not current metadata.\n\nMarks persist across filters. Commands use all marked\nIDs, including hidden marks, or the current row when\nnone are marked. No command is executed by this browser.\n\nArtifact previews are limited to 256 KiB. Open the\nsaved file directly for its complete contents.".to_owned()
        } else if self.view == View::Operators {
            self.report.operator_summary()
        } else if self.view == View::Score {
            match &self.report.run.score {
                Some(score) => format!(
                    "{}\n{}\n\nScore uses the entire saved run selection.\nBrowser filters and marks do not change it.\nDismissals do not change the raw score.\nDisplay rounds to two decimal places; the\nminimum-score gate compares the exact ratio.\n",
                    score.details(),
                    self.report.run.score_policy.as_ref().unwrap(),
                ),
                None => "Mutation score: not recorded\n\nThis older report has no saved score or\nminimum-score policy. Outcomes remain available.\n".into(),
            }
        } else if self.view == View::Dismissals {
            self.dismissal_review()
        } else if self.view == View::Command {
            match self.command() {
                Ok(command) => format!(
                    "POSIX shell command (not executed):\n\n{command}\n\nUses {} marked IDs (including {} hidden by filters), or the current row if none.\nAdjust the module path for a moved checkout, or reopen with --module PATH.\nA rerun validates current source and runs a fresh baseline.\n\nPress e to close the browser and print this command.",
                    self.marked.len(),
                    self.hidden_marks()
                ),
                Err(error) => error.to_string(),
            }
        } else {
            let artifact = match self.view {
                View::Baseline => self
                    .report
                    .baseline
                    .as_ref()
                    .and_then(|c| c.logs.text.as_deref()),
                View::Discovery => self
                    .report
                    .discovery
                    .as_ref()
                    .and_then(|c| c.logs.text.as_deref()),
                View::Diff => self
                    .selected()
                    .and_then(|i| self.report.mutations[i].diff.as_deref()),
                View::Log => self
                    .selected()
                    .and_then(|i| self.report.executions[i].logs.as_ref())
                    .and_then(|logs| logs.text.as_deref()),
                View::Command | View::Help | View::Dismissals | View::Score | View::Operators => {
                    None
                }
            };
            let preface = match self.view {
                View::Baseline | View::Discovery => {
                    let command = if self.view == View::Baseline {
                        &self.report.baseline
                    } else {
                        &self.report.discovery
                    };
                    format!(
                        "Run: {} | Phase: {}\n{}\n{}\n",
                        self.report.run.status,
                        one_line(self.report.run.phase.as_deref().unwrap_or("unknown")),
                        command
                            .as_ref()
                            .map(|c| format!("Command: {}", one_line(&c.outcome)))
                            .unwrap_or_else(|| "Command did not run.".into()),
                        self.report
                            .run
                            .error
                            .as_deref()
                            .map(clean_text)
                            .unwrap_or_default()
                    )
                }
                _ => self
                    .selected()
                    .map(|index| {
                        let mutation = &self.report.mutations[index];
                        let execution = &self.report.executions[index];
                        format!(
                            "#{} {} | {} | {}\n{}:{}:{}\n{} -> {}\n{}{}\n",
                            mutation.id,
                            execution_label(execution),
                            mutation.operator,
                            execution
                                .duration_seconds
                                .filter(|value| value.is_finite() && *value >= 0.0)
                                .map(|value| format!("{value:.2}s"))
                                .unwrap_or_else(|| "duration unavailable".into()),
                            one_line(&mutation.file.to_string_lossy()),
                            mutation.line,
                            mutation.column,
                            one_line(&mutation.original),
                            one_line(&mutation.replacement),
                            self.report
                                .dismissal(mutation.id)
                                .map(|d| format!(
                                    "Review: dismissed (saved at discovery)\n{}",
                                    d.description()
                                ))
                                .unwrap_or_else(|| if self.report.dismissals.is_some() {
                                    "Review: undismissed (saved at discovery)\n".into()
                                } else {
                                    "Dismissal review: not recorded\n".into()
                                }),
                            execution
                                .error
                                .as_deref()
                                .map(clean_text)
                                .unwrap_or_default()
                        )
                    })
                    .unwrap_or_default(),
            };
            let contents = match artifact {
                Some(path) => match self.report.artifact(path) {
                    Ok(text) => format!(
                        "Artifact: {}\n\n{}",
                        one_line(&path.to_string_lossy()),
                        if text.is_empty() {
                            "[Empty artifact]"
                        } else {
                            &text
                        }
                    ),
                    Err(error) => format!(
                        "Artifact unavailable: {}\n{error:#}",
                        one_line(&path.to_string_lossy())
                    ),
                },
                None => "No artifact recorded for this view.".into(),
            };
            let retention = match self.view {
                View::Baseline => self.report.baseline.as_ref().map(|c| &c.logs),
                View::Discovery => self.report.discovery.as_ref().map(|c| &c.logs),
                View::Log => self
                    .selected()
                    .and_then(|i| self.report.executions[i].logs.as_ref()),
                _ => None,
            }
            .and_then(|logs| logs.retention.as_ref());
            let mut notices = retention.map(|r| r.notices()).unwrap_or_default();
            if !notices.is_empty() {
                notices.push_str("Omitted output is not stored in this report.\n");
            }
            format!("{preface}{notices}{contents}")
        };
        self.detail = clean_text(&text).lines().map(str::to_owned).collect();
    }

    fn dismissal_review(&self) -> String {
        let Some(snapshot) = &self.report.dismissals else {
            return "Dismissal review was not recorded for this run.\nOlder reports and runs that did not reach inventory publication have no saved review.\n\nNo current checkout is read by this browser.".into();
        };
        let missed = self
            .report
            .executions
            .iter()
            .filter(|e| e.outcome == Some(Outcome::Missed));
        let total_missed = missed.clone().count();
        let dismissed_missed = missed
            .filter(|e| self.report.dismissal(e.id).is_some())
            .count();
        let mut text = format!(
            "Dismissals saved at discovery\n{} dismissed; {} need review\nMissed: {} undismissed; {} dismissed\nExecution outcomes and counts are unchanged.\n\nDecisions reflect this run's source snapshot.\nLater repository edits are not read by this browser.\n[D] marks dismissed rows. s cycles the review filter.\nDismissed rows can be marked and rerun.\n\nMatched dismissals:\n",
            snapshot.matched.len(),
            snapshot.needs_review.len(),
            total_missed - dismissed_missed,
            dismissed_missed
        );
        if snapshot.matched.is_empty() {
            text.push_str("None.\n");
        }
        for entry in &snapshot.matched {
            text.push_str(&format!(
                "\nMutation #{}\n{}",
                entry.mutation_id,
                entry.dismissal.description()
            ));
        }
        text.push_str("\nNeeds review (not applied):\n");
        if snapshot.needs_review.is_empty() {
            text.push_str("None.\n");
        }
        for entry in &snapshot.needs_review {
            text.push_str(&format!(
                "\n{}\n{}",
                entry.status.label(),
                entry.dismissal.description()
            ));
        }
        text
    }

    fn hidden_marks(&self) -> usize {
        self.marked.len().saturating_sub(
            self.visible
                .iter()
                .filter(|index| self.marked.contains(&self.report.mutations[**index].id))
                .count(),
        )
    }

    fn navigate(&mut self, forward: bool, amount: usize) {
        match self.focus {
            Focus::List if !self.visible.is_empty() => {
                let index = self.list.selected().unwrap_or(0);
                self.list.select(Some(if forward {
                    index.saturating_add(amount).min(self.visible.len() - 1)
                } else {
                    index.saturating_sub(amount)
                }));
            }
            Focus::Detail => {
                self.scroll = if forward {
                    self.scroll
                        .saturating_add(amount)
                        .min(self.detail.len().saturating_sub(1))
                } else {
                    self.scroll.saturating_sub(amount)
                }
            }
            _ => {}
        }
    }

    fn edit_text(&mut self, text: &str) {
        if let Some(draft) = &mut self.draft {
            let remaining = 256usize.saturating_sub(draft.chars().count());
            draft.extend(
                text.chars()
                    .map(|c| if c.is_control() { ' ' } else { c })
                    .take(remaining),
            );
        }
    }

    /// Returns true only for an explicit exit; release events never act as keys.
    fn handle(&mut self, event: Event) -> bool {
        if let Event::Paste(text) = &event {
            self.edit_text(text);
            return false;
        }
        let Event::Key(key) = event else {
            return false;
        };
        if key.kind == KeyEventKind::Release {
            return false;
        }
        if self.draft.is_some() {
            match key.code {
                KeyCode::Esc => self.draft = None,
                KeyCode::Enter => {
                    self.file_filter = self.draft.take().unwrap();
                    self.refilter();
                }
                KeyCode::Backspace => {
                    self.draft.as_mut().unwrap().pop();
                }
                KeyCode::Char(c)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    self.edit_text(&c.to_string())
                }
                _ => {}
            }
            return false;
        }
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return false;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.navigate(false, 1),
            KeyCode::Down | KeyCode::Char('j') => self.navigate(true, 1),
            KeyCode::PageUp => self.navigate(false, self.page_rows),
            KeyCode::PageDown => self.navigate(true, self.page_rows),
            KeyCode::Home => self.navigate(false, usize::MAX),
            KeyCode::End => self.navigate(true, usize::MAX),
            KeyCode::Left if self.focus == Focus::Detail => {
                self.horizontal = self.horizontal.saturating_sub(8)
            }
            KeyCode::Right if self.focus == Focus::Detail => {
                self.horizontal = self.horizontal.saturating_add(8)
            }
            _ if key.kind == KeyEventKind::Repeat => {}
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = if self.focus == Focus::List {
                    Focus::Detail
                } else {
                    Focus::List
                }
            }
            KeyCode::Char('/') => self.draft = Some(self.file_filter.clone()),
            KeyCode::Char('o') => {
                let index = ResultFilter::ALL
                    .iter()
                    .position(|value| *value == self.result)
                    .unwrap();
                self.result = ResultFilter::ALL[(index + 1) % ResultFilter::ALL.len()];
                self.refilter();
            }
            KeyCode::Char('p') => {
                let operators = Operator::value_variants();
                self.operator = match self.operator {
                    None => operators.first().copied(),
                    Some(value) => operators
                        .iter()
                        .position(|o| *o == value)
                        .and_then(|index| operators.get(index + 1))
                        .copied(),
                };
                self.refilter();
            }
            KeyCode::Char('s') => {
                self.review = self.review.next();
                self.refilter();
            }
            KeyCode::Char('x') => {
                self.result = ResultFilter::All;
                self.review = ReviewFilter::All;
                self.operator = None;
                self.file_filter.clear();
                self.refilter();
            }
            KeyCode::Char(' ') => {
                if let Some(index) = self.selected() {
                    let id = self.report.mutations[index].id;
                    if !self.marked.insert(id) {
                        self.marked.remove(&id);
                    }
                    if self.view == View::Command {
                        self.detail_key = None;
                    }
                }
            }
            KeyCode::Char('a') => {
                let remove = self
                    .visible
                    .iter()
                    .all(|index| self.marked.contains(&self.report.mutations[*index].id));
                for index in &self.visible {
                    let id = self.report.mutations[*index].id;
                    if remove {
                        self.marked.remove(&id);
                    } else {
                        self.marked.insert(id);
                    }
                }
                if self.view == View::Command {
                    self.detail_key = None;
                }
            }
            KeyCode::Char('c') => {
                self.marked.clear();
                if self.view == View::Command {
                    self.detail_key = None;
                }
            }
            KeyCode::Char('d') => self.view = View::Diff,
            KeyCode::Char('?') => {
                self.view = View::Help;
                self.focus = Focus::Detail;
            }
            KeyCode::Char('l') => self.view = View::Log,
            KeyCode::Char('b') => self.view = View::Baseline,
            KeyCode::Char('g') => self.view = View::Discovery,
            KeyCode::Char('v') => {
                self.view = View::Dismissals;
                self.focus = Focus::Detail;
            }
            KeyCode::Char('m') => {
                self.view = View::Score;
                self.focus = Focus::Detail;
            }
            KeyCode::Char('t') => {
                self.view = View::Operators;
                self.focus = Focus::Detail;
            }
            KeyCode::Char('r') => {
                self.view = View::Command;
                self.focus = Focus::Detail;
            }
            KeyCode::Char('e') => {
                self.view = View::Command;
                if let Ok(command) = self.command() {
                    self.export = Some(command);
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        if area.width < 24 || area.height < 12 {
            frame.render_widget(
                Paragraph::new("Report browser: resize to 24x12 or larger; q quits"),
                area,
            );
            return;
        }
        let [heading, filters, body, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(2),
            Constraint::Length(2),
        ])
        .areas(area);
        let total = self.report.mutations.len();
        let missed = self
            .report
            .executions
            .iter()
            .filter(|e| e.outcome == Some(Outcome::Missed))
            .count();
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" mutie report ", Style::new().bold()),
                Span::raw(format!(
                    "{} | {total} mutations | {missed} missed | {}",
                    self.report.run.status,
                    one_line(&self.report.directory.to_string_lossy())
                )),
            ])),
            heading,
        );
        let query = self.draft.as_deref().unwrap_or(&self.file_filter);
        let operator = self
            .operator
            .map(|value| value.to_string())
            .unwrap_or_else(|| "all".into());
        frame.render_widget(
            Paragraph::new(vec![
                Line::raw(format!(
                    " Outcome: {} | Operator: {} | Marked: {} ({} hidden)",
                    self.result.label(),
                    operator,
                    self.marked.len(),
                    self.hidden_marks()
                )),
                Line::raw(format!(
                    " File{}: {}",
                    if self.draft.is_some() {
                        " [editing]"
                    } else {
                        ""
                    },
                    query
                )),
                Line::raw(format!(
                    " Review: {} | {}",
                    self.review.label(),
                    self.report
                        .dismissals
                        .as_ref()
                        .map(|d| format!(
                            "{} dismissed | {} need review (saved)",
                            d.matched.len(),
                            d.needs_review.len()
                        ))
                        .unwrap_or_else(|| "not recorded".into())
                )),
            ]),
            filters,
        );
        if self.draft.is_some() {
            // Display the suffix of long input so the cursor stays visible.
            let prefix = " File: ";
            let width = usize::from(filters.width).saturating_sub(prefix.len() + 1);
            let mut suffix = query;
            while Line::raw(suffix).width() > width && !suffix.is_empty() {
                suffix = &suffix[suffix.chars().next().unwrap().len_utf8()..];
            }
            let input_area = Rect::new(filters.x, filters.y + 1, filters.width, 1);
            frame.render_widget(Paragraph::new(format!("{prefix}{suffix}")), input_area);
            frame.set_cursor_position((
                input_area.x + (prefix.len() + Line::raw(suffix).width()) as u16,
                input_area.y,
            ));
        }
        let [list_area, detail_area] = if area.width >= 90 {
            Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]).areas(body)
        } else {
            let list_height = ((u32::from(body.height) * 3).div_ceil(10) as u16).max(3);
            Layout::vertical([Constraint::Length(list_height), Constraint::Min(2)]).areas(body)
        };
        let list_block = Block::default().borders(Borders::ALL).title(format!(
            " {}Mutations {}/{} ",
            if self.focus == Focus::List { "> " } else { "" },
            self.visible.len(),
            total
        ));
        let detail_block = Block::default().borders(Borders::ALL).title(format!(
            " {}{} | line {}/{} ",
            if self.focus == Focus::Detail {
                "> "
            } else {
                ""
            },
            self.view.label(),
            self.scroll + 1,
            self.detail.len()
        ));
        let detail_inner = detail_block.inner(detail_area);
        self.page_rows = usize::from(if self.focus == Focus::List {
            list_block.inner(list_area).height
        } else {
            detail_inner.height
        })
        .max(1);
        if self.visible.is_empty() {
            frame.render_widget(
                Paragraph::new(if total == 0 {
                    "No mutations recorded. b/g: run logs"
                } else {
                    "No matches. x clears filters."
                })
                .block(list_block),
                list_area,
            );
        } else {
            let items = self
                .visible
                .iter()
                .map(|index| {
                    let mutation = &self.report.mutations[*index];
                    let execution = &self.report.executions[*index];
                    ListItem::new(Line::from(vec![
                        Span::raw(format!(
                            "[{}] {:06} ",
                            if self.marked.contains(&mutation.id) {
                                "x"
                            } else {
                                " "
                            },
                            mutation.id
                        )),
                        Span::styled(
                            format!("{:<12} ", execution_label(execution)),
                            outcome_style(execution.outcome),
                        ),
                        Span::styled(
                            if self.report.dismissal(mutation.id).is_some() {
                                "[D] "
                            } else {
                                ""
                            },
                            Style::new().fg(Color::Cyan),
                        ),
                        Span::raw(format!(
                            "{}:{}",
                            one_line(&mutation.file.to_string_lossy()),
                            mutation.line
                        )),
                    ]))
                })
                .collect::<Vec<_>>();
            frame.render_stateful_widget(
                List::new(items)
                    .block(list_block)
                    .highlight_symbol("> ")
                    .highlight_style(Style::new().add_modifier(Modifier::REVERSED)),
                list_area,
                &mut self.list,
            );
        }
        let lines = self
            .detail
            .iter()
            .skip(self.scroll)
            .take(usize::from(detail_inner.height))
            .map(|line| {
                let style = if self.view == View::Diff && line.starts_with('+') {
                    Style::new().fg(Color::Green)
                } else if self.view == View::Diff && line.starts_with('-') {
                    Style::new().fg(Color::Red)
                } else {
                    Style::default()
                };
                Line::styled(line.as_str(), style)
            })
            .collect::<Vec<_>>();
        frame.render_widget(
            Paragraph::new(lines)
                .block(detail_block)
                .scroll((0, self.horizontal)),
            detail_area,
        );
        let hints = if self.draft.is_some() {
            " Enter apply | Esc cancel | Backspace delete\n File filter: case-insensitive substring"
        } else {
            " q quit | ? help | Tab focus | t totals | m score | v review\n o outcome p operator s review / file x reset | d diff l log r command"
        };
        frame.render_widget(Paragraph::new(hints).style(Style::new().dim()), footer);
    }
}

fn execution_label(execution: &SavedExecution) -> String {
    execution
        .outcome
        .map(|value| value.to_string())
        .unwrap_or_else(|| {
            match execution.state {
                ExecutionState::Pending => "pending",
                ExecutionState::Interrupted => "interrupted",
                ExecutionState::Running => "running",
                ExecutionState::Completed => "completed",
            }
            .into()
        })
}

fn outcome_style(outcome: Option<Outcome>) -> Style {
    Style::new().fg(match outcome {
        Some(Outcome::Killed) => Color::Green,
        Some(Outcome::Missed | Outcome::Error) => Color::Red,
        _ => Color::Yellow,
    })
}

fn one_line(text: &str) -> String {
    clean_text(text).replace('\n', " ")
}

fn quote_path(path: &Path) -> Result<String> {
    let path = path
        .to_str()
        .context("Cannot generate a command for a non-UTF-8 path.")?;
    ensure!(
        !path.is_empty() && !path.chars().any(char::is_control),
        "Cannot generate a command for an empty path or one containing control characters."
    );
    Ok(format!("'{}'", path.replace('\'', "'\\''")))
}

struct BrowserTerminal {
    terminal: DefaultTerminal,
    restored: bool,
}

impl BrowserTerminal {
    fn start() -> Result<Self> {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = restore_terminal(&mut io::stdout());
            previous(info);
        }));
        let terminal = ratatui::try_init().inspect_err(|_| {
            let _ = restore_terminal(&mut io::stdout());
        })?;
        let mut guard = Self {
            terminal,
            restored: false,
        };
        execute!(guard.terminal.backend_mut(), EnableBracketedPaste)?;
        Ok(guard)
    }
    fn restore(&mut self) -> io::Result<()> {
        if self.restored {
            return Ok(());
        }
        self.restored = true;
        restore_terminal(self.terminal.backend_mut())
    }
}

impl Drop for BrowserTerminal {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

// Ratatui 0.30.1's restore stops on the first error. Attempt every inverse,
// including the bracketed paste mode owned by this browser.
fn restore_terminal(writer: &mut impl Write) -> io::Result<()> {
    let raw = disable_raw_mode();
    let paste = execute!(writer, DisableBracketedPaste);
    let screen = execute!(writer, LeaveAlternateScreen);
    let color = execute!(writer, ResetColor);
    let cursor = execute!(writer, Show);
    let flush = writer.flush();
    raw.and(paste).and(screen).and(color).and(cursor).and(flush)
}

pub fn run(directory: &Path, module: Option<&Path>, cancellation: &Cancellation) -> Result<()> {
    let report = LoadedReport::load(directory)?;
    ensure!(
        io::stdin().is_terminal()
            && io::stdout().is_terminal()
            && std::env::var("TERM").is_ok_and(|term| term != "dumb"),
        "report browser requires interactive terminal stdin/stdout and TERM other than dumb; inspect summary.txt for plain output"
    );
    let module = module
        .map(|path| -> Result<PathBuf> {
            ensure!(
                !path.as_os_str().is_empty(),
                "module path must not be empty"
            );
            Ok(std::env::current_dir()?.join(path))
        })
        .transpose()?;
    let mut app = Browser::new(report, module);
    app.load_detail();
    cancellation.check()?;
    let mut terminal = BrowserTerminal::start().context("starting report browser")?;
    let result = (|| -> Result<()> {
        let mut dirty = true;
        loop {
            cancellation.check()?;
            if dirty {
                terminal.terminal.draw(|frame| app.render(frame))?;
                dirty = false;
            }
            if event::poll(Duration::from_millis(100))? {
                let event = event::read()?;
                if let Event::Key(key) = event
                    && key.kind != KeyEventKind::Release
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('c' | 'C'))
                {
                    cancellation.cancel();
                    cancellation.check()?;
                }
                if app.handle(event) {
                    break;
                }
                app.load_detail();
                dirty = true;
            }
        }
        Ok(())
    })();
    let restored = terminal.restore();
    drop(terminal);
    if let Err(error) = restored {
        return match result {
            Err(original) => Err(original.context(format!("restoring browser terminal: {error}"))),
            Ok(()) => Err(error.into()),
        };
    }
    result?;
    if let Some(command) = app.export {
        println!("{command}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;
    use ratatui::{Terminal, TerminalOptions, Viewport, backend::TestBackend, buffer::Buffer};
    use std::fs;

    fn browser() -> (tempfile::TempDir, Browser) {
        let temp = crate::report_reader::fixture();
        let mut app = Browser::new(LoadedReport::load(temp.path()).unwrap(), None);
        app.load_detail();
        (temp, app)
    }

    #[test]
    fn operator_view_keeps_saved_totals_across_filters_marks_and_resizes() {
        let temp = crate::report_reader::fixture_with_dismissals();
        let mut app = Browser::new(LoadedReport::load(temp.path()).unwrap(), None);
        app.report.directory = PathBuf::from("/saved/run");
        app.report.executions[2].duration_seconds = None;
        key(&mut app, KeyCode::Char('t'));
        assert_eq!(app.focus, Focus::Detail);
        let detail = app.detail.clone();
        assert!(
            detail
                .join("\n")
                .contains("Time: 0.25s (1 recorded, 1 missing)")
        );
        assert!(
            detail
                .join("\n")
                .contains("Pending: 1 | Running: 0 | Interrupted: 1")
        );
        for code in [' ', 'o', 'p', 's'] {
            key(&mut app, KeyCode::Char(code));
        }
        assert!(app.visible.is_empty());
        assert_eq!(app.marked.len(), 1);
        assert_eq!(app.detail, detail);
        for (width, height) in [(100, 30), (60, 30), (24, 12), (100, 12), (10, 5), (100, 30)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| app.render(frame)).unwrap();
            let rendered = text(terminal.backend().buffer());
            if width >= 60 && height == 30 {
                assert!(rendered.contains("arithmetic_replace"), "{rendered}");
                assert!(
                    rendered.contains("Time: 0.25s (1 recorded, 0 missing)"),
                    "{rendered}"
                );
                let name = if width == 100 { "wide" } else { "compact" };
                let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("tests/snapshots/report-operators-{name}.txt"));
                if std::env::var_os("MUTIE_UPDATE_SNAPSHOTS").is_some() {
                    fs::write(&path, format!("{rendered}\n")).unwrap();
                }
                assert_eq!(
                    rendered.trim_end(),
                    fs::read_to_string(path).unwrap().trim_end()
                );
            }
        }
        key(&mut app, KeyCode::PageDown);
        assert!(app.scroll > 0);
        key(&mut app, KeyCode::Right);
        assert_eq!(app.horizontal, 8);
        key(&mut app, KeyCode::Home);
        assert_eq!(app.scroll, 0);
        key(&mut app, KeyCode::Char('/'));
        key(&mut app, KeyCode::Char('t'));
        assert_eq!(app.draft.as_deref(), Some("t"));
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.detail, detail);
        key(&mut app, KeyCode::Char('d'));
        key(&mut app, KeyCode::Char('t'));
        assert_eq!(app.horizontal, 0);
        assert_eq!(app.detail, detail);
    }

    #[test]
    fn operator_view_distinguishes_empty_and_unpublished_inventories() {
        let (_temp, mut app) = browser();
        app.report.mutations.clear();
        app.report.executions.clear();
        app.refilter();
        key(&mut app, KeyCode::Char('t'));
        assert!(
            app.detail
                .join("\n")
                .contains("No mutations in this selection")
        );
        app.report.complete = false;
        app.detail_key = None;
        app.load_detail();
        let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();
        assert!(text(terminal.backend().buffer()).contains("inventory was not published"));
    }

    #[test]
    fn score_view_keeps_saved_scope_across_filters_and_resizes() {
        let temp = crate::report_reader::fixture_with_score();
        let mut app = Browser::new(LoadedReport::load(temp.path()).unwrap(), None);
        app.report.directory = PathBuf::from("/saved/run");
        key(&mut app, KeyCode::Char('m'));
        let detail = app.detail.clone();
        assert!(detail.join("\n").contains("Minimum-score policy: failed"));
        key(&mut app, KeyCode::Char('o'));
        key(&mut app, KeyCode::Char('s'));
        assert_eq!(app.detail, detail);
        assert!(app.visible.len() < app.report.mutations.len());
        for (width, height) in [(100, 30), (60, 30), (24, 12), (10, 5), (100, 30)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| app.render(frame)).unwrap();
            let rendered = text(terminal.backend().buffer());
            if width >= 60 {
                assert!(rendered.contains("Mutation score: 66.67%"), "{rendered}");
                assert!(rendered.contains("rerun selection only"), "{rendered}");
                assert!(
                    rendered.contains("Dismissed in score: 2 (included)"),
                    "{rendered}"
                );
                let name = if width == 100 { "wide" } else { "compact" };
                let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("tests/snapshots/report-score-{name}.txt"));
                if std::env::var_os("MUTIE_UPDATE_SNAPSHOTS").is_some() {
                    fs::write(&path, format!("{rendered}\n")).unwrap();
                }
                assert_eq!(
                    rendered.trim_end(),
                    fs::read_to_string(path).unwrap().trim_end()
                );
            }
        }
        let (_temp, mut legacy) = browser();
        key(&mut legacy, KeyCode::Char('m'));
        assert!(legacy.detail.join("\n").contains("not recorded"));
    }

    #[test]
    fn truncated_logs_are_visible_even_when_artifacts_are_missing() {
        let (temp, mut app) = browser();
        let r = crate::capture::Retention {
            total_bytes: 10000,
            head_bytes: 32,
            tail_bytes: 32,
            omitted_bytes: 9936,
        };
        let retention = crate::saved_report::LogRetention {
            text: r.clone(),
            stdout: r.clone(),
            stderr: r,
        };
        app.report.executions[0].logs.as_mut().unwrap().retention = Some(retention.clone());
        app.report.baseline.as_mut().unwrap().logs.retention = Some(retention.clone());
        app.report.discovery.as_mut().unwrap().logs.retention = Some(retention);
        for key_code in ['l', 'b', 'g'] {
            key(&mut app, KeyCode::Char(key_code));
            let detail = app.detail.join("\n");
            assert!(detail.contains("Log truncated: 9936 bytes omitted"));
            assert!(detail.contains("Omitted output is not stored"));
            for (w, h) in [(100, 30), (60, 30)] {
                let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                terminal.draw(|frame| app.render(frame)).unwrap();
                assert!(text(terminal.backend().buffer()).contains("Log truncated"));
            }
        }
        fs::remove_file(temp.path().join("logs/000001.log")).unwrap();
        key(&mut app, KeyCode::Char('l'));
        assert!(app.detail.join("\n").contains("Artifact unavailable"));
        assert!(app.detail.join("\n").contains("9936 bytes omitted"));
    }

    fn key(app: &mut Browser, code: KeyCode) -> bool {
        let quit = app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
        app.load_detail();
        quit
    }

    fn text(buffer: &Buffer) -> String {
        buffer
            .content
            .chunks(usize::from(buffer.area.width).max(1))
            .map(|row| {
                let mut line = String::new();
                let mut column = 0;
                while column < row.len() {
                    let symbol = row[column].symbol();
                    line.push_str(symbol);
                    column += Line::raw(symbol).width().max(1);
                }
                line.trim_end().to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn filters_intersect_and_hidden_marks_keep_saved_ids() {
        let (_temp, mut app) = browser();
        key(&mut app, KeyCode::Char(' '));
        key(&mut app, KeyCode::Down);
        key(&mut app, KeyCode::Char(' '));
        key(&mut app, KeyCode::Char('o'));
        assert_eq!(app.visible, vec![0]);
        assert_eq!(app.hidden_marks(), 1);
        assert!(app.command().unwrap().ends_with("--id 1,2"));
        key(&mut app, KeyCode::Char('p'));
        assert!(app.visible.is_empty());
        key(&mut app, KeyCode::End);
        assert_eq!(app.selected(), None);
        key(&mut app, KeyCode::Char('c'));
        assert!(app.command().is_err());
        key(&mut app, KeyCode::Char('x'));
        key(&mut app, KeyCode::Char('/'));
        app.handle(Event::Paste("目录/B".into()));
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.visible, vec![1]);
        assert!(app.command().unwrap().ends_with("--id 2"));
        key(&mut app, KeyCode::Char('a'));
        assert_eq!(app.marked, BTreeSet::from([2]));
        key(&mut app, KeyCode::Char('a'));
        assert!(app.marked.is_empty());
        app.result = ResultFilter::Pending;
        app.file_filter.clear();
        app.refilter();
        assert_eq!(app.visible, vec![3]);
        app.result = ResultFilter::Interrupted;
        app.refilter();
        assert_eq!(app.visible, vec![2]);
    }

    #[test]
    fn dismissal_filter_intersects_preserves_hidden_marks_and_shows_saved_review() {
        let temp = crate::report_reader::fixture_with_dismissals();
        let mut app = Browser::new(LoadedReport::load(temp.path()).unwrap(), None);
        app.load_detail();
        assert!(app.detail.join("\n").contains("#1 missed"));
        assert!(
            app.detail
                .join("\n")
                .contains("Review: dismissed (saved at discovery)")
        );
        assert!(app.detail.join("\n").contains("Reviewed café / 目录"));
        assert!(!app.detail.join("\n").contains('\x1b'));
        key(&mut app, KeyCode::Char(' '));
        key(&mut app, KeyCode::Char('s'));
        assert_eq!(app.visible, [1, 3]);
        assert_eq!(app.hidden_marks(), 1);
        assert!(app.command().unwrap().ends_with("--id 1"));
        key(&mut app, KeyCode::Char('s'));
        assert_eq!(app.visible, [0, 2]);
        key(&mut app, KeyCode::Char('o'));
        assert_eq!(app.visible, [0]);
        key(&mut app, KeyCode::Char('p'));
        assert!(app.visible.is_empty());
        assert!(app.command().unwrap().ends_with("--id 1"));
        key(&mut app, KeyCode::Char('x'));
        assert_eq!(app.review, ReviewFilter::All);
        key(&mut app, KeyCode::Char('s'));
        key(&mut app, KeyCode::Char('s'));
        key(&mut app, KeyCode::Char('/'));
        app.handle(Event::Paste("c.go".into()));
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.visible, [2]);
        key(&mut app, KeyCode::Char('a'));
        assert!(app.command().unwrap().ends_with("--id 1,3"));
        key(&mut app, KeyCode::Char('v'));
        let review = app.detail.join("\n");
        for expected in [
            "2 dismissed; 1 need review",
            "Missed: 0 undismissed; 1 dismissed",
            "missing",
            "removed.go",
            "outcomes and counts are unchanged",
        ] {
            assert!(review.contains(expected), "{review}");
        }
        assert!(key(&mut app, KeyCode::Char('e')));
        assert!(app.export.unwrap().ends_with("--id 1,3"));
    }

    #[test]
    fn legacy_reports_remain_browsable_with_unrecorded_review_and_empty_dismissed_filter() {
        let (_temp, mut app) = browser();
        assert!(
            app.detail
                .join("\n")
                .contains("Dismissal review: not recorded")
        );
        key(&mut app, KeyCode::Char('s'));
        assert_eq!(app.visible.len(), 4);
        key(&mut app, KeyCode::Char('s'));
        assert!(app.visible.is_empty());
        key(&mut app, KeyCode::Char('v'));
        assert!(app.detail.join("\n").contains("was not recorded"));
        key(&mut app, KeyCode::Char('x'));
        assert_eq!(app.visible.len(), 4);
    }

    #[test]
    fn input_editing_event_kinds_and_detail_navigation() {
        let (_temp, mut app) = browser();
        key(&mut app, KeyCode::Char('/'));
        assert!(!key(&mut app, KeyCode::Char('q')));
        assert_eq!(app.draft.as_deref(), Some("q"));
        key(&mut app, KeyCode::Esc);
        assert_eq!(app.file_filter, "");
        key(&mut app, KeyCode::Char('/'));
        app.handle(Event::Paste(format!("\n\x1b{}", "界".repeat(300))));
        let draft = app.draft.as_ref().unwrap();
        assert_eq!(draft.chars().count(), 256);
        assert!(!draft.chars().any(char::is_control));
        key(&mut app, KeyCode::Esc);
        assert!(!app.handle(Event::Key(KeyEvent::new_with_kind(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
            KeyEventKind::Release
        ))));
        app.handle(Event::Key(KeyEvent::new_with_kind(
            KeyCode::Char(' '),
            KeyModifiers::NONE,
            KeyEventKind::Repeat,
        )));
        assert!(app.marked.is_empty());
        app.handle(Event::Key(KeyEvent::new_with_kind(
            KeyCode::Down,
            KeyModifiers::NONE,
            KeyEventKind::Repeat,
        )));
        assert_eq!(app.selected(), Some(1));
        key(&mut app, KeyCode::Char('l'));
        assert!(app.detail.join("\n").contains("diagnostic 2"));
        assert!(!app.detail.join("\n").contains('\x1b'));
        key(&mut app, KeyCode::Tab);
        key(&mut app, KeyCode::End);
        assert_eq!(app.scroll, app.detail.len() - 1);
        key(&mut app, KeyCode::Char('b'));
        assert_eq!(app.scroll, 0);
        assert!(app.detail.join("\n").contains("baseline passed"));
        key(&mut app, KeyCode::Char('g'));
        assert!(app.detail.join("\n").contains("packages discovered"));
    }

    #[test]
    fn commands_quote_paths_and_export_without_executing() {
        let (_temp, mut app) = browser();
        let command = app.command().unwrap();
        assert!(command.starts_with("mutie '/missing/a project'\\''s checkout' --rerun '"));
        app.module = Some(PathBuf::from("/new/$checkout `literal`"));
        key(&mut app, KeyCode::End);
        key(&mut app, KeyCode::Char('r'));
        assert!(
            app.detail
                .join("\n")
                .contains("mutie '/new/$checkout `literal`'")
        );
        assert!(app.detail.join("\n").contains("--id 4"));
        assert!(key(&mut app, KeyCode::Char('e')));
        assert!(app.export.as_ref().unwrap().ends_with("--id 4"));
        app.module = Some(PathBuf::from("/bad\npath"));
        assert!(app.command().is_err());
    }

    #[test]
    fn rust_reports_export_explicit_language_and_legacy_reports_remain_inspectable() {
        let temp = crate::report_reader::fixture();
        let path = temp.path().join("run.json");
        let mut run: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        run["effective_config"] = serde_json::json!({ "lang": "rust" });
        fs::write(&path, serde_json::to_vec(&run).unwrap()).unwrap();
        let mut app = Browser::new(LoadedReport::load(temp.path()).unwrap(), None);
        key(&mut app, KeyCode::Char('d'));
        assert!(app.detail.join("\n").contains("+false"));
        key(&mut app, KeyCode::Char('r'));
        assert!(app.detail.join("\n").contains("lacks execution scope"));
        assert!(!key(&mut app, KeyCode::Char('e')));
        assert!(app.export.is_none());
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();
        assert!(text(terminal.backend().buffer()).contains("lacks execution scope"));
        assert!(
            crate::rerun::RerunSelection::load(&crate::rerun::RerunRequest {
                source_run: temp.path().into(),
                filter: crate::rerun::RerunFilter::new(
                    crate::rerun::RerunMode::All,
                    vec![],
                    vec![]
                )
            })
            .is_err()
        );
        // Fill the scope as a current report would. The browser does not run
        // Cargo or require a current checkout to print this command.
        run["rust_scope"] = serde_json::json!({
            "package":"sample", "binary":"sample", "source":"src/main.rs",
            "edition":"2024", "manifest_fingerprint":"sha256:manifest",
            "lockfile_fingerprint":"sha256:lock", "default_features":true,
            "test_scope":"binary unit and integration tests; ignored tests excluded",
            "profile":"test", "rustc":"rustc version", "cargo":"cargo version",
            "discovery_exclusions":[]
        });
        fs::write(&path, serde_json::to_vec(&run).unwrap()).unwrap();
        let mut app = Browser::new(
            LoadedReport::load(temp.path()).unwrap(),
            Some("/moved/a project's checkout".into()),
        );
        key(&mut app, KeyCode::Char(' '));
        key(&mut app, KeyCode::Char('r'));
        assert!(app.detail.join("\n").contains(" --lang rust --rerun "));
        terminal.draw(|frame| app.render(frame)).unwrap();
        assert!(text(terminal.backend().buffer()).contains("--lang rust --rerun"));
        assert!(key(&mut app, KeyCode::Char('e')));
        let command = app.export.unwrap();
        assert!(
            command.starts_with("mutie '/moved/a project'\\''s checkout' --lang rust --rerun '")
        );
        assert!(command.ends_with("--id 1"));
    }

    #[test]
    fn vitest_exports_explicit_runner_and_requires_saved_identity() {
        let temp = crate::report_reader::fixture();
        let path = temp.path().join("run.json");
        let mut run: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        run["effective_config"] = serde_json::json!({"lang":"vitest"});
        fs::write(&path, serde_json::to_vec(&run).unwrap()).unwrap();
        let inventory_path = temp.path().join("mutants.json");
        let mut inventory: serde_json::Value =
            serde_json::from_slice(&fs::read(&inventory_path).unwrap()).unwrap();
        for mutation in inventory["mutations"].as_array_mut().unwrap() {
            mutation["file"] = format!("src/{}.ts", mutation["id"]).into();
            mutation["operator"] = "boolean_flip".into();
        }
        fs::write(&inventory_path, serde_json::to_vec(&inventory).unwrap()).unwrap();
        let mut app = Browser::new(LoadedReport::load(temp.path()).unwrap(), None);
        key(&mut app, KeyCode::Char('r'));
        assert!(app.detail.join("\n").contains("lacks execution scope"));
        assert!(!key(&mut app, KeyCode::Char('e')));
        assert!(app.export.is_none());
        run["vitest_scope"] = serde_json::to_value(crate::vitest::fixture_scope()).unwrap();
        fs::write(&path, serde_json::to_vec(&run).unwrap()).unwrap();
        let mut app = Browser::new(
            LoadedReport::load(temp.path()).unwrap(),
            Some("/moved/a project's checkout".into()),
        );
        key(&mut app, KeyCode::Char(' '));
        key(&mut app, KeyCode::End);
        key(&mut app, KeyCode::Char(' '));
        key(&mut app, KeyCode::Char('r'));
        let mut terminal = Terminal::new(TestBackend::new(120, 24)).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();
        assert!(text(terminal.backend().buffer()).contains("--lang vitest"));
        assert!(key(&mut app, KeyCode::Char('e')));
        let command = app.export.unwrap();
        assert!(
            command.starts_with("mutie '/moved/a project'\\''s checkout' --lang vitest --rerun '")
        );
        assert!(command.ends_with("--id 1,4"));
    }

    #[test]
    fn missing_artifacts_are_inline_and_rendering_uses_cached_content() {
        let (temp, mut app) = browser();
        fs::remove_file(temp.path().join("logs/000001.log")).unwrap();
        key(&mut app, KeyCode::Char('l'));
        assert!(app.detail.join("\n").contains("Artifact unavailable"));
        key(&mut app, KeyCode::Char('d'));
        fs::remove_file(temp.path().join("diff/000001.diff")).unwrap();
        key(&mut app, KeyCode::Char(' '));
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();
        assert!(text(terminal.backend().buffer()).contains("+false"));
    }

    #[test]
    fn frames_show_selection_diff_styles_and_unicode_cursor_across_sizes() {
        let (_temp, mut app) = browser();
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();
        let buffer = terminal.backend().buffer();
        let contents = text(buffer);
        assert!(contents.contains("4 mutations | 1 missed"), "{contents}");
        assert!(contents.contains("000001 missed"), "{contents}");
        assert!(
            buffer
                .content
                .iter()
                .any(|cell| cell.symbol() == "+" && cell.fg == Color::Green)
        );
        assert!(
            buffer
                .content
                .iter()
                .any(|cell| cell.modifier.contains(Modifier::REVERSED))
        );
        app.draft = Some("界e\u{301}".into());
        terminal.draw(|frame| app.render(frame)).unwrap();
        assert_eq!(terminal.get_cursor_position().unwrap(), (10, 2).into());
        for (width, height) in [(80, 24), (24, 12), (18, 5), (1, 1), (0, 0), (110, 14)] {
            terminal.backend_mut().resize(width, height);
            terminal.draw(|frame| app.render(frame)).unwrap();
            if (width, height) == (24, 12) {
                assert!(text(terminal.backend().buffer()).contains("000001"));
            }
        }
        app.draft = None;
        app.file_filter = "absent".into();
        app.refilter();
        app.load_detail();
        terminal.draw(|frame| app.render(frame)).unwrap();
        assert!(text(terminal.backend().buffer()).contains("No matches"));
        let mut offset = Terminal::with_options(
            TestBackend::new(110, 30),
            TerminalOptions {
                viewport: Viewport::Fixed(Rect::new(3, 4, 100, 24)),
            },
        )
        .unwrap();
        offset.draw(|frame| app.render(frame)).unwrap();
        assert_eq!(offset.backend().buffer()[(4, 4)].symbol(), "m");
        assert_eq!(offset.backend().buffer()[(0, 0)].symbol(), " ");
    }

    #[test]
    fn restoration_attempts_all_steps_after_an_output_error() {
        #[derive(Default)]
        struct FailOnce {
            failed: bool,
            bytes: Vec<u8>,
        }
        impl Write for FailOnce {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if !self.failed {
                    self.failed = true;
                    return Err(io::Error::other("injected failure"));
                }
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut writer = FailOnce::default();
        assert!(restore_terminal(&mut writer).is_err());
        let output = String::from_utf8(writer.bytes).unwrap();
        assert!(output.contains("\x1b[?1049l"));
        assert!(output.contains("\x1b[?25h"));
    }

    #[test]
    fn report_frame_snapshots() {
        let (_temp, mut app) = browser();
        app.report.directory = PathBuf::from("/saved/run");
        for (name, width, height) in [("report-wide", 100, 24), ("report-compact", 70, 24)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| app.render(frame)).unwrap();
            let actual = text(terminal.backend().buffer());
            let path =
                Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/snapshots/{name}.txt"));
            if std::env::var_os("MUTIE_UPDATE_SNAPSHOTS").is_some() {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, format!("{actual}\n")).unwrap();
            }
            assert_eq!(
                actual.trim_end(),
                fs::read_to_string(path).unwrap().trim_end()
            );
        }
    }

    #[test]
    fn dismissal_frames_show_review_without_replacing_outcome_styles() {
        let temp = crate::report_reader::fixture_with_dismissals();
        let mut app = Browser::new(LoadedReport::load(temp.path()).unwrap(), None);
        app.load_detail();
        app.report.directory = PathBuf::from("/saved/run");
        for (name, width, height, view) in [
            ("report-dismissals-wide", 100, 24, View::Diff),
            ("report-dismissals-compact", 70, 24, View::Diff),
            ("report-review", 100, 28, View::Dismissals),
        ] {
            app.view = view;
            app.load_detail();
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| app.render(frame)).unwrap();
            let buffer = terminal.backend().buffer();
            let actual = text(buffer);
            assert!(actual.contains("[D]"), "{actual}");
            assert!(
                buffer
                    .content
                    .iter()
                    .any(|cell| cell.symbol() == "D" && cell.fg == Color::Cyan)
            );
            assert!(
                buffer
                    .content
                    .iter()
                    .any(|cell| cell.symbol() == "m" && cell.fg == Color::Red)
            );
            assert!(!actual.contains('\x1b'));
            let path =
                Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/snapshots/{name}.txt"));
            if std::env::var_os("MUTIE_UPDATE_SNAPSHOTS").is_some() {
                fs::write(&path, format!("{actual}\n")).unwrap();
            }
            assert_eq!(
                actual.trim_end(),
                fs::read_to_string(path).unwrap().trim_end()
            );
        }
        for (width, height) in [(24, 12), (23, 11), (1, 1), (0, 0), (110, 14)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| app.render(frame)).unwrap();
        }
    }
}
