use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use serde_json::Value;
use std::{
    collections::VecDeque,
    env, fs,
    io::{self, Read, Stdout},
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

const EXIT_ENV: i32 = 3;
const COMMANDS: &[&str] = &[
    "/help",
    "/status",
    "/agents",
    "/swarm",
    "/run",
    "/lifecycle",
    "/meter",
    "/ratchet",
    "/ledger",
    "/doctor",
    "/clear",
    "/exit",
];
const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub fn run() -> Result<(), i32> {
    let history_path = state_dir()?.join("repl-history");
    let history = load_history(&history_path)?;
    let mut app = App::new(history, history_path);
    let mut terminal = setup_terminal().map_err(|_| EXIT_ENV)?;
    let result = event_loop(&mut terminal, &mut app);
    restore_terminal(&mut terminal).map_err(|_| EXIT_ENV)?;
    result
}

struct App {
    input: String,
    cursor: usize,
    output: String,
    history: Vec<String>,
    history_index: Option<usize>,
    history_path: PathBuf,
    context: String,
    operation: Option<Operation>,
    pending_plan: Option<PendingPlan>,
    operation_queue: VecDeque<CommandSpec>,
    spinner_tick: usize,
    completion_index: usize,
}

struct Operation {
    child: Child,
    receiver: Receiver<StreamEvent>,
    streams_remaining: usize,
    command: String,
    cancelling: bool,
    cancelled_at: Option<Instant>,
}

enum StreamEvent {
    Data(String),
    Closed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CommandSpec {
    args: Vec<String>,
}

impl CommandSpec {
    fn display(&self) -> String {
        let rendered = self
            .args
            .iter()
            .map(|word| {
                if word.chars().all(|character| {
                    character.is_ascii_alphanumeric() || "-._/".contains(character)
                }) {
                    word.clone()
                } else {
                    format!("{:?}", word)
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        format!("fleet {rendered}")
    }
}

#[derive(Clone, Debug)]
struct PendingPlan {
    prompt: String,
    commands: Vec<CommandSpec>,
}

impl App {
    fn new(history: Vec<String>, history_path: PathBuf) -> Self {
        Self {
            input: String::new(),
            cursor: 0,
            output: "Fleet interactive mode. Type a task prompt, or /help for commands.\n"
                .to_string(),
            history,
            history_index: None,
            history_path,
            context: render_context(),
            operation: None,
            pending_plan: None,
            operation_queue: VecDeque::new(),
            spinner_tick: 0,
            completion_index: 0,
        }
    }
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()
}

fn event_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<(), i32> {
    loop {
        drain_operation(app);
        terminal
            .draw(|frame| render(frame, app))
            .map_err(|_| EXIT_ENV)?;
        if !event::poll(Duration::from_millis(80)).map_err(|_| EXIT_ENV)? {
            app.spinner_tick = (app.spinner_tick + 1) % SPINNER.len();
            continue;
        }
        let Event::Key(key) = event::read().map_err(|_| EXIT_ENV)? else {
            continue;
        };
        if handle_key(app, key)? {
            return Ok(());
        }
    }
}

fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool, i32> {
    if app.operation.is_some() {
        if is_ctrl_c(key) {
            cancel_operation(app);
        }
        return Ok(false);
    }
    if app.pending_plan.is_some() {
        if is_ctrl_c(key) {
            cancel_plan(app);
            return Ok(false);
        }
        match key.code {
            KeyCode::Enter => confirm_plan(app)?,
            KeyCode::Char('e') => edit_plan(app),
            KeyCode::Esc => cancel_plan(app),
            _ => {}
        }
        return Ok(false);
    }
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input.clear();
            app.cursor = 0;
            app.history_index = None;
        }
        KeyCode::Char(character) => {
            app.input.insert(app.cursor, character);
            app.cursor += character.len_utf8();
            app.completion_index = 0;
        }
        KeyCode::Backspace => {
            let previous = previous_char_boundary(&app.input, app.cursor);
            if previous != app.cursor {
                app.input.drain(previous..app.cursor);
                app.cursor = previous;
            }
            app.completion_index = 0;
        }
        KeyCode::Delete => {
            let next = next_char_boundary(&app.input, app.cursor);
            if next != app.cursor {
                app.input.drain(app.cursor..next);
            }
            app.completion_index = 0;
        }
        KeyCode::Left => app.cursor = previous_char_boundary(&app.input, app.cursor),
        KeyCode::Right => app.cursor = next_char_boundary(&app.input, app.cursor),
        KeyCode::Home => app.cursor = 0,
        KeyCode::End => app.cursor = app.input.len(),
        KeyCode::Up => history_move(app, -1),
        KeyCode::Down => history_move(app, 1),
        KeyCode::Tab => complete(app),
        KeyCode::Enter => return submit(app),
        _ => {}
    }
    Ok(false)
}

fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Length(3),
        ])
        .split(area);
    frame.render_widget(
        Paragraph::new(app.context.as_str())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("fleet context"),
            )
            .style(Style::default().fg(Color::Cyan)),
        chunks[0],
    );
    let output_height = chunks[1].height.saturating_sub(2) as usize;
    let line_count = app.output.lines().count();
    let scroll = line_count
        .saturating_sub(output_height)
        .min(u16::MAX as usize) as u16;
    frame.render_widget(
        Paragraph::new(app.output.as_str())
            .block(Block::default().borders(Borders::ALL).title("output"))
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        chunks[1],
    );
    let prompt = if let Some(operation) = &app.operation {
        let glyph = SPINNER[app.spinner_tick % SPINNER.len()];
        let state = if operation.cancelling {
            "cancelling"
        } else {
            "running"
        };
        Line::from(vec![
            Span::styled(
                format!(" {glyph} {state} "),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(&operation.command),
            Span::styled(" · Ctrl-C cancels", Style::default().fg(Color::DarkGray)),
        ])
    } else if app.pending_plan.is_some() {
        Line::from(vec![
            Span::styled(
                " PLAN READY ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Enter run · e edit · Esc cancel"),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                " ❯ ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(&app.input),
        ])
    };
    frame.render_widget(
        Paragraph::new(prompt).block(Block::default().borders(Borders::ALL).title("command")),
        chunks[2],
    );
    if app.operation.is_none() && app.pending_plan.is_none() {
        let cursor_x = chunks[2].x.saturating_add(4).saturating_add(
            app.input[..app.cursor]
                .chars()
                .count()
                .min(u16::MAX as usize) as u16,
        );
        frame.set_cursor_position((
            cursor_x.min(chunks[2].right().saturating_sub(2)),
            chunks[2].y + 1,
        ));
    }
}

fn submit(app: &mut App) -> Result<bool, i32> {
    let input = app.input.trim().to_string();
    app.input.clear();
    app.cursor = 0;
    app.history_index = None;
    if input.is_empty() {
        return Ok(false);
    }
    app.output.push_str(&format!("\n❯ {input}\n"));
    if !input.starts_with('/') {
        return stage_natural_language_plan(app, input).map(|()| false);
    }
    remember(app, &input)?;
    let words = match split_words(&input) {
        Ok(words) => words,
        Err(reason) => {
            app.output.push_str(&format!("refused: {reason}\n"));
            return Ok(false);
        }
    };
    let Some(command) = words.first().map(String::as_str) else {
        return Ok(false);
    };
    if command == "/exit" {
        return Ok(true);
    }
    if command == "/clear" {
        app.output.clear();
        return Ok(false);
    }
    let Some(mut args) = mapped_args(command) else {
        let suggestion = nearest(command);
        app.output.push_str(&format!(
            "unknown command {command:?}; did you mean {suggestion}?\n"
        ));
        return Ok(false);
    };
    if command == "/swarm" && words.len() == 1 {
        args.push("status".to_string());
    }
    args.extend(words.into_iter().skip(1));
    app.operation = Some(spawn_operation(input, args)?);
    Ok(false)
}

fn stage_natural_language_plan(app: &mut App, prompt: String) -> Result<(), i32> {
    let intent = match crate::intent::classify(&prompt) {
        Ok(intent) => intent,
        Err(closest) => {
            app.output
                .push_str("unrecognised intent; refusing rather than guessing. Closest matches:\n");
            for example in closest {
                app.output.push_str(&format!("  - {example}\n"));
            }
            // The refusal is written only after it is visible in the transcript.
            crate::append_receipt(
                "refusal",
                serde_json::json!({"reason":"UNKNOWN_INTENT","prompt":prompt}),
                "fleet-repl",
                None,
                Some(7),
            )?;
            return Ok(());
        }
    };
    let (plan, rendered) = match build_plan(intent, &prompt) {
        Ok(plan) => plan,
        Err(code) => {
            app.output.push_str(
                "refused: deterministic routing prerequisites are unavailable; run /meter and /doctor, then retry\n",
            );
            crate::append_receipt(
                "refusal",
                serde_json::json!({
                    "reason":"ROUTING_PREREQUISITES_UNAVAILABLE",
                    "prompt":prompt,
                    "cause_exit":code
                }),
                "fleet-repl",
                None,
                Some(7),
            )?;
            return Ok(());
        }
    };
    // This append is deliberately before history persistence or process spawn.
    app.output.push_str(&rendered);
    app.pending_plan = Some(plan);
    Ok(())
}

fn build_plan(intent: crate::intent::Intent, prompt: &str) -> Result<(PendingPlan, String), i32> {
    let repo = env::current_dir().map_err(|_| EXIT_ENV)?;
    let mut routing = "no model/lane: local read-only command; capability, quota, and verifier independence are not applicable".to_string();
    let mut agents = "none (local fleet process); lifecycle: no transition".to_string();
    let mut skills = "none".to_string();
    let mut mcp_tools = "none (no lease required)".to_string();
    let mut token_cost = "0 tokens (local command; checked=0,total=0 model lanes)".to_string();
    let commands = match intent {
        crate::intent::Intent::LedgerVerify
        | crate::intent::Intent::MeterShow
        | crate::intent::Intent::Diagnose => intent
            .command_prefixes()
            .iter()
            .map(|prefix| CommandSpec {
                args: prefix.iter().map(|word| (*word).to_string()).collect(),
            })
            .collect(),
        crate::intent::Intent::Change => {
            let snapshot = crate::meter::planning_snapshot_at(&state_dir()?)?;
            let estimate = snapshot.estimated_task_tokens;
            let candidates = [
                ("codex", "codex", "Codex worker-tier coding capability"),
                ("claude", "claude", "Sonnet worker-tier coding capability"),
            ];
            let selected = candidates.iter().find(|(lane, executable, _)| {
                let installed = Command::new(executable)
                    .arg("--version")
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .is_ok_and(|status| status.success());
                let remaining = snapshot.remaining.get(*lane).copied().flatten();
                installed
                    && remaining.is_some_and(|remaining| {
                        remaining > 0 && estimate.is_none_or(|tokens| remaining >= tokens)
                    })
            });
            let Some((lane, worker, capability)) = selected else {
                return Err(7);
            };
            let remaining = snapshot.remaining.get(*lane).copied().flatten().ok_or(7)?;
            routing = format!(
                "builder role -> worker tier -> {capability} -> lane {lane} has {remaining} measured tokens remaining -> verifier resolves to stub-verifier-v1, distinct from the builder's resolved model (self-verification is refused)"
            );
            let registry = fleet::agent::Registry::load_default()?;
            let lifecycle = [
                ("lead", "Intake -> Specified"),
                ("designer", "Intake -> Specified -> Reviewed"),
                ("builder", "Intake -> Specified -> Reviewed -> Decomposed -> Contracted -> Briefed -> Leased -> Building -> Built"),
                ("verifier", "Intake -> Specified -> Reviewed -> Decomposed -> Contracted -> Briefed -> Leased -> Building -> Built -> Verifying -> Verified"),
                ("meter", "Intake -> Specified -> Reviewed -> Decomposed -> Contracted -> Briefed -> Leased -> Building -> Built -> Verifying -> Verified -> Attested -> Accepted -> Proposed -> Observed"),
            ];
            agents = lifecycle
                .iter()
                .map(|(role, states)| format!("{role}: {states}"))
                .collect::<Vec<_>>()
                .join("\n      ");
            skills = registry
                .agents()
                .iter()
                .map(|agent| format!("{}={}", agent.agent_id(), agent.skills().join(",")))
                .collect::<Vec<_>>()
                .join("; ");
            let manifest = crate::mcp::manifest_for_lease("keel/**")?;
            mcp_tools = manifest
                .get("tools")
                .and_then(Value::as_array)
                .ok_or(6)?
                .iter()
                .filter_map(|tool| tool.get("name").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(", ");
            token_cost = match estimate {
                Some(tokens) => format!(
                    "estimated {tokens} tokens from fleet meter reservations (checked={},total={})",
                    snapshot.checked, snapshot.total
                ),
                None => format!(
                    "unknown: fleet meter has no measured task reservation (checked={},total={}); quota remains measured",
                    snapshot.checked, snapshot.total
                ),
            };
            vec![CommandSpec {
                args: vec![
                    "swarm".into(),
                    "dispatch".into(),
                    "--task".into(),
                    prompt.into(),
                    "--repo".into(),
                    repo.display().to_string(),
                    "--agent".into(),
                    (*worker).into(),
                ],
            }]
        }
    };
    let command_lines = commands
        .iter()
        .enumerate()
        .map(|(index, command)| format!("  {}. {}", index + 1, command.display()))
        .collect::<Vec<_>>()
        .join("\n");
    let rendered = format!(
        "PLAN · intent: {}\ncommands (in order):\n{command_lines}\n  agents/lifecycle: {agents}\n  routing: {routing}\n  declared skills: {skills}\n  MCP lease tools: {mcp_tools}\n  token cost: {token_cost}\n\nEnter to run · e to edit the prompt/plan · Esc to cancel\n",
        intent.name()
    );
    Ok((
        PendingPlan {
            prompt: prompt.into(),
            commands,
        },
        rendered,
    ))
}

fn confirm_plan(app: &mut App) -> Result<(), i32> {
    let Some(plan) = app.pending_plan.take() else {
        return Ok(());
    };
    remember(app, &plan.prompt)?;
    app.output.push_str("plan confirmed\n");
    app.operation_queue.extend(plan.commands);
    start_next_operation(app)
}

fn edit_plan(app: &mut App) {
    if let Some(plan) = app.pending_plan.take() {
        app.input = plan.prompt;
        app.cursor = app.input.len();
        app.output
            .push_str("edit the prompt, then press Enter to regenerate the plan\n");
    }
}

fn cancel_plan(app: &mut App) {
    if app.pending_plan.take().is_some() {
        app.output.push_str("plan cancelled; no command was run\n");
    }
}

fn start_next_operation(app: &mut App) -> Result<(), i32> {
    let Some(spec) = app.operation_queue.pop_front() else {
        return Ok(());
    };
    let display = spec.display();
    app.operation = Some(spawn_operation(display, spec.args)?);
    Ok(())
}

fn mapped_args(command: &str) -> Option<Vec<String>> {
    let args = match command {
        "/help" => vec!["--help"],
        "/status" => vec!["swarm", "status"],
        "/agents" => vec!["agents", "list"],
        "/swarm" => vec!["swarm"],
        "/run" => vec!["run"],
        "/lifecycle" => vec!["lifecycle", "states"],
        "/meter" => vec!["meter", "show"],
        "/ratchet" => vec!["ratchet", "show"],
        "/ledger" => vec!["ledger", "count"],
        "/doctor" => vec!["doctor"],
        _ => return None,
    };
    Some(args.into_iter().map(str::to_string).collect())
}

fn spawn_operation(command: String, args: Vec<String>) -> Result<Operation, i32> {
    let executable = env::current_exe().map_err(|_| EXIT_ENV)?;
    spawn_operation_with(executable, command, args)
}

fn spawn_operation_with(
    executable: PathBuf,
    command: String,
    args: Vec<String>,
) -> Result<Operation, i32> {
    let mut child = Command::new(executable)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|_| EXIT_ENV)?;
    let stdout = child.stdout.take().ok_or(EXIT_ENV)?;
    let stderr = child.stderr.take().ok_or(EXIT_ENV)?;
    let (sender, receiver) = mpsc::channel();
    for mut stream in [Box::new(stdout) as Box<dyn Read + Send>, Box::new(stderr)] {
        let sender = sender.clone();
        thread::spawn(move || {
            let mut buffer = [0_u8; 4096];
            loop {
                match stream.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(length) => {
                        if sender
                            .send(StreamEvent::Data(
                                String::from_utf8_lossy(&buffer[..length]).into_owned(),
                            ))
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
            let _ = sender.send(StreamEvent::Closed);
        });
    }
    Ok(Operation {
        child,
        receiver,
        streams_remaining: 2,
        command,
        cancelling: false,
        cancelled_at: None,
    })
}

fn drain_operation(app: &mut App) {
    let Some(operation) = app.operation.as_mut() else {
        return;
    };
    while let Ok(event) = operation.receiver.try_recv() {
        match event {
            StreamEvent::Data(chunk) => app.output.push_str(&chunk),
            StreamEvent::Closed => {
                operation.streams_remaining = operation.streams_remaining.saturating_sub(1)
            }
        }
    }
    let finished = operation.child.try_wait().ok().flatten();
    let Some(status) = finished else {
        if operation
            .cancelled_at
            .is_some_and(|when| when.elapsed() >= Duration::from_secs(2))
        {
            let pid = operation.child.id();
            unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
        }
        return;
    };
    if operation.streams_remaining != 0 {
        return;
    }
    while let Ok(event) = operation.receiver.try_recv() {
        match event {
            StreamEvent::Data(chunk) => app.output.push_str(&chunk),
            StreamEvent::Closed => {
                operation.streams_remaining = operation.streams_remaining.saturating_sub(1)
            }
        }
    }
    let outcome = match status.code() {
        Some(0) => "operation complete",
        Some(3) => "operation ended: environment fault",
        Some(6) => "operation ended: invariant violation",
        Some(7) => "operation refused",
        Some(8) => "operation ended: verification mismatch",
        Some(_) | None => "operation ended unexpectedly",
    };
    app.output.push_str(&format!("\n{outcome}\n"));
    app.operation = None;
    if let Err(code) = start_next_operation(app) {
        app.output.push_str(&format!(
            "unable to start next planned command (exit {code})\n"
        ));
        app.operation_queue.clear();
    }
    app.context = render_context();
}

fn cancel_operation(app: &mut App) {
    let Some(operation) = app.operation.as_mut() else {
        return;
    };
    if operation.cancelling {
        return;
    }
    let pid = operation.child.id();
    // The child starts its own process group, so descendants are part of the current operation.
    unsafe { libc::kill(-(pid as i32), libc::SIGINT) };
    operation.cancelling = true;
    operation.cancelled_at = Some(Instant::now());
    app.output
        .push_str("\ncancellation requested for current operation\n");
}

fn is_ctrl_c(key: KeyEvent) -> bool {
    key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn complete(app: &mut App) {
    if app.input.contains(char::is_whitespace) {
        return;
    }
    let matches = COMMANDS
        .iter()
        .filter(|candidate| candidate.starts_with(&app.input))
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return;
    }
    let selected_index = app.completion_index % matches.len();
    let selected = matches[selected_index];
    app.input = (*selected).to_string();
    app.cursor = app.input.len();
    app.completion_index = (selected_index + 1) % matches.len();
}

fn history_move(app: &mut App, delta: isize) {
    if app.history.is_empty() {
        return;
    }
    let current = app.history_index.unwrap_or(app.history.len());
    let next = (current as isize + delta).clamp(0, app.history.len() as isize) as usize;
    if next == app.history.len() {
        app.history_index = None;
        app.input.clear();
        app.cursor = 0;
    } else {
        app.history_index = Some(next);
        app.input = app.history[next].clone();
        app.cursor = app.input.len();
    }
}

fn previous_char_boundary(input: &str, cursor: usize) -> usize {
    input[..cursor]
        .char_indices()
        .next_back()
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn next_char_boundary(input: &str, cursor: usize) -> usize {
    input[cursor..]
        .chars()
        .next()
        .map_or(cursor, |character| cursor + character.len_utf8())
}

fn remember(app: &mut App, command: &str) -> Result<(), i32> {
    app.history.push(command.to_string());
    let parent = app.history_path.parent().ok_or(EXIT_ENV)?;
    fs::create_dir_all(parent).map_err(|_| EXIT_ENV)?;
    let mut text = app.history.join("\n");
    text.push('\n');
    fs::write(&app.history_path, text).map_err(|_| EXIT_ENV)
}

fn load_history(path: &Path) -> Result<Vec<String>, i32> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(str::to_string)
            .collect()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(_) => Err(EXIT_ENV),
    }
}

fn state_dir() -> Result<PathBuf, i32> {
    if let Some(path) = env::var_os("FLEET_STATE") {
        if path.is_empty() {
            return Err(EXIT_ENV);
        }
        return Ok(PathBuf::from(path));
    }
    let home = env::var_os("HOME").ok_or(EXIT_ENV)?;
    Ok(PathBuf::from(home).join(".local/state/fleet"))
}

fn render_context() -> String {
    let repo = env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "—".to_string());
    let state = state_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "—".to_string());
    let lane = active_lane();
    let counts = agent_counts()
        .map(|(idle, busy, total)| {
            format!("idle={idle}/{total} · busy={busy}/{total} · checked={total},total={total}")
        })
        .unwrap_or_else(|| "idle=— · busy=— · checked=0,total=0".to_string());
    format!("repo {repo} · state {state} · lane {lane} · agents {counts}")
}

fn active_lane() -> String {
    if let Ok(lane) = env::var("FLEET_ACTIVE_LANE").or_else(|_| env::var("FLEET_LANE")) {
        if !lane.trim().is_empty() {
            return lane;
        }
    }
    Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|branch| branch.trim().to_string())
        .filter(|branch| !branch.is_empty())
        .unwrap_or_else(|| "—".to_string())
}

fn agent_counts() -> Option<(usize, usize, usize)> {
    let agents_dir = state_dir().ok()?.join("agents");
    let entries = fs::read_dir(agents_dir).ok()?;
    let mut total = 0usize;
    let mut idle = 0usize;
    for entry in entries.flatten() {
        if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let value: Value = serde_json::from_slice(&fs::read(entry.path()).ok()?).ok()?;
        let load = value.get("load")?.as_u64()?;
        total = total.checked_add(1)?;
        if load == 0 {
            idle = idle.checked_add(1)?;
        }
    }
    if total == 0 {
        return None;
    }
    Some((idle, total.checked_sub(idle)?, total))
}

fn nearest(input: &str) -> &'static str {
    COMMANDS
        .iter()
        .copied()
        .min_by_key(|candidate| edit_distance(input, candidate))
        .unwrap_or("/help")
}

fn edit_distance(left: &str, right: &str) -> usize {
    let mut previous = (0..=right.chars().count()).collect::<Vec<_>>();
    for (row, left_char) in left.chars().enumerate() {
        let mut current = vec![row + 1];
        for (column, right_char) in right.chars().enumerate() {
            current.push(
                (current[column] + 1)
                    .min(previous[column + 1] + 1)
                    .min(previous[column] + usize::from(left_char != right_char)),
            );
        }
        previous = current;
    }
    previous.last().copied().unwrap_or(0)
}

fn split_words(input: &str) -> Result<Vec<String>, &'static str> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut escaped = false;
    for character in input.chars() {
        if escaped {
            word.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(delimiter) = quote {
            if character == delimiter {
                quote = None;
            } else {
                word.push(character);
            }
        } else if character == '\'' || character == '"' {
            quote = Some(character);
        } else if character.is_whitespace() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
        } else {
            word.push(character);
        }
    }
    if escaped || quote.is_some() {
        return Err("unfinished quote or escape");
    }
    if !word.is_empty() {
        words.push(word);
    }
    Ok(words)
}

#[cfg(test)]
mod tests {
    use super::{
        drain_operation, edit_distance, handle_key, history_move, nearest, split_words, submit,
        App, Operation, StreamEvent,
    };
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::{
        path::PathBuf,
        process::{Command, Stdio},
        sync::mpsc,
        thread,
        time::Duration,
    };

    #[test]
    fn parses_quoted_command_arguments() {
        assert_eq!(
            split_words("/run --task 'fix it'").unwrap(),
            ["/run", "--task", "fix it"]
        );
    }

    #[test]
    fn suggests_nearest_slash_command() {
        assert_eq!(nearest("/stats"), "/status");
        assert!(edit_distance("/stats", "/status") < edit_distance("/stats", "/doctor"));
    }

    #[test]
    fn plan_is_emitted_before_any_side_effect() {
        let history = std::env::temp_dir().join(format!(
            "fleet-repl-ordering-{}-history",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&history);
        let mut app = App::new(Vec::new(), PathBuf::from(&history));
        app.input = "is the ledger ok".to_string();
        assert!(!submit(&mut app).unwrap());
        assert!(app.output.contains("PLAN · intent: verify the ledger"));
        assert!(app.pending_plan.is_some());
        assert!(
            !history.exists(),
            "history is a side effect and must wait for confirmation"
        );
    }

    #[test]
    fn ctrl_c_cancels_a_pending_plan_before_execution() {
        let history =
            std::env::temp_dir().join(format!("fleet-repl-cancel-{}-history", std::process::id()));
        let mut app = App::new(Vec::new(), history);
        app.input = "is the ledger ok".to_string();
        assert!(!submit(&mut app).unwrap());
        assert!(app.pending_plan.is_some());

        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(!handle_key(&mut app, ctrl_c).unwrap());
        assert!(app.pending_plan.is_none());
        assert!(app.operation_queue.is_empty());
        assert!(app.output.contains("plan cancelled; no command was run"));
    }

    #[test]
    fn line_editing_keeps_cursor_and_history_consistent() {
        let history =
            std::env::temp_dir().join(format!("fleet-repl-editing-{}-history", std::process::id()));
        let mut app = App::new(vec!["/help".to_string()], history);
        app.input = "ac".to_string();
        app.cursor = 1;
        let plain = KeyModifiers::NONE;
        handle_key(&mut app, KeyEvent::new(KeyCode::Char('b'), plain)).unwrap();
        assert_eq!((app.input.as_str(), app.cursor), ("abc", 2));
        handle_key(&mut app, KeyEvent::new(KeyCode::Home, plain)).unwrap();
        handle_key(&mut app, KeyEvent::new(KeyCode::Delete, plain)).unwrap();
        assert_eq!((app.input.as_str(), app.cursor), ("bc", 0));
        handle_key(&mut app, KeyEvent::new(KeyCode::End, plain)).unwrap();
        handle_key(&mut app, KeyEvent::new(KeyCode::Backspace, plain)).unwrap();
        assert_eq!((app.input.as_str(), app.cursor), ("b", 1));
        history_move(&mut app, -1);
        assert_eq!((app.input.as_str(), app.cursor), ("/help", 5));
    }

    #[test]
    fn completed_child_waits_for_stream_eof_before_dropping_tail() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "exit 0"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let _stdout = child.stdout.take().unwrap();
        let _stderr = child.stderr.take().unwrap();
        let (sender, receiver) = mpsc::channel();
        let mut app = App::new(
            Vec::new(),
            std::env::temp_dir().join(format!("fleet-repl-stream-{}", std::process::id())),
        );
        app.operation = Some(Operation {
            child,
            receiver,
            streams_remaining: 1,
            command: "stream-test".to_string(),
            cancelling: false,
            cancelled_at: None,
        });
        thread::sleep(Duration::from_millis(20));
        drain_operation(&mut app);
        assert!(
            app.operation.is_some(),
            "child exit must not drop open streams"
        );

        sender
            .send(StreamEvent::Data("final chunk".to_string()))
            .unwrap();
        sender.send(StreamEvent::Closed).unwrap();
        for _ in 0..100 {
            drain_operation(&mut app);
            if app.operation.is_none() {
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
        assert!(app.operation.is_none());
        assert!(app.output.contains("final chunk"));
    }
}
