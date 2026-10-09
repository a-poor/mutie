use std::{
    io::{self, IsTerminal, Write},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use crossterm::{
    cursor::{MoveToColumn, MoveUp, Show},
    execute, queue,
    style::ResetColor,
    terminal::{Clear, ClearType, DisableLineWrap, EnableLineWrap},
};
use ratatui::{
    Frame, Terminal, TerminalOptions, Viewport,
    backend::{Backend, ClearType as BackendClearType, CrosstermBackend},
    layout::{Constraint, Layout, Rect, Size},
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
    terminal: Terminal<CrosstermBackend<Vec<u8>>>,
    size: Size,
    height: u16,
    area: Rect,
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
        Self::initialize(workers)
            .inspect_err(|_| {
                let _ = crossterm::terminal::disable_raw_mode();
                let _ = restore_writer(&mut io::stdout());
            })
            .context("initializing live terminal status")
    }

    fn initialize(workers: usize) -> Result<Self> {
        let mut backend = CrosstermBackend::new(Vec::new());
        let size = backend.size()?;
        let height = 8 + workers.min((u16::MAX - 8) as usize) as u16;
        let area = allocate_inline(&mut backend, size, height, true)?;
        let terminal = fixed_terminal(backend, area)?;
        LIVE.store(true, Ordering::SeqCst);
        Ok(Self {
            terminal,
            size,
            height,
            area,
            restored: false,
        })
    }

    fn draw(&mut self, state: &RunState, cancelling: bool) -> Result<()> {
        ensure!(
            LIVE.load(Ordering::SeqCst),
            "terminal session stopped after a panic"
        );
        let size = self.terminal.size()?;
        if size != self.size {
            // Ratatui 0.30.1's resize clears the entire visible screen on a
            // horizontal shrink, including content above an inline viewport.
            // Rebuild fixed buffers instead; only clear the area we own.
            self.area = allocate_inline(self.terminal.backend_mut(), size, self.height, false)?;
            self.terminal = fixed_terminal(CrosstermBackend::new(Vec::new()), self.area)?;
            self.size = size;
        }
        let now = Instant::now();
        self.terminal
            .draw(|frame| render(frame, state, now, cancelling))
            .context("drawing live terminal status")?;
        // Park at the start of our area between frames. The terminal tracks
        // this anchor through resizing/reflow, unlike a saved absolute row.
        self.terminal.set_cursor_position(self.area.as_position())?;
        flush_frame(self.terminal.backend_mut(), self.area.y)?;
        Ok(())
    }

    fn restore(&mut self) -> Result<()> {
        if self.restored {
            return Ok(());
        }
        self.restored = true;
        LIVE.store(false, Ordering::SeqCst);
        // Clear from the parked cursor, not a stale absolute row: the terminal
        // may have resized since our last frame. No teardown query is needed.
        // Attempt restoration even if clearing the inline area fails.
        let cleared = execute!(
            self.terminal.backend_mut(),
            MoveToColumn(0),
            Clear(ClearType::FromCursorDown)
        )
        .and_then(|()| flush_commands(self.terminal.backend_mut()));
        let restored = restore_writer(&mut io::stdout());
        cleared.and(restored).context("restoring terminal")
    }
}

fn fixed_terminal(
    backend: CrosstermBackend<Vec<u8>>,
    area: Rect,
) -> io::Result<Terminal<CrosstermBackend<Vec<u8>>>> {
    Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Fixed(area),
        },
    )
}

// Setup/cleanup commands already use relative rows. Keep their allocation for reuse.
fn flush_commands(backend: &mut CrosstermBackend<Vec<u8>>) -> io::Result<()> {
    let commands = backend.writer_mut();
    let mut stdout = io::stdout().lock();
    let written = stdout.write_all(commands);
    commands.clear();
    written.and_then(|()| stdout.flush())
}

fn flush_frame(backend: &mut CrosstermBackend<Vec<u8>>, origin_row: u16) -> io::Result<()> {
    let frame = relative_frame(backend.writer_mut(), origin_row)?;
    backend.writer_mut().clear();
    let mut stdout = io::stdout().lock();
    stdout.write_all(&frame)?;
    stdout.flush()
}

// A frame may reach the terminal after a resize, even if its size was checked
// immediately before writing. Absolute rows would overwrite pulled-back history
// and move our parked cursor above the UI. Translate the pinned Crossterm
// backend's CUP sequences to motion relative to that terminal-maintained anchor.
// This processes only our renderer's commands, never subprocess output.
fn relative_frame(commands: &[u8], mut row: u16) -> io::Result<Vec<u8>> {
    let mut frame = Vec::with_capacity(commands.len());
    // A late wide frame must not wrap when the terminal has become narrower.
    queue!(frame, DisableLineWrap)?;
    let mut offset = 0;
    while offset < commands.len() {
        if let Some((column, next_row, length)) = cursor_move(&commands[offset..]) {
            if next_row > row {
                // Unlike MoveDown, LF scrolls at the bottom. This keeps history
                // above the UI even when a late frame is taller than the screen.
                frame.extend(std::iter::repeat_n(b'\n', usize::from(next_row - row)));
            } else if next_row < row {
                queue!(frame, MoveUp(row - next_row))?;
            }
            queue!(frame, MoveToColumn(column))?;
            row = next_row;
            offset += length;
        } else {
            frame.push(commands[offset]);
            offset += 1;
        }
    }
    queue!(frame, EnableLineWrap)?;
    Ok(frame)
}

// Crossterm 0.29 encodes MoveTo as ESC [ row ; column H (one-based u16s).
fn cursor_move(bytes: &[u8]) -> Option<(u16, u16, usize)> {
    if !bytes.starts_with(b"\x1b[") {
        return None;
    }
    let end = bytes.iter().take(14).position(|byte| *byte == b'H')?;
    let coordinates = std::str::from_utf8(&bytes[2..end]).ok()?;
    let (row, column) = coordinates.split_once(';')?;
    Some((
        column.parse::<u16>().ok()?.checked_sub(1)?,
        row.parse::<u16>().ok()?.checked_sub(1)?,
        end + 1,
    ))
}

/// Reserve an inline area using normal scrolling, preserving earlier output in
/// scrollback. On resize the cursor is still parked at the previous area's start.
fn allocate_inline(
    backend: &mut CrosstermBackend<Vec<u8>>,
    size: Size,
    height: u16,
    starting: bool,
) -> Result<Rect> {
    ensure!(
        size.width > 0 && size.height > 0,
        "terminal has no drawable area"
    );
    let cursor = backend.get_cursor_position()?;
    let mut row = cursor.y.min(size.height - 1);
    if starting && cursor.x > 0 {
        // Preserve an existing partial line, too.
        backend.append_lines(1)?;
        row = row.saturating_add(1).min(size.height - 1);
    }
    // The terminal may resize after the query; retain its actual cursor row.
    execute!(backend, MoveToColumn(0))?;
    backend.clear_region(BackendClearType::AfterCursor)?;
    let height = height.min(size.height);
    // Clamp BEFORE appending lines: requesting more rows than the screen has
    // otherwise needlessly scrolls old output away behind blank history.
    backend.append_lines(height.saturating_sub(1))?;
    let area = Rect::new(0, row.min(size.height - height), size.width, height);
    if height > 1 {
        execute!(backend, MoveUp(height - 1))?;
    }
    execute!(backend, MoveToColumn(0))?;
    flush_commands(backend)?;
    Ok(area)
}

impl Drop for LiveTerminal {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

fn restore_writer(writer: &mut impl Write) -> io::Result<()> {
    let reset = execute!(writer, ResetColor);
    let wrap = execute!(writer, EnableLineWrap);
    let show = execute!(writer, Show);
    let flush = writer.flush();
    reset.and(wrap).and(show).and(flush)
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

fn outcome_counts(state: &RunState, labels: [&str; 5], separator: &str) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (index, ((count, outcome), label)) in [
        (state.summary.killed, Outcome::Killed),
        (state.summary.missed, Outcome::Missed),
        (state.summary.timeout, Outcome::Timeout),
        (state.summary.build_failed, Outcome::BuildFailed),
        (state.summary.errors, Outcome::Error),
    ]
    .into_iter()
    .zip(labels)
    .enumerate()
    {
        if index > 0 {
            spans.push(Span::raw(separator.to_owned()));
        }
        spans.push(Span::styled(
            format!("{count} {label}"),
            outcome_style(Some(outcome)),
        ));
    }
    Line::from(spans)
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
    let mut counts = outcome_counts(
        state,
        ["killed", "missed", "timeout", "build failed", "errors"],
        " | ",
    );
    if counts.width() > area.width as usize {
        counts = outcome_counts(state, ["kill", "miss", "time", "build", "err"], " ");
    }
    if counts.width() > area.width as usize {
        counts = outcome_counts(state, ["K", "M", "T", "B", "E"], " ");
    }
    let mut hint = Line::from(if cancelling {
        " Stopping workers and saving results..."
    } else {
        " Ctrl-C to stop  |  Test output saved in report logs"
    })
    .dim();
    if hint.width() > area.width as usize {
        hint = Line::from(if cancelling {
            " Stopping; saving results..."
        } else {
            " Ctrl-C to stop | Logs saved"
        })
        .dim();
    }
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
    // Geometry depends on the worker count, never on which workers are busy or
    // how many results have arrived. Reserve a tail even before the first result.
    let tail_space = available.saturating_sub(2).min(3);
    let worker_space = available.saturating_sub(tail_space);
    let worker_slots = if state.workers > worker_space {
        worker_space.saturating_sub(1)
    } else {
        state.workers
    };
    let hidden_workers = state.workers.saturating_sub(worker_slots);
    let active_rows = worker_slots + usize::from(hidden_workers > 0 && available > 0);
    let recent_slots = available.saturating_sub(active_rows).min(3);
    let mut lines = Vec::new();
    for worker in 1..=worker_slots {
        let prefix = if area.width < 70 {
            format!("w{worker}")
        } else {
            format!("worker {worker}")
        };
        let Some(active) = state.active.get(&worker) else {
            lines.push(Line::from(format!("   {prefix} idle")).dim());
            continue;
        };
        let elapsed = now.saturating_duration_since(active.started).as_secs_f64();
        let details = if area.width < 70 {
            let phase = match active.phase {
                crate::coordinator::WorkerPhase::Preparing => "prep",
                crate::coordinator::WorkerPhase::Running => "run",
                crate::coordinator::WorkerPhase::Building => "build",
                crate::coordinator::WorkerPhase::Testing => "test",
            };
            format!(
                "{phase} #{} {elapsed:.1}s {}",
                active.id,
                safe_text(&active.description)
            )
        } else {
            format!(
                "{:9} #{:06} ({elapsed:.1}s) {}",
                active.phase.name(),
                active.id,
                safe_text(&active.description)
            )
        };
        lines.push(Line::from(vec![
            Span::styled(format!(" {spinner} {prefix} "), Style::new().cyan()),
            Span::raw(details),
        ]));
    }
    if hidden_workers > 0 && available > 0 {
        let active = state
            .active
            .keys()
            .filter(|worker| **worker > worker_slots)
            .count();
        lines.push(Line::from(format!("   + {hidden_workers} workers ({active} active)")).dim());
    }
    if active_rows > 0 && available > active_rows + recent_slots {
        lines.push(Line::default());
    }
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
    use crate::coordinator::{ActiveWorker, Phase, RecentResult, WorkerPhase};
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
                phase: WorkerPhase::Building,
            },
        );
        state.active.insert(
            2,
            ActiveWorker {
                id: 5,
                description: "ready.go:4:2: true -> false".into(),
                started: state.started,
                phase: WorkerPhase::Testing,
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
    fn relative_frames_match_backend_output_for_full_and_incremental_draws() -> Result<()> {
        let mut state = state();
        state.active.get_mut(&1).unwrap().description = "目录/e\u{301}😀.go:2:1: + -> -".into();
        let area = Rect::new(0, 4, 88, 10);
        let mut terminal = fixed_terminal(CrosstermBackend::new(Vec::new()), area)?;
        let mut absolute = vt100::Parser::new(24, 88, 100);
        let mut relative = vt100::Parser::new(24, 88, 100);
        for parser in [&mut absolute, &mut relative] {
            parser.process(b"earlier output\r\n$ mutie\x1b[5;1H");
        }
        for elapsed in [0, 1, 2] {
            if elapsed == 2 {
                state.active.clear();
                state.recent.clear();
            }
            terminal.draw(|frame| {
                render(
                    frame,
                    &state,
                    state.started + Duration::from_secs(elapsed),
                    false,
                )
            })?;
            terminal.set_cursor_position(area.as_position())?;
            let bytes = std::mem::take(terminal.backend_mut().writer_mut());
            let translated = relative_frame(&bytes, area.y)?;
            assert!((0..translated.len()).all(|i| cursor_move(&translated[i..]).is_none()));
            absolute.process(&bytes);
            relative.process(&translated);
            assert_eq!(relative.screen().contents(), absolute.screen().contents());
            assert_eq!(relative.screen().cursor_position(), (4, 0));
            assert!(relative.screen().contents().contains("earlier output"));
        }
        Ok(())
    }

    #[test]
    fn late_tall_wide_frames_scroll_history_and_restore_line_wrapping() -> Result<()> {
        let state = state();
        let area = Rect::new(0, 14, 88, 10);
        let mut terminal = fixed_terminal(CrosstermBackend::new(Vec::new()), area)?;
        terminal.draw(|frame| render(frame, &state, state.started, false))?;
        terminal.set_cursor_position(area.as_position())?;
        // The frame was prepared for 24 rows but is consumed after shrinking.
        let mut parser = vt100::Parser::new(5, 20, 100);
        parser.process(b"earlier output\r\n$ mutie\r\n");
        parser.process(&relative_frame(
            terminal.backend_mut().writer_mut(),
            area.y,
        )?);
        assert_eq!(parser.screen().cursor_position(), (0, 0));
        let mut history = parser.screen().clone();
        history.set_scrollback(usize::MAX);
        let rows = history.scrollback();
        for marker in ["earlier output", "$ mutie"] {
            assert!(
                (0..=rows).any(|offset| {
                    history.set_scrollback(offset);
                    history.contents().contains(marker)
                }),
                "lost {marker}"
            );
        }
        // Normal shell text wraps again after the protected frame ends.
        parser.process(b"\x1b[5;20Hab");
        assert_eq!(parser.screen().cursor_position(), (4, 1));
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
                " | worker 1 building  #000004 (2.0s) calc.go:8:3: + -> -\n",
                " | worker 2 testing   #000005 (2.0s) ready.go:4:2: true -> false\n",
                "\n",
                "   killed       add.go:3:2: + -> - (0.2s)\n",
                "\n\n",
                " Mutations 3/8  |  Workers 2/2\n",
                " 2 killed | 1 missed | 0 timeout | 0 build failed | 0 errors\n",
                " Ctrl-C to stop  |  Test output saved in report logs"
            )
        );
        assert_eq!(buffer[(3, 4)].fg, Color::Green);
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
        state.workers = 12;
        for worker in 3..=12 {
            state.active.insert(
                worker,
                ActiveWorker {
                    id: worker,
                    description: "路径/long-name-e\u{301}😀.go\x1b[2J\n:1:1".repeat(3),
                    started: state.started,
                    phase: WorkerPhase::Preparing,
                },
            );
        }
        let mut terminal = Terminal::new(TestBackend::new(90, 14))?;
        terminal.draw(|frame| render(frame, &state, state.started, false))?;
        assert!(text(terminal.backend().buffer()).contains("workers (6 active)"));
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
    fn narrow_summary_keeps_every_outcome_visible_and_workers_expand_with_height() -> Result<()> {
        let mut state = state();
        state.total = Some(77);
        state.workers = 6;
        state.summary.killed = 53;
        state.summary.missed = 18;
        state.summary.timeout = 3;
        state.summary.build_failed = 2;
        state.summary.errors = 1;
        for worker in 3..=6 {
            state.active.insert(
                worker,
                ActiveWorker {
                    id: worker,
                    description: "parser.go:245:7: == -> !=".into(),
                    started: state.started,
                    phase: WorkerPhase::Running,
                },
            );
        }
        let mut terminal = Terminal::new(TestBackend::new(48, 7))?;
        terminal.draw(|frame| render(frame, &state, state.started, false))?;
        let narrow = text(terminal.backend().buffer());
        assert!(
            narrow.contains("53 kill 18 miss 3 time 2 build 1 err"),
            "{narrow}"
        );
        assert!(narrow.contains("Ctrl-C to stop | Logs saved"));
        assert!(narrow.contains("w1 build #4 0.0s calc.go:8:3"));
        assert!(narrow.contains("+ 5 workers (5 active)"));
        assert!(narrow.contains("killed"));
        terminal.backend_mut().resize(100, 12);
        terminal.draw(|frame| render(frame, &state, state.started, false))?;
        let wide = text(terminal.backend().buffer());
        assert!(wide.contains("worker 4 running   #000004"), "{wide}");
        assert!(wide.contains("+ 2 workers (2 active)"));
        assert!(!wide.contains("worker 5"));
        terminal.backend_mut().resize(100, 14);
        terminal.draw(|frame| render(frame, &state, state.started, false))?;
        assert!(text(terminal.backend().buffer()).contains("worker 6 running"));
        Ok(())
    }

    #[test]
    fn worker_rows_stay_fixed_through_idle_reuse_and_new_results() -> Result<()> {
        let mut state = state();
        let mut terminal = Terminal::new(TestBackend::new(100, 10))?;
        terminal.draw(|frame| render(frame, &state, state.started, false))?;
        let row = |buffer: &Buffer, y: u16| -> String {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        };
        let second = row(terminal.backend().buffer(), 2);
        let first = state.active.remove(&1).unwrap();
        for (name, outcome) in [
            ("second.go", Outcome::Missed),
            ("third.go", Outcome::BuildFailed),
            ("fourth.go", Outcome::Timeout),
            ("last.go", Outcome::Error),
        ] {
            state.recent.push_back(RecentResult {
                description: name.into(),
                outcome: Some(outcome),
                elapsed: None,
            });
        }
        terminal.draw(|frame| render(frame, &state, state.started, false))?;
        let buffer = terminal.backend().buffer();
        assert_eq!(row(buffer, 1), "   worker 1 idle");
        assert_eq!(row(buffer, 2), second);
        assert_eq!(row(buffer, 4), "   build failed third.go");
        assert_eq!(row(buffer, 5), "   timeout      fourth.go");
        assert_eq!(row(buffer, 6), "   error        last.go");
        assert_eq!(
            buffer[(3, 4)].fg,
            outcome_style(Some(Outcome::BuildFailed)).fg.unwrap()
        );
        state.active.insert(
            1,
            ActiveWorker {
                id: 6,
                phase: WorkerPhase::Preparing,
                ..first
            },
        );
        state.recent.clear();
        terminal.draw(|frame| render(frame, &state, state.started, false))?;
        let buffer = terminal.backend().buffer();
        assert!(row(buffer, 1).contains("worker 1 preparing #000006"));
        assert_eq!(row(buffer, 2), second);
        assert_eq!(row(buffer, 4), "");
        Ok(())
    }

    #[test]
    fn restoration_attempts_wrapping_and_cursor_even_if_reset_fails() {
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
        assert!(String::from_utf8_lossy(&writer.bytes).contains("\x1b[?7h"));
        assert!(String::from_utf8_lossy(&writer.bytes).contains("\x1b[?25h"));
        assert!(writer.flushed);
    }
}
