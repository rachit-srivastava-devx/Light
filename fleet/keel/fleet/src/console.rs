use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs, Wrap},
    Frame, Terminal,
};
use serde_json::{Map, Value};
use std::{
    env,
    fmt::{self, Display},
    fs,
    io::{self, Stdout},
    path::{Path, PathBuf},
    time::Duration,
};

const EXIT_ENV: i32 = 3;
const EXIT_REFUSAL: i32 = 7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Source {
    Receipt,
    Transcript,
    Derived,
}

impl Display for Source {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Receipt => "receipt",
            Self::Transcript => "transcript",
            Self::Derived => "derived",
        })
    }
}

#[derive(Clone, Debug)]
struct DisplayValue<T> {
    value: Option<T>,
    source: Option<Source>,
    reason: String,
}

impl<T> DisplayValue<T> {
    fn known(value: T, source: Source) -> Self {
        Self {
            value: Some(value),
            source: Some(source),
            reason: String::new(),
        }
    }

    fn absent(reason: impl Into<String>) -> Self {
        Self {
            value: None,
            source: None,
            reason: reason.into(),
        }
    }

    fn render_with(&self, format: impl FnOnce(&T) -> String) -> String {
        match (&self.value, self.source) {
            (Some(value), Some(source)) => format!("{} [{}]", format(value), source),
            _ => format!("— ({})", self.reason),
        }
    }
}

#[derive(Clone, Debug)]
struct BoundedCount {
    value: u64,
    total: u64,
    source: Source,
}

impl BoundedCount {
    fn try_new(value: u64, total: u64, source: Source) -> Result<Self, String> {
        if total == 0 {
            return Err("denominator is absent or zero".to_string());
        }
        if value > total {
            return Err(format!("count {} exceeds total {}", value, total));
        }
        Ok(Self {
            value,
            total,
            source,
        })
    }

    fn render(&self) -> String {
        let tenths =
            (u128::from(self.value) * 1000 + u128::from(self.total / 2)) / u128::from(self.total);
        format!(
            "{}/{} ({}.{:01}%) [{}]",
            self.value,
            self.total,
            tenths / 10,
            tenths % 10,
            self.source
        )
    }
}

#[derive(Clone, Debug)]
struct StatusValue {
    glyph: char,
    label: String,
    source: Source,
}

impl StatusValue {
    fn running(source: Source) -> Self {
        Self {
            glyph: '▶',
            label: "Running".to_string(),
            source,
        }
    }

    fn blocked(source: Source) -> Self {
        Self {
            glyph: '■',
            label: "Blocked".to_string(),
            source,
        }
    }

    fn complete(source: Source) -> Self {
        Self {
            glyph: '✓',
            label: "Complete".to_string(),
            source,
        }
    }

    fn unknown(source: Source) -> Self {
        Self {
            glyph: '?',
            label: "Unknown".to_string(),
            source,
        }
    }

    fn render(&self) -> String {
        format!("{} {} [{}]", self.glyph, self.label, self.source)
    }
}

#[derive(Clone, Debug)]
struct AgentRecord {
    name: String,
    name_source: Source,
    lifecycle: StatusValue,
    brief: DisplayValue<String>,
    diff: DisplayValue<String>,
    verdict: DisplayValue<String>,
    receipts: Vec<String>,
}

#[derive(Clone, Debug)]
struct LaneRecord {
    lane_id: String,
    role: String,
    state: LaneState,
    agent: DisplayValue<String>,
    ledger_seq: u64,
    ledger_hash: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LaneState {
    Queued,
    Running,
    Passed,
    Failed,
    Refused,
    Unknown,
}

impl LaneState {
    fn from_str(value: &str) -> Self {
        match value {
            "queued" => Self::Queued,
            "running" => Self::Running,
            "passed" => Self::Passed,
            "failed" => Self::Failed,
            "refused" => Self::Refused,
            _ => Self::Unknown,
        }
    }

    fn render(self) -> String {
        let (glyph, label) = match self {
            Self::Queued => ('·', "Queued"),
            Self::Running => ('▶', "Running"),
            Self::Passed => ('✓', "Passed"),
            Self::Failed => ('✗', "Failed"),
            Self::Refused => ('■', "Refused"),
            Self::Unknown => ('?', "Unknown"),
        };
        format!("{glyph} {label} [{}]", Source::Receipt)
    }
}

#[derive(Clone, Debug)]
struct TaskRecord {
    id: String,
    id_source: Source,
    brief: DisplayValue<String>,
    status: StatusValue,
    cost_cents: DisplayValue<u64>,
    verdict: DisplayValue<BoundedCount>,
    agents: Vec<AgentRecord>,
    receipts: Vec<String>,
    artifact_id: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct ConsoleState {
    tasks: Vec<TaskRecord>,
    lanes: Vec<LaneRecord>,
    absence: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum View {
    Fleet,
    Task,
    Agent,
    Lanes,
}

impl View {
    fn index(self) -> usize {
        match self {
            Self::Fleet => 0,
            Self::Task => 1,
            Self::Agent => 2,
            Self::Lanes => 3,
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Fleet => Self::Task,
            Self::Task => Self::Agent,
            Self::Agent => Self::Lanes,
            Self::Lanes => Self::Fleet,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Fleet => Self::Lanes,
            Self::Task => Self::Fleet,
            Self::Agent => Self::Task,
            Self::Lanes => Self::Agent,
        }
    }
}

pub fn run(args: &[String]) -> Result<(), i32> {
    let selected_id = parse_task_arg(args)?;
    let root = state_dir()?;
    let mut tail = LedgerTail::default();
    let state = load_state()?;
    // The initial load is the immutable-enough baseline; keep the tail in sync with what was
    // read so the first poll only re-reads the bytes a dispatch writes after launch.
    let chain = root.join("ledger/chain.jsonl");
    if chain.exists() {
        if let Ok(metadata) = fs::metadata(&chain) {
            tail.offset = metadata.len();
        }
    }
    let selected_task = match selected_id.as_deref() {
        Some(id) => state
            .tasks
            .iter()
            .position(|task| task.id == id)
            .ok_or(EXIT_REFUSAL)?,
        None => 0,
    };
    let mut app = App {
        state,
        view: View::Fleet,
        task_index: selected_task,
        agent_index: 0,
        tail,
        ledger_path: chain,
    };
    let mut terminal = setup_terminal().map_err(|_| EXIT_ENV)?;
    let result = event_loop(&mut terminal, &mut app);
    restore_terminal(&mut terminal).map_err(|_| EXIT_ENV)?;
    result
}

fn parse_task_arg(args: &[String]) -> Result<Option<String>, i32> {
    if args.is_empty() {
        return Ok(None);
    }
    if args.len() == 2 && args[0] == "--task" && !args[1].trim().is_empty() {
        return Ok(Some(args[1].clone()));
    }
    Err(EXIT_REFUSAL)
}

struct App {
    state: ConsoleState,
    view: View,
    task_index: usize,
    agent_index: usize,
    tail: LedgerTail,
    ledger_path: PathBuf,
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
        // S2: refresh the ledger on every tick so a dispatch running in another terminal shows up
        // without reopening the console. Reads only appended bytes (capped by a 250ms poll), so a
        // long-running task's receipts stream in as they land.
        refresh_ledger(&mut app.state, &mut app.tail, &app.ledger_path);
        terminal
            .draw(|frame| render(frame, frame.area(), app))
            .map_err(|_| EXIT_ENV)?;
        if !event::poll(Duration::from_millis(250)).map_err(|_| EXIT_ENV)? {
            continue;
        }
        let Event::Key(key) = event::read().map_err(|_| EXIT_ENV)? else {
            continue;
        };
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
            KeyCode::Tab | KeyCode::Right => app.view = app.view.next(),
            KeyCode::BackTab | KeyCode::Left => app.view = app.view.previous(),
            KeyCode::Char('1') => app.view = View::Fleet,
            KeyCode::Char('2') => app.view = View::Task,
            KeyCode::Char('3') => app.view = View::Agent,
            KeyCode::Char('4') => app.view = View::Lanes,
            KeyCode::Down | KeyCode::Char('j') => move_selection(app, 1),
            KeyCode::Up | KeyCode::Char('k') => move_selection(app, -1),
            _ => {}
        }
    }
}

fn move_selection(app: &mut App, delta: isize) {
    if app.state.tasks.is_empty() {
        return;
    }
    if app.view == View::Agent {
        if let Some(agent_count) = app
            .state
            .tasks
            .get(app.task_index)
            .map(|task| task.agents.len())
        {
            if agent_count > 0 {
                app.agent_index = wrap_index(app.agent_index, delta, agent_count);
            }
        }
    } else {
        app.task_index = wrap_index(app.task_index, delta, app.state.tasks.len());
        app.agent_index = 0;
    }
}

fn wrap_index(index: usize, delta: isize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let current = index as isize;
    ((current + delta).rem_euclid(len as isize)) as usize
}

fn render(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Length(2),
        ])
        .split(area);
    let titles = [
        Line::from("1 FLEET · running · blocked · cost"),
        Line::from("2 TASK · agents · lifecycle · verdicts"),
        Line::from("3 AGENT · brief · diff · verdict · receipts"),
        Line::from("4 LANES · role · state · ledger"),
    ];
    let tabs = Tabs::new(titles)
        .select(app.view.index())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("fleet console"),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(tabs, chunks[0]);
    match app.view {
        View::Fleet => render_fleet(frame, chunks[1], app),
        View::Task => render_task(frame, chunks[1], app),
        View::Agent => render_agent(frame, chunks[1], app),
        View::Lanes => render_lanes(frame, chunks[1], app),
    }
    let footer =
        Paragraph::new("q quit · ←/→ or tab views · ↑/↓ or j/k select · [source] is provenance")
            .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);
}

fn render_fleet(frame: &mut Frame, area: Rect, app: &App) {
    let running = count_status(&app.state.tasks, "Running");
    let blocked = count_status(&app.state.tasks, "Blocked");
    let cost = total_cost(&app.state.tasks);
    let summary = format!(
        "FLEET · running {} · blocked {} · cost {}",
        render_count(running, app.state.tasks.len() as u64),
        render_count(blocked, app.state.tasks.len() as u64),
        cost.render_with(|value| format!("{} cents", value))
    );
    let rows = if app.state.tasks.is_empty() {
        vec![Row::new(vec![
            Cell::from(absence(&app.state, "no run receipts")),
            Cell::from("—"),
            Cell::from("—"),
        ])]
    } else {
        app.state
            .tasks
            .iter()
            .map(|task| {
                Row::new(vec![
                    Cell::from(format!("{} [{}]", task.id, task.id_source)),
                    Cell::from(task.status.render()),
                    Cell::from(
                        task.cost_cents
                            .render_with(|value| format!("{} cents", value)),
                    ),
                ])
            })
            .collect()
    };
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(30),
            Constraint::Percentage(35),
            Constraint::Percentage(35),
        ],
    )
    .header(
        Row::new(vec!["TASK", "STATUS", "COST"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(Block::default().borders(Borders::ALL).title(summary))
    .column_spacing(1);
    frame.render_widget(table, area);
}

fn render_task(frame: &mut Frame, area: Rect, app: &App) {
    let Some(task) = app.state.tasks.get(app.task_index) else {
        frame.render_widget(absence_paragraph(&app.state, "no task receipt"), area);
        return;
    };
    let title = format!(
        "TASK {} [{}] · {}",
        task.id,
        task.id_source,
        task.status.render()
    );
    let rows = if task.agents.is_empty() {
        vec![Row::new(vec![
            Cell::from(absence(&app.state, "no agent receipt")),
            Cell::from("—"),
            Cell::from(task.verdict.render_with(|value| value.render())),
        ])]
    } else {
        task.agents
            .iter()
            .map(|agent| {
                Row::new(vec![
                    Cell::from(format!("{} [{}]", agent.name, agent.name_source)),
                    Cell::from(agent.lifecycle.render()),
                    Cell::from(agent.verdict.render_with(|value| value.clone())),
                ])
            })
            .collect()
    };
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(32),
            Constraint::Percentage(32),
            Constraint::Percentage(36),
        ],
    )
    .header(
        Row::new(vec!["AGENT", "LIFECYCLE", "VERDICT"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(Block::default().borders(Borders::ALL).title(title));
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(area);
    frame.render_widget(
        Paragraph::new(format!(
            "brief: {}",
            task.brief.render_with(|value| value.clone())
        ))
        .wrap(Wrap { trim: true }),
        layout[0],
    );
    frame.render_widget(table, layout[1]);
}

fn render_agent(frame: &mut Frame, area: Rect, app: &App) {
    let Some(task) = app.state.tasks.get(app.task_index) else {
        frame.render_widget(absence_paragraph(&app.state, "no task receipt"), area);
        return;
    };
    let Some(agent) = task.agents.get(app.agent_index) else {
        frame.render_widget(absence_paragraph(&app.state, "no agent receipt"), area);
        return;
    };
    let title = format!(
        "AGENT {} [{}] · {}",
        agent.name,
        agent.name_source,
        agent.lifecycle.render()
    );
    let text = vec![
        Line::from(format!(
            "brief   {}",
            agent.brief.render_with(|value| value.clone())
        )),
        Line::from(format!(
            "diff    {}",
            agent.diff.render_with(|value| value.clone())
        )),
        Line::from(format!(
            "verdict {}",
            agent.verdict.render_with(|value| value.clone())
        )),
        Line::from(format!("receipts {}", render_receipts(&agent.receipts))),
        Line::from(format!("task    {} [{}]", task.id, task.id_source)),
    ];
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: false }),
        area,
    );
}

/// S2: live status stream. Every row here is a projection of one `lane_status` ledger receipt
/// (contracts/lane-status.v1.json) -- this is the fleet-rs half of the design-graph / lane-status
/// object; the orb-side voice/graph UI that also consumes this shape lives in a different repo
/// and is not rendered here.
fn render_lanes(frame: &mut Frame, area: Rect, app: &App) {
    let rows = if app.state.lanes.is_empty() {
        vec![Row::new(vec![
            Cell::from(absence(&app.state, "no lane_status receipts")),
            Cell::from("—"),
            Cell::from("—"),
            Cell::from("—"),
        ])]
    } else {
        app.state
            .lanes
            .iter()
            .map(|lane| {
                Row::new(vec![
                    Cell::from(format!(
                        "{} ({}) [{}]",
                        lane.lane_id,
                        lane.role,
                        Source::Receipt
                    )),
                    Cell::from(lane.state.render()),
                    Cell::from(lane.agent.render_with(|value| value.clone())),
                    Cell::from(format!(
                        "seq={} hash={}.. [{}]",
                        lane.ledger_seq,
                        lane.ledger_hash.chars().take(14).collect::<String>(),
                        Source::Receipt
                    )),
                ])
            })
            .collect()
    };
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(25),
            Constraint::Percentage(35),
        ],
    )
    .header(
        Row::new(vec!["LANE (ROLE)", "STATE", "AGENT", "LEDGER REF"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("LANES · {} tracked", app.state.lanes.len())),
    )
    .column_spacing(1);
    frame.render_widget(table, area);
}

fn absence(state: &ConsoleState, fallback: &str) -> String {
    state
        .absence
        .first()
        .cloned()
        .unwrap_or_else(|| format!("ABSENCE · {}", fallback))
}

fn absence_paragraph(state: &ConsoleState, fallback: &str) -> Paragraph<'static> {
    Paragraph::new(absence(state, fallback)).block(
        Block::default()
            .borders(Borders::ALL)
            .title("STATED ABSENCE"),
    )
}

fn render_count(value: Option<u64>, total: u64) -> String {
    match value {
        Some(value) => match BoundedCount::try_new(value, total, Source::Derived) {
            Ok(count) => count.render(),
            Err(reason) => format!("— ({})", reason),
        },
        None => "— (no status receipts)".to_string(),
    }
}

fn count_status(tasks: &[TaskRecord], wanted: &str) -> Option<u64> {
    if tasks.is_empty() {
        return None;
    }
    Some(
        tasks
            .iter()
            .filter(|task| task.status.label == wanted)
            .count() as u64,
    )
}

fn total_cost(tasks: &[TaskRecord]) -> DisplayValue<u64> {
    if tasks.is_empty() {
        return DisplayValue::absent("no cost receipts");
    }
    let mut total = 0u64;
    for task in tasks {
        let Some(cost) = task.cost_cents.value else {
            return DisplayValue::absent(format!("cost absent for task {}", task.id));
        };
        let Some(next) = total.checked_add(cost) else {
            return DisplayValue::absent("cost total overflow");
        };
        total = next;
    }
    DisplayValue::known(total, Source::Derived)
}

fn render_receipts(receipts: &[String]) -> String {
    if receipts.is_empty() {
        return "— (no receipt links)".to_string();
    }
    receipts
        .iter()
        .map(|receipt| format!("{} [receipt]", receipt))
        .collect::<Vec<_>>()
        .join(", ")
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

/// Tracks the bytes of ledger/chain.jsonl already consumed so a running console can re-read only
/// the rows appended since the last render. The ledger is append-only, so offsets are stable and
/// selection indices built from earlier rows are preserved across a refresh.
#[derive(Clone, Debug, Default)]
struct LedgerTail {
    /// Absolute file size (in bytes) read up to, trimmed to the last newline so a partial
    /// trailing line is never parsed as a full row.
    offset: u64,
}

/// Read the immutable part of the state once at console launch. The live path is `refresh_ledger`.
fn load_state() -> Result<ConsoleState, i32> {
    let root = state_dir()?;
    let mut state = ConsoleState::default();
    let chain = root.join("ledger/chain.jsonl");
    let mut tail = LedgerTail::default();
    if chain.exists() {
        refresh_ledger(&mut state, &mut tail, &chain);
    } else {
        state
            .absence
            .push("ABSENCE · ledger/chain.jsonl is missing".to_string());
    }
    load_transcripts(&mut state, &root.join("transcripts"));
    load_scorecards(&mut state, &root.join("scorecards"));
    load_submissions(&mut state, &root.join("runs"));
    load_attestations(&mut state, &root.join("attestations"));
    load_artifacts(&mut state, &root.join("artifacts"));
    if state.tasks.is_empty() && state.absence.is_empty() {
        state
            .absence
            .push("ABSENCE · no task receipts found".to_string());
    }
    Ok(state)
}

/// Re-read only the bytes appended to the ledger since the last call and apply any new complete
/// rows in sequence order. Returns the number of new rows examined. This is what makes the
/// console live: called on every event-loop tick, so a `swarm dispatch` running in another
/// terminal is reflected without reopening the console.
fn refresh_ledger(state: &mut ConsoleState, tail: &mut LedgerTail, chain: &Path) -> usize {
    let Ok(metadata) = fs::metadata(chain) else {
        return 0;
    };
    let size = metadata.len();
    if size < tail.offset {
        // The ledger was truncated or replaced underneath us; fall back to a full re-read rather
        // than skipping rows after the new tail.
        tail.offset = 0;
    }
    if size == tail.offset {
        return 0;
    }
    let Ok(text) = fs::read_to_string(chain) else {
        return 0;
    };
    // Trim the tail to the last newline so a half-written trailing row is not treated as a
    // truncated receipt. Any bytes past the last newline remain for the next poll.
    let bytes = text.len() as u64;
    let content = &text[..text.rfind('\n').map(|i| i + 1).unwrap_or(0)];
    // A line begins only after a newline; the very first row (offset 0, no leading newline) is
    // the whole first line.
    let start = if tail.offset == 0 {
        0
    } else {
        (tail.offset as usize).min(content.len())
    };
    if start >= content.len() {
        tail.offset = bytes;
        return 0;
    }
    let new = &content[start..];
    let mut rows = Vec::new();
    for line in new.lines().filter(|line| !line.trim().is_empty()) {
        match serde_json::from_str::<Value>(line) {
            Ok(row) => rows.push(row),
            Err(_) => state
                .absence
                .push("ABSENCE · ledger receipt is unreadable".to_string()),
        }
    }
    load_ledger(state, &rows);
    tail.offset = bytes;
    rows.len()
}

fn load_ledger(state: &mut ConsoleState, rows: &[Value]) {
    for row in rows {
        let Some(object) = row.as_object() else {
            continue;
        };
        let body = object.get("body").and_then(Value::as_object);
        let event = object
            .get("event")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let Some(seq) = object.get("seq").and_then(Value::as_u64) else {
            state
                .absence
                .push("ABSENCE · ledger receipt has no sequence".to_string());
            continue;
        };
        let receipt = object
            .get("hash")
            .and_then(Value::as_str)
            .map(str::to_string);
        let id = body
            .and_then(|value| first_string(value, &["task_id", "run_id", "task"]))
            .or_else(|| receipt.clone())
            .unwrap_or_else(|| format!("receipt-{}", seq));
        match event {
            "run_start" => {
                let task = task_mut(state, &id);
                task.id_source = Source::Receipt;
                task.status = StatusValue::running(Source::Receipt);
                if let Some(brief) = body.and_then(|value| first_string(value, &["task", "brief"]))
                {
                    task.brief = DisplayValue::known(brief, Source::Receipt);
                }
                if let Some(agent) =
                    body.and_then(|value| value.get("agent").and_then(Value::as_str))
                {
                    let agent_record = add_agent(task, agent, Source::Receipt);
                    agent_record.lifecycle = StatusValue::running(Source::Receipt);
                }
                push_receipt(task, receipt);
            }
            "run_end" => {
                let task = task_mut(state, &id);
                task.id_source = Source::Receipt;
                let status = body
                    .and_then(|value| value.get("status"))
                    .and_then(Value::as_str);
                task.status = if status == Some("ok") {
                    StatusValue::complete(Source::Receipt)
                } else {
                    StatusValue::blocked(Source::Receipt)
                };
                if let Some(agent) =
                    body.and_then(|value| value.get("agent").and_then(Value::as_str))
                {
                    let agent_record = add_agent(task, agent, Source::Receipt);
                    agent_record.lifecycle = if status == Some("ok") {
                        StatusValue::complete(Source::Receipt)
                    } else {
                        StatusValue::blocked(Source::Receipt)
                    };
                }
                push_receipt(task, receipt);
            }
            "refusal" => {
                let task = task_mut(state, &id);
                task.id_source = Source::Receipt;
                task.status = StatusValue::blocked(Source::Receipt);
                if let Some(reason) = body.and_then(|value| first_string(value, &["reason"])) {
                    task.brief = DisplayValue::known(reason, Source::Receipt);
                }
                push_receipt(task, receipt);
            }
            "gate_verdict" => {
                let task = task_mut(state, &id);
                task.id_source = Source::Receipt;
                task.verdict = parse_verdict(body, Source::Receipt);
                if let Some(agent) =
                    body.and_then(|value| value.get("agent").and_then(Value::as_str))
                {
                    let agent_record = add_agent(task, agent, Source::Receipt);
                    agent_record.verdict =
                        parse_text_field(body, &["verdict", "status"], Source::Receipt)
                            .unwrap_or_else(|| DisplayValue::absent("no agent verdict receipt"));
                    push_receipt(agent_record, receipt.clone());
                }
                push_receipt(task, receipt);
            }
            "lane_status" => {
                load_lane_status(state, body, seq, receipt.clone());
            }
            "artifact_frozen" | "attested" => {
                let artifact_id =
                    body.and_then(|value| first_string(value, &["artifact_id", "artifact"]));
                let task_index = artifact_id
                    .as_deref()
                    .and_then(|artifact| {
                        state
                            .tasks
                            .iter()
                            .position(|task| task.artifact_id.as_deref() == Some(artifact))
                    })
                    .or_else(|| {
                        state.tasks.iter().rposition(|task| {
                            task.status.label == "Running" && task.artifact_id.is_none()
                        })
                    });
                let task = match task_index {
                    Some(index) => &mut state.tasks[index],
                    None => task_mut(state, &id),
                };
                task.id_source = Source::Receipt;
                if artifact_id.is_some() {
                    task.artifact_id = artifact_id;
                }
                push_receipt(task, receipt);
            }
            _ => {}
        }
        if let Some(cost) = body.and_then(parse_cost) {
            task_mut(state, &id).cost_cents = DisplayValue::known(cost, Source::Receipt);
        }
    }
}

/// S2: a `lane_status` receipt's body matches contracts/lane-status.v1.json. `seq`/`receipt` are
/// the envelope fields the ledger itself stamped -- that link (`ledger_ref` in the contract) is
/// what makes this a projection of a real receipt rather than a free-standing claim.
fn load_lane_status(
    state: &mut ConsoleState,
    body: Option<&Map<String, Value>>,
    seq: u64,
    receipt: Option<String>,
) {
    let Some(body) = body else {
        state
            .absence
            .push("ABSENCE · lane_status receipt has no body".to_string());
        return;
    };
    let Some(lane_id) = first_string(body, &["lane_id"]) else {
        state
            .absence
            .push("ABSENCE · lane_status receipt has no lane_id".to_string());
        return;
    };
    let Some(hash) = receipt else {
        state
            .absence
            .push("ABSENCE · lane_status receipt has no hash".to_string());
        return;
    };
    let role = first_string(body, &["role"]).unwrap_or_else(|| lane_id.clone());
    let lane_state = body
        .get("state")
        .and_then(Value::as_str)
        .map(LaneState::from_str)
        .unwrap_or(LaneState::Unknown);
    let agent = parse_text_field(Some(body), &["agent"], Source::Receipt)
        .unwrap_or_else(|| DisplayValue::absent("no agent receipt"));
    let record = LaneRecord {
        lane_id,
        role,
        state: lane_state,
        agent,
        ledger_seq: seq,
        ledger_hash: hash,
    };
    match state
        .lanes
        .iter()
        .position(|existing| existing.lane_id == record.lane_id)
    {
        // Ledger rows are read in seq order, so the latest lane_status for a lane_id always wins
        // -- this is the "state changes as lanes progress" behaviour the console renders live.
        Some(index) => state.lanes[index] = record,
        None => state.lanes.push(record),
    }
}

fn load_transcripts(state: &mut ConsoleState, directory: &Path) {
    for value in json_files(directory) {
        let Some(object) = value.as_object() else {
            continue;
        };
        let Some(id) = first_string(object, &["task_id", "run_id", "task"]) else {
            continue;
        };
        let task = task_mut(state, &id);
        if task.id_source == Source::Derived {
            task.id_source = Source::Transcript;
        }
        if let Some(agent) = object.get("agent").and_then(Value::as_str) {
            let agent_record = add_agent(task, agent, Source::Transcript);
            agent_record.brief =
                parse_text_field(Some(object), &["brief", "summary"], Source::Transcript)
                    .unwrap_or_else(|| DisplayValue::absent("no transcript brief"));
            agent_record.diff = parse_text_field(Some(object), &["diff"], Source::Transcript)
                .unwrap_or_else(|| DisplayValue::absent("no transcript diff"));
            agent_record.verdict =
                parse_text_field(Some(object), &["verdict", "status"], Source::Transcript)
                    .unwrap_or_else(|| DisplayValue::absent("no transcript verdict"));
            if let Some(lifecycle) = object.get("lifecycle").and_then(Value::as_str) {
                agent_record.lifecycle = status_from_label(lifecycle, Source::Transcript);
            }
        }
    }
}

fn load_scorecards(state: &mut ConsoleState, directory: &Path) {
    for value in json_files(directory) {
        let Some(object) = value.as_object() else {
            continue;
        };
        let Some(id) = first_string(object, &["task_id", "run_id", "task"]) else {
            continue;
        };
        let task = task_mut(state, &id);
        task.id_source = Source::Receipt;
        task.verdict = parse_verdict(Some(object), Source::Receipt);
        if let Some(agent) = object.get("agent").and_then(Value::as_str) {
            let agent_record = add_agent(task, agent, Source::Receipt);
            agent_record.verdict =
                parse_text_field(Some(object), &["verdict", "status"], Source::Receipt)
                    .unwrap_or_else(|| DisplayValue::absent("no scorecard verdict"));
        }
    }
}

fn load_submissions(state: &mut ConsoleState, directory: &Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path().join("submission.json");
        let Ok(bytes) = fs::read(path) else { continue };
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        let Some(body) = value.get("body").and_then(Value::as_object) else {
            continue;
        };
        let Some(id) = first_string(body, &["task_id", "run_id", "task"]) else {
            continue;
        };
        if let Some(agent) = body.get("agent").and_then(Value::as_str) {
            let task = task_mut(state, &id);
            add_agent(task, agent, Source::Transcript);
        }
    }
}

fn load_attestations(state: &mut ConsoleState, directory: &Path) {
    for value in json_files(directory) {
        let Some(root) = value.as_object() else {
            continue;
        };
        let Some(predicate) = root.get("predicate").and_then(Value::as_object) else {
            continue;
        };
        let Some(elements) = predicate.get("elements").and_then(Value::as_object) else {
            continue;
        };
        let Some(task_text) = elements
            .get("sow")
            .and_then(Value::as_object)
            .and_then(|sow| sow.get("task"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        let task = task_mut(state, task_text);
        task.id_source = Source::Receipt;
        if let Some(receipts) = predicate.get("receipts").and_then(Value::as_array) {
            for receipt in receipts.iter().filter_map(Value::as_str) {
                push_receipt(task, Some(receipt.to_string()));
            }
        }
    }
}

fn load_artifacts(state: &mut ConsoleState, directory: &Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let Some(id) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if let Some(task) = state
            .tasks
            .iter_mut()
            .find(|task| task.artifact_id.as_deref() == Some(id.as_str()))
        {
            let diff = fs::read_to_string(entry.path())
                .ok()
                .map(|value| value.chars().take(500).collect::<String>());
            task.agents.iter_mut().for_each(|agent| {
                if agent.diff.value.is_none() {
                    agent.diff = diff.clone().map_or_else(
                        || DisplayValue::absent("artifact diff is unreadable"),
                        |value| DisplayValue::known(value, Source::Receipt),
                    );
                }
            });
        }
    }
}

fn json_files(directory: &Path) -> Vec<Value> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("json"))
        .filter_map(|entry| fs::read(entry.path()).ok())
        .filter_map(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .collect()
}

fn task_mut<'a>(state: &'a mut ConsoleState, id: &str) -> &'a mut TaskRecord {
    if let Some(index) = state.tasks.iter().position(|task| task.id == id) {
        return &mut state.tasks[index];
    }
    state.tasks.push(TaskRecord {
        id: id.to_string(),
        id_source: Source::Derived,
        brief: DisplayValue::absent("no task brief receipt"),
        status: StatusValue::unknown(Source::Derived),
        cost_cents: DisplayValue::absent("no cost receipt"),
        verdict: DisplayValue::absent("no verdict receipt"),
        agents: Vec::new(),
        receipts: Vec::new(),
        artifact_id: None,
    });
    state.tasks.last_mut().expect("task was just inserted")
}

fn add_agent<'a>(task: &'a mut TaskRecord, name: &str, source: Source) -> &'a mut AgentRecord {
    if let Some(index) = task.agents.iter().position(|agent| agent.name == name) {
        return &mut task.agents[index];
    }
    task.agents.push(AgentRecord {
        name: name.to_string(),
        name_source: source,
        lifecycle: StatusValue::unknown(source),
        brief: DisplayValue::absent("no brief receipt"),
        diff: DisplayValue::absent("no diff receipt"),
        verdict: DisplayValue::absent("no verdict receipt"),
        receipts: Vec::new(),
    });
    task.agents.last_mut().expect("agent was just inserted")
}

fn push_receipt<T>(record: &mut T, receipt: Option<String>)
where
    T: ReceiptList,
{
    if let Some(receipt) = receipt {
        record.receipts_mut().push(receipt);
    }
}

trait ReceiptList {
    fn receipts_mut(&mut self) -> &mut Vec<String>;
}

impl ReceiptList for TaskRecord {
    fn receipts_mut(&mut self) -> &mut Vec<String> {
        &mut self.receipts
    }
}

impl ReceiptList for AgentRecord {
    fn receipts_mut(&mut self) -> &mut Vec<String> {
        &mut self.receipts
    }
}

fn first_string(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str).map(str::to_string))
}

fn parse_text_field(
    object: Option<&Map<String, Value>>,
    keys: &[&str],
    source: Source,
) -> Option<DisplayValue<String>> {
    object
        .and_then(|value| first_string(value, keys))
        .map(|value| DisplayValue::known(value, source))
}

fn parse_cost(object: &Map<String, Value>) -> Option<u64> {
    object
        .get("cost_cents")
        .and_then(Value::as_u64)
        .or_else(|| object.get("cost").and_then(Value::as_u64))
}

fn parse_verdict(
    object: Option<&Map<String, Value>>,
    source: Source,
) -> DisplayValue<BoundedCount> {
    let Some(object) = object else {
        return DisplayValue::absent("no verdict receipt");
    };
    let checked = object.get("checked").and_then(Value::as_u64);
    let total = object.get("total").and_then(Value::as_u64);
    match (checked, total) {
        (Some(checked), Some(total)) => match BoundedCount::try_new(checked, total, source) {
            Ok(value) => DisplayValue::known(value, source),
            Err(reason) => DisplayValue::absent(reason),
        },
        _ => DisplayValue::absent("verdict denominator is absent"),
    }
}

fn status_from_label(label: &str, source: Source) -> StatusValue {
    match label.to_ascii_lowercase().as_str() {
        "running" | "in_progress" | "started" => StatusValue::running(source),
        "blocked" | "failed" | "refused" => StatusValue::blocked(source),
        "complete" | "completed" | "ok" | "done" => StatusValue::complete(source),
        _ => StatusValue::unknown(source),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use std::io::Write;

    fn sample_state() -> ConsoleState {
        let verdict = BoundedCount::try_new(74, 521, Source::Receipt).expect("valid sample");
        ConsoleState {
            tasks: vec![TaskRecord {
                id: "task-001".to_string(),
                id_source: Source::Receipt,
                brief: DisplayValue::known(
                    "ship the operator console".to_string(),
                    Source::Receipt,
                ),
                status: StatusValue::running(Source::Receipt),
                cost_cents: DisplayValue::absent("cost not measured"),
                verdict: DisplayValue::known(verdict, Source::Receipt),
                agents: vec![AgentRecord {
                    name: "codex".to_string(),
                    name_source: Source::Transcript,
                    lifecycle: StatusValue::complete(Source::Transcript),
                    brief: DisplayValue::known(
                        "implement the three views".to_string(),
                        Source::Transcript,
                    ),
                    diff: DisplayValue::known(
                        "+ ratatui console\n+ crossterm input".to_string(),
                        Source::Receipt,
                    ),
                    verdict: DisplayValue::known("PASS".to_string(), Source::Receipt),
                    receipts: vec!["blake3:abc".to_string()],
                }],
                receipts: vec!["blake3:def".to_string()],
                artifact_id: None,
            }],
            lanes: vec![
                LaneRecord {
                    lane_id: "builder".to_string(),
                    role: "builder".to_string(),
                    state: LaneState::Running,
                    agent: DisplayValue::known("codex".to_string(), Source::Receipt),
                    ledger_seq: 3,
                    ledger_hash: "blake3:aaaa000011112222333344445555666677778888".to_string(),
                },
                LaneRecord {
                    lane_id: "verifier".to_string(),
                    role: "verifier".to_string(),
                    state: LaneState::Queued,
                    agent: DisplayValue::known("claude".to_string(), Source::Receipt),
                    ledger_seq: 1,
                    ledger_hash: "blake3:bbbb000011112222333344445555666677778888".to_string(),
                },
            ],
            absence: vec!["ABSENCE · cost not measured".to_string()],
        }
    }

    fn snapshot(width: u16, view: View) -> String {
        let backend = TestBackend::new(width, 18);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let app = App {
            state: sample_state(),
            view,
            task_index: 0,
            agent_index: 0,
            tail: LedgerTail::default(),
            ledger_path: PathBuf::new(),
        };
        terminal
            .draw(|frame| render(frame, frame.area(), &app))
            .expect("render");
        let buffer = terminal.backend().buffer();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.cell((x, y)).expect("cell").symbol().to_string())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn bounded_count_rejects_invalid_denominator_and_overflow() {
        assert!(BoundedCount::try_new(1, 0, Source::Derived).is_err());
        assert!(BoundedCount::try_new(2, 1, Source::Derived).is_err());
    }

    #[test]
    fn verdict_gate_runs_good_control_and_bad_fixture() {
        let good: Map<String, Value> =
            serde_json::from_str(include_str!("../tests/fixtures/console/good_verdict.json"))
                .expect("good verdict fixture");
        let bad: Map<String, Value> =
            serde_json::from_str(include_str!("../tests/fixtures/console/bad_verdict.json"))
                .expect("bad verdict fixture");
        assert!(parse_verdict(Some(&good), Source::Receipt).value.is_some());
        assert!(parse_verdict(Some(&bad), Source::Receipt).value.is_none());
    }

    #[test]
    fn render_has_snapshots_at_three_widths() {
        insta::assert_snapshot!("render_fleet_80", snapshot(80, View::Fleet));
        insta::assert_snapshot!("render_fleet_120", snapshot(120, View::Fleet));
        insta::assert_snapshot!("render_fleet_160", snapshot(160, View::Fleet));
        insta::assert_snapshot!("render_task_80", snapshot(80, View::Task));
        insta::assert_snapshot!("render_task_120", snapshot(120, View::Task));
        insta::assert_snapshot!("render_task_160", snapshot(160, View::Task));
        insta::assert_snapshot!("render_agent_80", snapshot(80, View::Agent));
        insta::assert_snapshot!("render_agent_120", snapshot(120, View::Agent));
        insta::assert_snapshot!("render_agent_160", snapshot(160, View::Agent));
        insta::assert_snapshot!("render_lanes_80", snapshot(80, View::Lanes));
        insta::assert_snapshot!("render_lanes_120", snapshot(120, View::Lanes));
        insta::assert_snapshot!("render_lanes_160", snapshot(160, View::Lanes));
    }

    #[test]
    fn lane_status_receipts_update_in_seq_order_and_carry_ledger_ref() {
        let mut state = ConsoleState::default();
        let queued: Map<String, Value> = serde_json::from_str(
            r#"{"lane_id":"builder","role":"builder","state":"queued","agent":"codex"}"#,
        )
        .expect("queued lane body");
        let running: Map<String, Value> = serde_json::from_str(
            r#"{"lane_id":"builder","role":"builder","state":"running","agent":"codex"}"#,
        )
        .expect("running lane body");
        load_lane_status(
            &mut state,
            Some(&queued),
            0,
            Some("blake3:aaa1".to_string()),
        );
        assert_eq!(state.lanes.len(), 1);
        assert_eq!(state.lanes[0].state, LaneState::Queued);
        load_lane_status(
            &mut state,
            Some(&running),
            1,
            Some("blake3:aaa2".to_string()),
        );
        assert_eq!(
            state.lanes.len(),
            1,
            "same lane_id must update in place, not duplicate"
        );
        assert_eq!(state.lanes[0].state, LaneState::Running);
        assert_eq!(state.lanes[0].ledger_seq, 1);
        assert_eq!(state.lanes[0].ledger_hash, "blake3:aaa2");
    }

    fn lane_body(lane_id: &str, state_label: &str, role: &str, seq: u64) -> String {
        // A realistic chain.jsonl row: envelope + lane_status body. `seq` drives both the
        // envelope and (uniquely identifiable) hash so refresh tests can assert which rows ran.
        let hash = format!("blake3:lane{seq}");
        format!(
            "{{\"schema_version\":\"1.0\",\"seq\":{seq},\"hash\":\"{hash}\",\"event\":\"lane_status\",\"body\":{{\"lane_id\":\"{lane_id}\",\"role\":\"{role}\",\"state\":\"{state_label}\"}}}}\n"
        )
    }

    fn append(path: &Path, text: &str) {
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("open ledger")
            .write_all(text.as_bytes())
            .expect("append");
    }

    // S2 regression: the console must be LIVE. This test fails under the original behaviour
    // (load once, never reload) but passes with the incremental tail read. It also guards the
    // implementation against two common regressions: re-reading all rows every tick (would return
    // 3 on the second refresh, not 1) and never refreshing (would return 0 on the first).
    #[test]
    fn refresh_ledger_reads_only_appended_rows_incrementally() {
        let state_dir = mktemp().expect("temp state");
        let chain = state_dir.join("chain.jsonl");
        append(&chain, &lane_body("builder", "queued", "builder", 1));
        append(&chain, &lane_body("builder", "running", "builder", 2));

        let mut state = ConsoleState::default();
        let mut tail = LedgerTail::default();
        let examined = refresh_ledger(&mut state, &mut tail, &chain);
        assert_eq!(
            examined, 2,
            "initial refresh must examine the 2 baseline rows, not 0 (never-refresh regression)"
        );
        assert_eq!(state.lanes.len(), 1);
        assert_eq!(state.lanes[0].state, LaneState::Running);

        // A dispatch in another terminal writes a third row while the console is open.
        append(&chain, &lane_body("builder", "passed", "builder", 3));

        let examined = refresh_ledger(&mut state, &mut tail, &chain);
        assert_eq!(
            examined, 1,
            "second refresh must examine only the 1 new row, not re-read the whole file (re-read-all regression)"
        );
        assert_eq!(state.lanes.len(), 1);
        assert_eq!(state.lanes[0].state, LaneState::Passed);
        assert_eq!(state.lanes[0].ledger_hash, "blake3:lane3");

        fs::remove_dir_all(&state_dir).expect("cleanup");
    }

    // S2: a ledger truncated or replaced underneath the live console must fall back to a full
    // re-read instead of silently keeping stale rows from an offset past the new end.
    #[test]
    fn refresh_ledger_recovers_from_a_rewritten_ledger() {
        let state_dir = mktemp().expect("temp state");
        let chain = state_dir.join("chain.jsonl");
        append(&chain, &lane_body("builder", "queued", "builder", 1));
        append(&chain, &lane_body("builder", "running", "builder", 2));

        let mut state = ConsoleState::default();
        let mut tail = LedgerTail::default();
        let first = refresh_ledger(&mut state, &mut tail, &chain);
        assert_eq!(first, 2);

        // The ledger file is replaced with a single, shorter row.
        fs::write(&chain, lane_body("verifier", "queued", "verifier", 9)).expect("rewrite");
        let second = refresh_ledger(&mut state, &mut tail, &chain);
        assert_eq!(
            second, 1,
            "a shortened/rewritten ledger must fall back to a full re-read, not return 0"
        );
        assert!(
            state.lanes.iter().any(|lane| lane.lane_id == "verifier"),
            "the rewritten ledger's row must be re-read and applied, got lanes: {:?}",
            state
                .lanes
                .iter()
                .map(|l| l.lane_id.clone())
                .collect::<Vec<_>>()
        );
        let verifier = state
            .lanes
            .iter()
            .find(|lane| lane.lane_id == "verifier")
            .expect("verifier lane present");
        assert_eq!(verifier.state, LaneState::Queued);
        assert_eq!(verifier.ledger_seq, 9);

        fs::remove_dir_all(&state_dir).expect("cleanup");
    }

    fn mktemp() -> Result<PathBuf, i32> {
        let output = std::process::Command::new("mktemp")
            .arg("-d")
            .output()
            .map_err(|_| EXIT_ENV)?;
        if !output.status.success() {
            return Err(EXIT_ENV);
        }
        let value = String::from_utf8(output.stdout).map_err(|_| EXIT_ENV)?;
        Ok(PathBuf::from(value.trim()))
    }
}
