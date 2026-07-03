//! Interactive TUI dashboard (ratatui + crossterm): live metrics, parameter
//! steering, prompt editing, and explicit config save.

use crate::config::{save_config, AppConfig};
use crate::ffi::ffi;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, Paragraph, Sparkline, Wrap},
    Terminal,
};
use std::collections::VecDeque;
use std::fs::File;
use std::sync::Arc;

/// Redirects the process's stdout file descriptor to /dev/null for the lifetime
/// of the returned guard, restoring it on drop.
///
/// The C++ MRT2 engine writes diagnostic lines straight to stdout on background
/// threads (e.g. `[MagentaRT] Combined Prompt (...) tokens: ...` in
/// mlx_engine.cpp when the async prompt encoder finishes). During a TUI session
/// that print lands on the ratatui alternate screen and corrupts the frame until
/// the next full redraw. We can't patch upstream C++, so we silence fd 1 while
/// the TUI owns the screen.
///
/// Crucially, the TUI itself does NOT render through fd 1 while this guard is
/// active -- it renders through an explicit /dev/tty handle (see
/// `run_tui_dashboard`). That separation is what makes silencing fd 1 safe:
/// only the engine's stray writes get dropped, never our own drawing.
/// Restored (Drop) even on panic/early return.
struct StdoutSilencer {
    saved_fd: Option<libc::c_int>,
}

impl StdoutSilencer {
    fn new() -> Self {
        // SAFETY: standard dup/open/dup2 fd juggling; all return values checked.
        unsafe {
            let saved = libc::dup(libc::STDOUT_FILENO);
            if saved < 0 {
                return Self { saved_fd: None };
            }
            let devnull = libc::open(c"/dev/null".as_ptr(), libc::O_WRONLY);
            if devnull < 0 {
                libc::close(saved);
                return Self { saved_fd: None };
            }
            libc::dup2(devnull, libc::STDOUT_FILENO);
            libc::close(devnull);
            Self { saved_fd: Some(saved) }
        }
    }
}

impl Drop for StdoutSilencer {
    fn drop(&mut self) {
        if let Some(saved) = self.saved_fd {
            // SAFETY: `saved` is a valid fd we dup'd in new(); restore and close it.
            unsafe {
                libc::dup2(saved, libc::STDOUT_FILENO);
                libc::close(saved);
            }
        }
    }
}

/// Helper function to create a centered rectangular area for TUI popup modals.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn run_tui_dashboard(
    runner: &Arc<cxx::UniquePtr<ffi::RealtimeRunnerBridge>>,
    base_config: AppConfig,
    model_path: &Option<String>,
    audio_format: String,
) {
    // Render the TUI through an explicit /dev/tty handle rather than stdout. This
    // lets us redirect the process's fd 1 (stdout) to /dev/null for the session
    // -- silencing the C++ engine's stray background std::cout writes -- without
    // affecting our own drawing, which goes to /dev/tty. If /dev/tty can't be
    // opened (unusual), fall back to a stdout-backed terminal without silencing.
    let tty = File::options().read(true).write(true).open("/dev/tty").ok();

    // Only silence fd 1 when we have a separate /dev/tty to draw on; otherwise
    // silencing would also kill our stdout-backed rendering.
    let stdout_silencer = tty.as_ref().map(|_| StdoutSilencer::new());

    enable_raw_mode().expect("❌ Failed to enable raw mode");

    // Backend writer: /dev/tty if available, else stdout.
    let mut writer: Box<dyn std::io::Write + Send> = match tty {
        Some(f) => Box::new(f),
        None => Box::new(std::io::stdout()),
    };
    execute!(writer, EnterAlternateScreen).expect("❌ Failed to enter alternate screen");
    let backend = CrosstermBackend::new(writer);
    let mut terminal = Terminal::new(backend).expect("❌ Failed to create terminal");

    // Dynamic history ring for sparkline (will be resized on draw ticks to match terminal columns)
    let mut trans_history: VecDeque<u64> = VecDeque::with_capacity(120);
    let session_start = std::time::Instant::now();

    let mut last_trans_ms = 0.0f64;
    let mut last_dropped: u64 = 0;
    let mut resets: u32 = 0;

    // Mutably track live-adjusted parameters so they reflect in the TUI in real
    // time. Initialized from the effective (CLI-over-config-merged) session
    // settings, so a --volume-db / --midi-gate passed on the CLI is reflected
    // in the initial TUI display and in what an explicit save would persist.
    let mut cur_temp = base_config.temperature;
    let mut cur_topk = base_config.topk;
    let mut cur_cfg_text = base_config.cfg_text;
    let mut cur_cfg_drums = base_config.cfg_drums;
    let mut cur_volume_db = base_config.volume_db;
    let mut cur_midi_gate = base_config.midi_gate;
    let mut current_prompt = base_config.prompt.clone();

    // Input mode for changing style prompt mid-playback
    let mut input_mode = false;
    let mut input_string = String::new();

    // Transient status message shown in the title bar (e.g. after saving config)
    let mut status_message: Option<(String, std::time::Instant)> = None;

    let model_display = model_path
        .as_deref()
        .and_then(|p| std::path::Path::new(p).file_stem())
        .and_then(|s| s.to_str())
        .unwrap_or("none")
        .to_string();

    let draw_tick = std::time::Duration::from_millis(33); // ~30 Hz
    let metrics_tick = std::time::Duration::from_millis(200); // 5 Hz
    let mut last_metrics = std::time::Instant::now();

    let result = (|| -> std::io::Result<()> {
        loop {
            // Query dynamic terminal width to adjust sparkline history size
            let max_history_size = if let Ok(sz) = terminal.size() {
                // The sparkline block width minus borders
                (sz.width as usize).saturating_sub(2).max(10)
            } else {
                60
            };

            // Keep history trimmed to the dynamic column width
            while trans_history.len() > max_history_size {
                trans_history.pop_front();
            }

            // Poll crossterm events (non-blocking)
            if event::poll(draw_tick)? {
                if let Event::Key(key) = event::read()? {
                    if input_mode {
                        match key.code {
                            KeyCode::Enter => {
                                if !input_string.trim().is_empty() {
                                    current_prompt = input_string.clone();
                                    runner.set_prompt(&current_prompt);
                                    runner.toggle_play(true); // Immediate reset and re-anchor!
                                    runner.reset_dropped_frames();
                                    resets += 1;
                                }
                                input_mode = false;
                                input_string.clear();
                            }
                            KeyCode::Esc => {
                                input_mode = false;
                                input_string.clear();
                            }
                            KeyCode::Backspace => {
                                input_string.pop();
                            }
                            KeyCode::Char(c) => {
                                if input_string.len() < 200 {
                                    input_string.push(c);
                                }
                            }
                            _ => {}
                        }
                    } else {
                        match key.code {
                            // Lifecycle
                            KeyCode::Char('q') | KeyCode::Esc => break,
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                            KeyCode::Char('r') => {
                                runner.toggle_play(true);
                                runner.reset_dropped_frames();
                                resets += 1;
                            }

                            // Enter prompt input mode
                            KeyCode::Char('/') | KeyCode::Char('p') => {
                                input_mode = true;
                                input_string = current_prompt.clone();
                            }

                            // Interactive Parameter Adjustments (steers C++ runner atomically!)
                            KeyCode::Char('[') => {
                                cur_cfg_text = (cur_cfg_text - 0.5).max(1.0);
                                runner.set_cfg_text(cur_cfg_text);
                            }
                            KeyCode::Char(']') => {
                                cur_cfg_text = (cur_cfg_text + 0.5).min(10.0);
                                runner.set_cfg_text(cur_cfg_text);
                            }
                            KeyCode::Char('-') => {
                                cur_temp = (cur_temp - 0.1).max(0.1);
                                runner.set_temperature(cur_temp);
                            }
                            KeyCode::Char('+') | KeyCode::Char('=') => {
                                cur_temp = (cur_temp + 0.1).min(2.5);
                                runner.set_temperature(cur_temp);
                            }
                            KeyCode::Char(',') | KeyCode::Char('<') => {
                                cur_topk = cur_topk.saturating_sub(5).max(5);
                                runner.set_top_k(cur_topk);
                            }
                            KeyCode::Char('.') | KeyCode::Char('>') => {
                                cur_topk = (cur_topk + 5).min(200);
                                runner.set_top_k(cur_topk);
                            }
                            KeyCode::Char('d') => {
                                cur_cfg_drums = (cur_cfg_drums - 0.5).max(0.0);
                                runner.set_cfg_drums(cur_cfg_drums);
                            }
                            KeyCode::Char('f') => {
                                cur_cfg_drums = (cur_cfg_drums + 0.5).min(10.0);
                                runner.set_cfg_drums(cur_cfg_drums);
                            }
                            KeyCode::Char('v') => {
                                cur_volume_db = (cur_volume_db - 2.0).max(-60.0);
                                runner.set_volume_db(cur_volume_db);
                            }
                            KeyCode::Char('b') => {
                                cur_volume_db = (cur_volume_db + 2.0).min(12.0);
                                runner.set_volume_db(cur_volume_db);
                            }
                            KeyCode::Char('g') => {
                                cur_midi_gate = !cur_midi_gate;
                                runner.set_midi_gate(cur_midi_gate);
                            }
                            // Explicitly persist the current live params to config.toml.
                            // Preserves the non-TUI-adjustable fields (model,
                            // resources, output_dir, cfg_notes, drumless) from
                            // the effective session config.
                            KeyCode::Char('S') => {
                                let mut to_save = base_config.clone();
                                to_save.prompt = current_prompt.clone();
                                to_save.temperature = cur_temp;
                                to_save.topk = cur_topk;
                                to_save.cfg_text = cur_cfg_text;
                                to_save.cfg_drums = cur_cfg_drums;
                                to_save.volume_db = cur_volume_db;
                                to_save.midi_gate = cur_midi_gate;
                                save_config(&to_save);
                                status_message = Some((
                                    "✓ Saved current parameters to config.toml".to_string(),
                                    std::time::Instant::now(),
                                ));
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Refresh metrics at 5 Hz (not every draw tick -- read_metrics is a JSON alloc)
            if last_metrics.elapsed() >= metrics_tick {
                last_metrics = std::time::Instant::now();
                let metrics_json = runner.read_metrics();
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&metrics_json) {
                    last_trans_ms = val.get("transformer_ms").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    last_dropped = val.get("dropped_frames").and_then(|v| v.as_u64()).unwrap_or(0);
                }
                let sparkval = last_trans_ms.round() as u64;
                trans_history.push_back(sparkval);
                if trans_history.len() > max_history_size {
                    trans_history.pop_front();
                }
            }

            // Draw frame
            let uptime = session_start.elapsed().as_secs();
            let trans_ms = last_trans_ms;
            let dropped = last_dropped;
            let spark_data: Vec<u64> = trans_history.iter().copied().collect();
            let resets_count = resets;
            let prompt_ref = current_prompt.as_str();
            let model_ref = model_display.as_str();
            let audio_ref = audio_format.as_str();

            // Traffic-light color for transformer latency vs 40ms budget
            let latency_color = if trans_ms < 30.0 {
                Color::Green
            } else if trans_ms < 40.0 {
                Color::Yellow
            } else {
                Color::Red
            };

            // Gauge ratio: how much of the 40ms budget we're using (capped at 100%)
            let budget_ratio = (trans_ms / 40.0).clamp(0.0, 1.0);

            // Transient status message (e.g. save confirmation), shown for ~3s in the title bar
            let title_text = match &status_message {
                Some((msg, at)) if at.elapsed() < std::time::Duration::from_secs(3) => {
                    format!(" Magenta RealTime 2 — Rust Player    [{}] ", msg)
                }
                _ => " Magenta RealTime 2 — Rust Player ".to_string(),
            };

            terminal.draw(|f| {
                let area = f.area();
                let rows = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(7),  // session info
                        Constraint::Length(4),  // budget gauge
                        Constraint::Length(8),  // sparkline
                        Constraint::Min(0),     // spacer
                        Constraint::Length(6),  // keyboard control help panel
                    ])
                    .split(area);

                // ── Session info panel ──────────────────────────────────────
                let info_lines = vec![
                    Line::from(vec![
                        Span::styled("  Model    ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(model_ref),
                    ]),
                    Line::from(vec![
                        Span::styled("  Prompt   ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(format!("\"{}\"", prompt_ref)),
                    ]),
                    Line::from(vec![
                        Span::styled("  Tuning   ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(format!("PromptStrength: {:.1}  Temp: {:.1}  TopK: {}  DrumsCFG: {:.1}  MidiGate: {}",
                            cur_cfg_text, cur_temp, cur_topk, cur_cfg_drums, if cur_midi_gate { "Enabled" } else { "Disabled" })),
                    ]),
                    Line::from(vec![
                        Span::styled("  Output   ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(format!("Volume: {:.1} dB  (Device: {})", cur_volume_db, audio_ref)),
                    ]),
                    Line::from(vec![
                        Span::styled("  Uptime   ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(format!("{:02}:{:02}:{:02}  resets: {}",
                            uptime / 3600, (uptime % 3600) / 60, uptime % 60, resets_count)),
                    ]),
                ];
                let info = Paragraph::new(info_lines)
                    .block(Block::default()
                        .borders(Borders::ALL)
                        .title(title_text.clone())
                        .title_style(Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)))
                    .wrap(Wrap { trim: false });
                f.render_widget(info, rows[0]);

                // ── Budget gauge ────────────────────────────────────────────
                let gauge_label = format!(
                    "Transformer  {:.1} ms  /  40.0 ms budget  ({:.0}%)   dropped frames: {}",
                    trans_ms, budget_ratio * 100.0, dropped
                );
                let gauge = Gauge::default()
                    .block(Block::default().borders(Borders::ALL).title(" Frame Budget "))
                    .gauge_style(Style::default().fg(latency_color))
                    .ratio(budget_ratio)
                    .label(gauge_label);
                f.render_widget(gauge, rows[1]);

                // ── Sparkline ───────────────────────────────────────────────
                let spark_label = format!(" Transformer ms (last {} samples, full width) ", max_history_size);
                let spark = Sparkline::default()
                    .block(Block::default()
                        .borders(Borders::ALL)
                        .title(spark_label))
                    .style(Style::default().fg(latency_color))
                    .data(&spark_data);
                f.render_widget(spark, rows[2]);

                // ── Key help bar ────────────────────────────────────────────
                let help_lines = vec![
                    Line::from(vec![
                        Span::styled("  [ / ] ", Style::default().fg(Color::Yellow)),
                        Span::raw("Prompt Strength  "),
                        Span::styled("  - / + ", Style::default().fg(Color::Yellow)),
                        Span::raw("Temp  "),
                        Span::styled("  , / . ", Style::default().fg(Color::Yellow)),
                        Span::raw("TopK  "),
                        Span::styled("  d / f ", Style::default().fg(Color::Yellow)),
                        Span::raw("Drums CFG  "),
                        Span::styled("  v / b ", Style::default().fg(Color::Yellow)),
                        Span::raw("Volume"),
                    ]),
                    Line::from(vec![
                        Span::styled("  p / / ", Style::default().fg(Color::Yellow)),
                        Span::raw("Edit Prompt Mid-play  "),
                        Span::styled("  g     ", Style::default().fg(Color::Yellow)),
                        Span::raw("Toggle MIDI Gate  "),
                        Span::styled("  r     ", Style::default().fg(Color::Yellow)),
                        Span::raw("Reset Audio Context (re-anchors to prompt)"),
                    ]),
                    Line::from(vec![
                        Span::styled("  S     ", Style::default().fg(Color::Green)),
                        Span::raw("Save current params to config    "),
                        Span::styled("  q/ESC ", Style::default().fg(Color::Red)),
                        Span::raw("Quit Player"),
                    ]),
                ];
                let help = Paragraph::new(help_lines)
                    .block(Block::default().borders(Borders::ALL).title(" Interactive Playback Controls "));
                f.render_widget(help, rows[4]);

                // ── Floating popup input box (drawn only in input mode) ────
                if input_mode {
                    let popup_area = centered_rect(65, 20, area);
                    let popup_block = Block::default()
                        .borders(Borders::ALL)
                        .title(" Edit Style Prompt ")
                        .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
                    let popup_text = vec![
                        Line::from(vec![
                            Span::styled("Type a new prompt and press ", Style::default()),
                            Span::styled("Enter", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                            Span::styled(" to apply & reset context, or ", Style::default()),
                            Span::styled("ESC", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                            Span::styled(" to cancel.", Style::default()),
                        ]),
                        Line::from(""),
                        Line::from(vec![
                            Span::styled("> ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                            Span::raw(&input_string),
                        ]),
                    ];
                    let popup = Paragraph::new(popup_text)
                        .block(popup_block)
                        .wrap(Wrap { trim: true });

                    f.render_widget(Clear, popup_area); // Clears background area
                    f.render_widget(popup, popup_area);
                }
            })?;
        }
        Ok(())
    })();

    // Always restore the terminal, even on panic/error
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    // Restore the real stdout before we print the final line below.
    drop(stdout_silencer);

    if let Err(e) = result {
        eprintln!("TUI error: {}", e);
    }

    println!("Playback stopped.");
}
