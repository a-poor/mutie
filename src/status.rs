use std::{
    io::{self, IsTerminal, Write},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use crossterm::{
    cursor::{MoveTo, Show},
    execute,
    style::ResetColor,
    terminal::{Clear, ClearType},
};
use ratatui::{
    Frame, Terminal, TerminalOptions, Viewport,
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};
use tokio::{sync::watch, task::JoinHandle};

use crate::{
    cancellation::Cancellation,
    cli::UiMode,
    config::RunConfig,
    coordinator::{Coordinator, RunState},
    report::Outcome,
};

pub fn select(config: &RunConfig) -> Result<bool> {
    select_mode(
        config.ui,
        config.list,
        io::stdin().is_terminal(),
        io::stdout().is_terminal(),
        std::env::var_os("CI").is_some(),
        std::env::var("TERM").is_ok_and(|term| term == "dumb"),
    )
}

fn select_mode(
    mode: UiMode,
    list: bool,
    stdin_tty: bool,
    stdout_tty: bool,
    ci: bool,
    dumb: bool,
) -> Result<bool> {
    if list || mode == UiMode::Plain {
        return Ok(false);
    }
    let supported = stdin_tty && stdout_tty && !dumb;
    if mode == UiMode::Interactive {
        ensure!(
            supported,
            "interactive UI requires terminal stdin/stdout and TERM other than dumb; use --ui plain"
        );
        return Ok(true);
    }
    Ok(supported && !ci)
}

// Also prevents another draw after a worker panic has restored the terminal.
static LIVE: AtomicBool = AtomicBool::new(false);

pub struct LiveTerminal {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    origin: (u16, u16),
    restored: bool,
}

impl LiveTerminal {
    pub fn start(workers: usize, cancellation: Cancellation) -> Result<Self> {
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            cancellation.cancel();
            if LIVE.swap(false, Ordering::SeqCst) {
                // A panic may interrupt a backend cursor-position query.
                let _ = crossterm::terminal::disable_raw_mode();
                let _ = restore_writer(&mut io::stdout());
            }
            previous_hook(info);
        }));
        // No persistent raw mode, alternate screen, or input task: normal
        // terminal signals remain available between backend cursor queries.
        let mut terminal = Terminal::with_options(
            CrosstermBackend::new(io::stdout()),
            TerminalOptions {
                viewport: Viewport::Inline(8 + workers.min(4) as u16),
            },
        )
        .inspect_err(|_| {
            let _ = crossterm::terminal::disable_raw_mode();
            let _ = restore_writer(&mut io::stdout());
        })
        .context("initializing live terminal status")?;
        LIVE.store(true, Ordering::SeqCst);
        let area = terminal.get_frame().area();
        Ok(Self {
            terminal,
            origin: (area.x, area.y),
            restored: false,
        })
    }

    fn draw(&mut self, state: &RunState, cancelling: bool) -> Result<()> {
        ensure!(
            LIVE.load(Ordering::SeqCst),
            "terminal session stopped after a panic"
        );
        let now = Instant::now();
        let frame = self
            .terminal
            .draw(|frame| render(frame, state, now, cancelling))
            .context("drawing live terminal status")?;
        self.origin = (frame.area.x, frame.area.y);
        Ok(())
    }

    fn restore(&mut self) -> Result<()> {
        if self.restored {
            return Ok(());
        }
        self.restored = true;
        LIVE.store(false, Ordering::SeqCst);
        // Avoid a cursor-position query during teardown. Attempt restoration
        // even if clearing the inline area fails.
        let cleared = execute!(
            self.terminal.backend_mut(),
            MoveTo(self.origin.0, self.origin.1),
            Clear(ClearType::FromCursorDown)
        );
        let restored = restore_writer(self.terminal.backend_mut());
        cleared.and(restored).context("restoring terminal")
    }
}

impl Drop for LiveTerminal {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

fn restore_writer(writer: &mut impl Write) -> io::Result<()> {
    let reset = execute!(writer, ResetColor);
    let show = execute!(writer, Show);
    let flush = writer.flush();
    reset.and(show).and(flush)
}

/// Refresh at most ten times per second, coalescing state snapshots. A display
/// failure requests cancellation but never drops the engine or its active workers.
pub async fn drive(
    mut engine: JoinHandle<(Coordinator, Result<()>)>,
    updates: watch::Receiver<RunState>,
    terminal: &mut Option<LiveTerminal>,
    cancellation: &Cancellation,
) -> Result<(Coordinator, Result<()>)> {
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut display_error = None;
    let finished = loop {
        tokio::select! {
            result = &mut engine => break result,
            _ = interval.tick(), if terminal.is_some() => {
                let state = updates.borrow().clone();
                if let Err(error) = terminal.as_mut().unwrap().draw(&state, cancellation.is_cancelled()) {
                    cancellation.cancel();
                    display_error = Some(error);
                    if let Some(mut terminal) = terminal.take()
                        && let Err(error) = terminal.restore() {
                        display_error = Some(display_error.unwrap().context(format!("{error:#}")));
                    }
                }
            }
        }
    };
    if let Some(mut terminal) = terminal.take()
        && let Err(error) = terminal.restore()
    {
        display_error = Some(error);
    }
    let (coordinator, result) = finished.context("run coordinator panicked")?;
    let result = match (result, display_error) {
        (result, None) => result,
        (Ok(()), Some(error)) => Err(error),
        (Err(run), Some(display)) => Err(display.context(format!("run stopped: {run:#}"))),
    };
    Ok((coordinator, result))
}

pub fn print_summary(coordinator: &Coordinator) {
    let Some(path) = coordinator.report_path() else {
        return;
    };
    let state = &coordinator.state;
    println!("\n{}", state.summary);
    println!(
        "Run {} in {:.2}s",
        state.status,
        state.started.elapsed().as_secs_f64()
    );
    if state.pending() > 0 || state.interrupted > 0 {
        println!(
            "Unfinished: {} pending, {} interrupted",
            state.pending(),
            state.interrupted
        );
    }
    if coordinator.report_finished {
        println!("Report saved: {}", path.join("summary.txt").display());
    } else {
        println!("Report incomplete: {}", path.display());
    }
}

fn safe_text(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

fn outcome_style(outcome: Option<Outcome>) -> Style {
    Style::default().fg(match outcome {
        Some(Outcome::Killed) => Color::Green,
        Some(Outcome::Missed | Outcome::Error) => Color::Red,
        Some(Outcome::Timeout | Outcome::BuildFailed) | None => Color::Yellow,
    })
}

fn render(frame: &mut Frame, state: &RunState, now: Instant, cancelling: bool) {
    let area = frame.area();
    if area.is_empty() {
        return;
    }
    let elapsed = now.saturating_duration_since(state.started);
    let spinner = ["|", "/", "-", "\\"][(elapsed.as_millis() / 100 % 4) as usize];
    let phase = if cancelling {
        "stopping"
    } else {
        state.phase.name()
    };
    let header = Line::from(vec![
        Span::styled(format!(" {spinner} mutie "), Style::new().cyan().bold()),
        Span::raw(format!("{phase}  {:.1}s", elapsed.as_secs_f64())),
    ]);
    let total = state
        .total
        .map(|n| n.to_string())
        .unwrap_or_else(|| "?".into());
    let progress = Line::from(format!(
        " Mutations {}/{total}  |  Workers {}/{}",
        state.summary.total(),
        state.active.len(),
        state.workers
    ));
    let counts = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            format!("{} killed", state.summary.killed),
            outcome_style(Some(Outcome::Killed)),
        ),
        Span::raw(" | "),
        Span::styled(
            format!("{} missed", state.summary.missed),
            outcome_style(Some(Outcome::Missed)),
        ),
        Span::raw(format!(
            " | {} timeout | {} build failed | {} errors",
            state.summary.timeout, state.summary.build_failed, state.summary.errors
        )),
    ]);
    let hint = Line::from(if cancelling {
        " Stopping workers and saving results..."
    } else {
        " Ctrl-C to stop  |  Test output saved in report logs"
    })
    .dim();
    if area.height <= 4 {
        frame.render_widget(Paragraph::new(vec![header, progress, counts, hint]), area);
        return;
    }
    let [heading, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .areas(area);
    frame.render_widget(header, heading);
    frame.render_widget(Paragraph::new(vec![progress, counts, hint]), footer);
    let available = body.height as usize;
    let worker_slots = state.active.len().min(4).min(available);
    let hidden_workers = state.active.len().saturating_sub(worker_slots);
    let worker_slots = if hidden_workers > 0 {
        worker_slots.saturating_sub(1)
    } else {
        worker_slots
    };
    let active_rows = worker_slots + usize::from(hidden_workers > 0 && available > 0);
    let recent_slots = available.saturating_sub(active_rows).min(3);
    let mut lines = Vec::new();
    for result in state
        .recent
        .iter()
        .rev()
        .take(recent_slots)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        let outcome = result
            .outcome
            .map(|o| o.to_string())
            .unwrap_or_else(|| "interrupted".into());
        let duration = result
            .elapsed
            .map(|d| format!(" ({:.1}s)", d.as_secs_f64()))
            .unwrap_or_default();
        lines.push(Line::from(vec![
            Span::styled(format!("   {outcome:12} "), outcome_style(result.outcome)),
            Span::raw(format!("{}{duration}", safe_text(&result.description))),
        ]));
    }
    for (worker, active) in state.active.iter().take(worker_slots) {
        lines.push(Line::from(vec![
            Span::styled(format!(" {spinner} worker {worker} "), Style::new().cyan()),
            Span::raw(format!(
                "#{:06} ({:.1}s) {}",
                active.id,
                now.saturating_duration_since(active.started).as_secs_f64(),
                safe_text(&active.description)
            )),
        ]));
    }
    if hidden_workers > 0 && available > 0 {
        lines.push(
            Line::from(format!(
                "   + {} more active workers",
                state.active.len() - worker_slots
            ))
            .dim(),
        );
    }
    if lines.is_empty() {
        lines.push(
            Line::from(match state.phase {
                crate::coordinator::Phase::Snapshot => "   Preparing isolated workspaces...",
                crate::coordinator::Phase::Baseline => "   Running baseline tests...",
                crate::coordinator::Phase::Discovery => "   Discovering eligible Go mutations...",
                crate::coordinator::Phase::Inventory => "   Saving mutation inventory and diffs...",
                crate::coordinator::Phase::Mutations => "   Preparing workers...",
            })
            .dim(),
        );
    }
    frame.render_widget(Paragraph::new(lines), body);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinator::{ActiveWorker, Phase, RecentResult};
    use ratatui::{backend::TestBackend, buffer::Buffer, layout::Rect};

    fn text(buffer: &Buffer) -> String {
        buffer
            .content
            .chunks(buffer.area.width as usize)
            .map(|row| {
                row.iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn state() -> RunState {
        let mut state = RunState {
            phase: Phase::Mutations,
            total: Some(8),
            workers: 2,
            ..RunState::default()
        };
        state.summary.killed = 2;
        state.summary.missed = 1;
        state.active.insert(
            1,
            ActiveWorker {
                id: 4,
                description: "calc.go:8:3: + -> -".into(),
                started: state.started,
            },
        );
        state.active.insert(
            2,
            ActiveWorker {
                id: 5,
                description: "ready.go:4:2: true -> false".into(),
                started: state.started,
            },
        );
        state.recent.push_back(RecentResult {
            description: "add.go:3:2: + -> -".into(),
            outcome: Some(Outcome::Killed),
            elapsed: Some(Duration::from_millis(200)),
        });
        state
    }

    #[test]
    fn mode_selection_accounts_for_ttys_ci_dumb_terminals_and_listing() -> Result<()> {
        for (input, output, ci, dumb, expected) in [
            (true, true, false, false, true),
            (true, true, true, false, false),
            (false, true, false, false, false),
            (true, false, false, false, false),
            (true, true, false, true, false),
        ] {
            assert_eq!(
                select_mode(UiMode::Auto, false, input, output, ci, dumb)?,
                expected
            );
        }
        assert!(select_mode(
            UiMode::Interactive,
            false,
            true,
            true,
            true,
            false
        )?);
        assert!(select_mode(UiMode::Interactive, false, false, true, false, false).is_err());
        assert!(select_mode(UiMode::Interactive, false, true, false, false, false).is_err());
        assert!(select_mode(UiMode::Interactive, false, true, true, false, true).is_err());
        assert!(!select_mode(
            UiMode::Plain,
            false,
            true,
            true,
            false,
            false
        )?);
        assert!(!select_mode(
            UiMode::Interactive,
            true,
            false,
            false,
            false,
            true
        )?);
        Ok(())
    }

    #[test]
    fn complete_frame_shows_workers_results_and_summary_with_semantic_styles() -> Result<()> {
        let state = state();
        let mut terminal = Terminal::new(TestBackend::new(88, 10))?;
        terminal
            .draw(|frame| render(frame, &state, state.started + Duration::from_secs(2), false))?;
        let buffer = terminal.backend().buffer();
        assert_eq!(
            text(buffer),
            concat!(
                " | mutie mutations  2.0s\n",
                "   killed       add.go:3:2: + -> - (0.2s)\n",
                " | worker 1 #000004 (2.0s) calc.go:8:3: + -> -\n",
                " | worker 2 #000005 (2.0s) ready.go:4:2: true -> false\n",
                "\n\n\n",
                " Mutations 3/8  |  Workers 2/2\n",
                " 2 killed | 1 missed | 0 timeout | 0 build failed | 0 errors\n",
                " Ctrl-C to stop  |  Test output saved in report logs"
            )
        );
        assert_eq!(buffer[(3, 1)].fg, Color::Green);
        assert_eq!(buffer[(12, 8)].fg, Color::Red);
        Ok(())
    }

    #[test]
    fn elapsed_and_activity_change_without_completion_and_shorter_frames_clear_old_rows()
    -> Result<()> {
        let mut state = state();
        let mut terminal = Terminal::new(TestBackend::new(88, 10))?;
        terminal.draw(|frame| render(frame, &state, state.started, false))?;
        let before = terminal.backend().buffer().clone();
        terminal.draw(|frame| {
            render(
                frame,
                &state,
                state.started + Duration::from_millis(300),
                false,
            )
        })?;
        assert_ne!(&before, terminal.backend().buffer());
        assert!(text(terminal.backend().buffer()).contains("(0.3s)"));
        state.active.clear();
        state.recent.clear();
        terminal.draw(|frame| render(frame, &state, state.started, true))?;
        let after = text(terminal.backend().buffer());
        assert!(after.contains("stopping"));
        assert!(!after.contains("calc.go"));
        assert!(!after.contains("ready.go"));
        assert!(!after.contains("add.go"));
        Ok(())
    }

    #[test]
    fn tiny_resized_and_offset_frames_clip_unicode_and_hide_excess_workers() -> Result<()> {
        let mut state = state();
        for worker in 3..=12 {
            state.active.insert(
                worker,
                ActiveWorker {
                    id: worker,
                    description: "路径/long-name-e\u{301}😀.go\x1b[2J\n:1:1".repeat(3),
                    started: state.started,
                },
            );
        }
        let mut terminal = Terminal::new(TestBackend::new(90, 14))?;
        terminal.draw(|frame| render(frame, &state, state.started, false))?;
        assert!(text(terminal.backend().buffer()).contains("more active workers"));
        assert!(!text(terminal.backend().buffer()).contains('\x1b'));
        for (width, height) in [(30, 6), (1, 1), (0, 0), (80, 24), (50, 3)] {
            terminal.backend_mut().resize(width, height);
            terminal.draw(|frame| render(frame, &state, state.started, false))?;
            assert_eq!(
                terminal.backend().buffer().area,
                Rect::new(0, 0, width, height)
            );
        }
        let mut offset = Terminal::with_options(
            TestBackend::new(100, 24),
            TerminalOptions {
                viewport: Viewport::Fixed(Rect::new(5, 3, 88, 10)),
            },
        )?;
        offset.draw(|frame| render(frame, &state, state.started, false))?;
        assert_eq!(offset.backend().buffer()[(6, 3)].symbol(), "|");
        assert_eq!(offset.backend().buffer()[(0, 0)].symbol(), " ");
        Ok(())
    }

    #[test]
    fn restoration_attempts_show_cursor_even_if_reset_fails() {
        #[derive(Default)]
        struct Writer {
            first: bool,
            bytes: Vec<u8>,
            flushed: bool,
        }
        impl Write for Writer {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                if !self.first {
                    self.first = true;
                    return Err(io::Error::other("failed write"));
                }
                self.bytes.extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                self.flushed = true;
                Ok(())
            }
        }
        let mut writer = Writer::default();
        assert!(restore_writer(&mut writer).is_err());
        assert!(String::from_utf8_lossy(&writer.bytes).contains("\x1b[?25h"));
        assert!(writer.flushed);
    }
}
