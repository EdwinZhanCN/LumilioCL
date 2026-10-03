//! Starting and supervising the game process.
//!
//! [`command_line`] turns a [`LaunchPlan`] into the argument vector; [`run`]
//! spawns it, streams its output, and reports how it ended. Behavior notes:
//! `docs/behavior/launcher-spine.md`.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::activity::CancellationToken;
use crate::launch::LaunchPlan;
use crate::launch_session::{LaunchFailure, LaunchSignal};
use crate::tuning::{LaunchTuning, split_words};

/// How long a process must stay alive before it counts as running when it
/// never prints the marker line.
pub const DEFAULT_SETTLE: Duration = Duration::from_secs(5);

/// How long to keep collecting output after the process itself has ended.
const DRAIN_LIMIT: Duration = Duration::from_millis(500);

/// Text the game prints once it has read its arguments and is starting up.
const STARTED_MARKER: &str = "Setting user:";

/// Flags every launch gets unless the user's own arguments already set the
/// same property. They close a remote-code-execution hole in the logging
/// library of older game versions.
const DEFAULT_PROPERTIES: [(&str, &str); 4] = [
    ("-Dlog4j2.formatMsgNoLookups=", "true"),
    ("-Djava.rmi.server.useCodebaseOnly=", "true"),
    ("-Dcom.sun.jndi.rmi.object.trustURLCodebase=", "false"),
    ("-Dcom.sun.jndi.cosnaming.object.trustURLCodebase=", "false"),
];

#[derive(Clone, Debug)]
pub struct GameOptions {
    java: PathBuf,
    max_memory_mb: Option<u32>,
    min_memory_mb: Option<u32>,
    extra_jvm_arguments: Vec<String>,
    window: Option<(u32, u32)>,
    fullscreen: bool,
    extra_game_arguments: Vec<String>,
    legacy_server: Option<(String, Option<u16>)>,
    wrapper: Vec<String>,
    environment: Vec<(String, String)>,
    settle: Duration,
}

impl GameOptions {
    #[must_use]
    pub fn new(java: impl Into<PathBuf>) -> Self {
        Self {
            java: java.into(),
            max_memory_mb: None,
            min_memory_mb: None,
            extra_jvm_arguments: Vec::new(),
            window: None,
            fullscreen: false,
            extra_game_arguments: Vec::new(),
            legacy_server: None,
            wrapper: Vec::new(),
            environment: Vec::new(),
            settle: DEFAULT_SETTLE,
        }
    }

    #[must_use]
    pub fn with_memory(mut self, min_mb: Option<u32>, max_mb: Option<u32>) -> Self {
        self.min_memory_mb = min_mb.filter(|value| *value > 0);
        self.max_memory_mb = max_mb.filter(|value| *value > 0);
        self
    }

    #[must_use]
    pub fn with_jvm_arguments(mut self, arguments: impl IntoIterator<Item = String>) -> Self {
        self.extra_jvm_arguments = arguments.into_iter().collect();
        self
    }

    /// Applies the launch tuning: window, fullscreen, JVM and game arguments,
    /// the wrapper command and the environment. Commands that run before and
    /// after the game are not part of the process line; see [`run_hook`].
    #[must_use]
    pub fn with_tuning(mut self, tuning: &LaunchTuning) -> Self {
        self.extra_jvm_arguments.clone_from(&tuning.jvm_arguments);
        self.window = tuning.window_width.zip(tuning.window_height);
        self.fullscreen = tuning.fullscreen == Some(true);
        self.extra_game_arguments.clone_from(&tuning.game_arguments);
        self.wrapper = tuning
            .wrapper
            .as_deref()
            .and_then(split_words)
            .unwrap_or_default();
        self.environment = tuning
            .environment
            .iter()
            .map(|variable| (variable.name.clone(), variable.value.clone()))
            .collect();
        self
    }

    /// Joins this server with the older `--server`/`--port` arguments, for
    /// game versions that predate direct quick play.
    #[must_use]
    pub fn with_legacy_server(mut self, host: impl Into<String>, port: Option<u16>) -> Self {
        self.legacy_server = Some((host.into(), port));
        self
    }

    /// Extra environment for the game process, after the tuning's own.
    #[must_use]
    pub fn with_environment(
        mut self,
        variables: impl IntoIterator<Item = (String, String)>,
    ) -> Self {
        self.environment.extend(variables);
        self
    }

    /// The environment the game process gets on top of the launcher's own.
    #[must_use]
    pub fn environment(&self) -> &[(String, String)] {
        &self.environment
    }

    #[must_use]
    pub const fn with_settle(mut self, settle: Duration) -> Self {
        self.settle = settle;
        self
    }
}

/// The full argument vector, program first.
///
/// Order: Java, memory limits, default safety properties, the user's JVM
/// arguments, the plan's JVM arguments (natives path, classpath, …), the main
/// class, then the game arguments. Memory limits and default properties yield
/// to the user's arguments: when the user already sets `-Xmx`, `-Xms`, or a
/// default property, the launcher does not add its own. A minimum above the
/// maximum is dropped, since the JVM refuses to start with it.
#[must_use]
pub fn command_line(plan: &LaunchPlan, options: &GameOptions) -> Vec<String> {
    let user = &options.extra_jvm_arguments;
    let user_sets = |prefix: &str| user.iter().any(|argument| argument.starts_with(prefix));

    let mut line = options.wrapper.clone();
    line.push(options.java.to_string_lossy().into_owned());
    if let Some(max) = options.max_memory_mb
        && !user_sets("-Xmx")
    {
        line.push(format!("-Xmx{max}m"));
    }
    if let Some(min) = options.min_memory_mb
        && !user_sets("-Xms")
        && options.max_memory_mb.is_none_or(|max| min <= max)
    {
        line.push(format!("-Xms{min}m"));
    }
    for (prefix, value) in DEFAULT_PROPERTIES {
        if !user_sets(prefix) {
            line.push(format!("{prefix}{value}"));
        }
    }
    line.extend(user.iter().cloned());
    line.extend(plan.jvm_arguments().iter().cloned());
    line.push(plan.main_class().to_owned());
    line.extend(plan.game_arguments().iter().cloned());
    // The window and the user's own game arguments come last, and a flag the
    // game already has (from the release or the user) is not added twice.
    let has = |flag: &str| {
        plan.game_arguments()
            .iter()
            .chain(&options.extra_game_arguments)
            .any(|argument| argument == flag)
    };
    if let Some((width, height)) = options.window {
        if !has("--width") {
            line.extend(["--width".to_owned(), width.to_string()]);
        }
        if !has("--height") {
            line.extend(["--height".to_owned(), height.to_string()]);
        }
    }
    if options.fullscreen && !has("--fullscreen") {
        line.push("--fullscreen".to_owned());
    }
    if let Some((host, port)) = &options.legacy_server {
        line.extend(["--server".to_owned(), host.clone()]);
        if let Some(port) = port {
            line.extend(["--port".to_owned(), port.to_string()]);
        }
    }
    line.extend(options.extra_game_arguments.iter().cloned());
    line
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogStream {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GameEvent {
    Spawned {
        pid: Option<u32>,
    },
    /// The game is up: it printed its start marker or outlived the settle time.
    Running,
    Line {
        stream: LogStream,
        text: String,
    },
    Exited {
        code: Option<i32>,
        ran_for: Duration,
    },
}

impl GameEvent {
    /// The launch signal this event means, if any.
    #[must_use]
    pub fn signal(&self, was_running: bool) -> Option<LaunchSignal> {
        match self {
            Self::Running => Some(LaunchSignal::Running),
            Self::Exited { code, .. } if was_running => Some(LaunchSignal::Exited { code: *code }),
            Self::Exited { code, .. } => Some(LaunchSignal::Failed(LaunchFailure::ExitedEarly {
                code: *code,
            })),
            Self::Spawned { .. } | Self::Line { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum ProcessError {
    /// The executable could not be started.
    Spawn { program: String, reason: String },
    /// The command line has no program.
    EmptyCommand,
}

impl Display for ProcessError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { program, reason } => write!(f, "could not start {program}: {reason}"),
            Self::EmptyCommand => f.write_str("empty command line"),
        }
    }
}

impl Error for ProcessError {}

/// How a supervised run ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GameExit {
    pub code: Option<i32>,
    pub ran_for: Duration,
    /// The run was stopped through the cancellation token.
    pub killed: bool,
    /// The game was up before it ended.
    pub was_running: bool,
}

/// Runs `line` (program first) in `working_dir`, sending events until it ends.
///
/// The working directory is created if missing. Cancelling the token kills the
/// process. Dropped receivers never stop the game; events are just discarded.
pub async fn run(
    line: &[String],
    working_dir: &Path,
    events: mpsc::UnboundedSender<GameEvent>,
    settle: Duration,
    cancel: &CancellationToken,
) -> Result<GameExit, ProcessError> {
    run_with_environment(line, working_dir, &[], events, settle, cancel).await
}

/// Like [`run`], with extra environment variables for the process.
pub async fn run_with_environment(
    line: &[String],
    working_dir: &Path,
    environment: &[(String, String)],
    events: mpsc::UnboundedSender<GameEvent>,
    settle: Duration,
    cancel: &CancellationToken,
) -> Result<GameExit, ProcessError> {
    let (program, arguments) = line.split_first().ok_or(ProcessError::EmptyCommand)?;
    std::fs::create_dir_all(working_dir).map_err(|error| ProcessError::Spawn {
        program: program.clone(),
        reason: error.to_string(),
    })?;
    let mut child = Command::new(program)
        .args(arguments)
        .envs(environment.iter().map(|(name, value)| (name, value)))
        .current_dir(working_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| ProcessError::Spawn {
            program: program.clone(),
            reason: error.to_string(),
        })?;
    let started = Instant::now();
    let _ = events.send(GameEvent::Spawned { pid: child.id() });

    let (lines_tx, mut lines_rx) = mpsc::unbounded_channel();
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(tokio::spawn(pump(
            stdout,
            LogStream::Stdout,
            lines_tx.clone(),
        )));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(tokio::spawn(pump(
            stderr,
            LogStream::Stderr,
            lines_tx.clone(),
        )));
    }
    drop(lines_tx);

    let mut running = false;
    let mut killed = false;
    let settle_at = tokio::time::sleep(settle);
    tokio::pin!(settle_at);
    // Every line goes through here, including those collected after the
    // process ended: a game that prints its marker and exits in the same
    // instant must still count as having started.
    let deliver = |running: &mut bool, stream: LogStream, text: String| {
        if !*running && text.contains(STARTED_MARKER) {
            *running = true;
            let _ = events.send(GameEvent::Running);
        }
        let _ = events.send(GameEvent::Line { stream, text });
    };
    let status = loop {
        tokio::select! {
            status = child.wait() => break status,
            Some((stream, text)) = lines_rx.recv() => deliver(&mut running, stream, text),
            () = &mut settle_at, if !running => {
                running = true;
                let _ = events.send(GameEvent::Running);
            }
            () = cancel.cancelled(), if !killed => {
                killed = true;
                let _ = child.start_kill();
            }
        }
    };
    // Drain what the readers still hold so the log is complete. A child the
    // game left behind may keep a pipe open forever, so the wait is bounded.
    for mut reader in readers {
        if tokio::time::timeout(DRAIN_LIMIT, &mut reader)
            .await
            .is_err()
        {
            reader.abort();
        }
    }
    while let Ok((stream, text)) = lines_rx.try_recv() {
        deliver(&mut running, stream, text);
    }

    let code = status.ok().and_then(|status| status.code());
    let ran_for = started.elapsed();
    let _ = events.send(GameEvent::Exited { code, ran_for });
    Ok(GameExit {
        code,
        ran_for,
        killed,
        was_running: running,
    })
}

/// How a user command (before launch or after exit) ended.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookOutcome {
    pub code: Option<i32>,
    /// What it printed, stdout and stderr, in the order lines arrived.
    pub lines: Vec<(LogStream, String)>,
    /// It was stopped because the run was cancelled or it ran too long.
    pub stopped: bool,
}

impl HookOutcome {
    #[must_use]
    pub fn succeeded(&self) -> bool {
        !self.stopped && self.code == Some(0)
    }
}

/// How long a command after the game may run before it is stopped.
pub const HOOK_TIME_LIMIT: Duration = Duration::from_secs(60);

/// Runs a command the user wrote, through the system shell, in `working_dir`
/// with `environment` added. Output is collected, not shown live. The command
/// stops when `cancel` fires or after `limit`.
pub async fn run_hook(
    command: &str,
    working_dir: &Path,
    environment: &[(String, String)],
    limit: Option<Duration>,
    cancel: &CancellationToken,
) -> Result<HookOutcome, ProcessError> {
    std::fs::create_dir_all(working_dir).map_err(|error| ProcessError::Spawn {
        program: command.to_owned(),
        reason: error.to_string(),
    })?;
    let mut shell = if cfg!(windows) {
        let mut shell = Command::new("cmd");
        shell.arg("/C").arg(command);
        shell
    } else {
        let mut shell = Command::new("sh");
        shell.arg("-c").arg(command);
        shell
    };
    let mut child = shell
        .envs(environment.iter().map(|(name, value)| (name, value)))
        .current_dir(working_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| ProcessError::Spawn {
            program: command.to_owned(),
            reason: error.to_string(),
        })?;
    let (lines_tx, mut lines_rx) = mpsc::unbounded_channel();
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(tokio::spawn(pump(
            stdout,
            LogStream::Stdout,
            lines_tx.clone(),
        )));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(tokio::spawn(pump(
            stderr,
            LogStream::Stderr,
            lines_tx.clone(),
        )));
    }
    drop(lines_tx);

    let mut stopped = false;
    let mut lines = Vec::new();
    let deadline = tokio::time::sleep(limit.unwrap_or(Duration::MAX));
    tokio::pin!(deadline);
    let status = loop {
        tokio::select! {
            status = child.wait() => break status,
            Some(line) = lines_rx.recv() => lines.push(line),
            () = &mut deadline, if limit.is_some() && !stopped => {
                stopped = true;
                let _ = child.start_kill();
            }
            () = cancel.cancelled(), if !stopped => {
                stopped = true;
                let _ = child.start_kill();
            }
        }
    };
    for mut reader in readers {
        if tokio::time::timeout(DRAIN_LIMIT, &mut reader)
            .await
            .is_err()
        {
            reader.abort();
        }
    }
    while let Ok(line) = lines_rx.try_recv() {
        lines.push(line);
    }
    Ok(HookOutcome {
        code: status.ok().and_then(|status| status.code()),
        lines,
        stopped,
    })
}

async fn pump<R: AsyncRead + Unpin>(
    reader: R,
    stream: LogStream,
    sink: mpsc::UnboundedSender<(LogStream, String)>,
) {
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(text)) = lines.next_line().await {
        if sink.send((stream, text)).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::HostProfile;
    use crate::launch::{LaunchContext, LaunchDirectories};
    use crate::release::ReleaseManifest;

    fn plan() -> LaunchPlan {
        let manifest = ReleaseManifest::decode_json(
            r#"{
                "id": "1.0",
                "mainClass": "net.example.Main",
                "minecraftArguments": "--username ${auth_player_name}",
                "libraries": []
            }"#,
        )
        .unwrap();
        let context = LaunchContext::new(
            HostProfile::current(),
            LaunchDirectories::under("/tmp/launcher"),
        )
        .with_value("auth_player_name", "Steve");
        manifest.build_launch_plan(&context).unwrap()
    }

    fn position(line: &[String], needle: &str) -> usize {
        line.iter().position(|item| item == needle).unwrap()
    }

    #[test]
    fn command_line_orders_java_memory_user_plan_main_and_game() {
        let plan = plan();
        let options = GameOptions::new("/jdk/bin/java")
            .with_memory(Some(512), Some(2048))
            .with_jvm_arguments(["-Dcustom=1".to_owned()]);
        let line = command_line(&plan, &options);
        assert_eq!(line[0], "/jdk/bin/java");
        assert_eq!(line[1], "-Xmx2048m");
        assert_eq!(line[2], "-Xms512m");
        assert!(position(&line, "-Dcustom=1") < position(&line, "-cp"));
        assert!(position(&line, "-cp") < position(&line, "net.example.Main"));
        let main = position(&line, "net.example.Main");
        assert_eq!(line[main + 1..], ["--username", "Steve"]);
        assert!(line.contains(&"-Dlog4j2.formatMsgNoLookups=true".to_owned()));
    }

    #[test]
    fn user_arguments_override_launcher_defaults() {
        let plan = plan();
        let options = GameOptions::new("java")
            .with_memory(Some(512), Some(2048))
            .with_jvm_arguments([
                "-Xmx8g".to_owned(),
                "-Xms1g".to_owned(),
                "-Dlog4j2.formatMsgNoLookups=false".to_owned(),
            ]);
        let line = command_line(&plan, &options);
        assert_eq!(line.iter().filter(|a| a.starts_with("-Xmx")).count(), 1);
        assert_eq!(line.iter().filter(|a| a.starts_with("-Xms")).count(), 1);
        assert!(line.contains(&"-Xmx8g".to_owned()));
        assert!(!line.contains(&"-Dlog4j2.formatMsgNoLookups=true".to_owned()));
    }

    #[test]
    fn a_minimum_above_the_maximum_is_dropped() {
        let options = GameOptions::new("java").with_memory(Some(4096), Some(1024));
        let line = command_line(&plan(), &options);
        assert!(line.contains(&"-Xmx1024m".to_owned()));
        assert!(!line.iter().any(|a| a.starts_with("-Xms")));
        let zero = GameOptions::new("java").with_memory(Some(0), Some(0));
        let line = command_line(&plan(), &zero);
        assert!(!line.iter().any(|a| a.starts_with("-Xm")));
    }

    #[test]
    fn events_map_to_launch_signals() {
        assert_eq!(
            GameEvent::Running.signal(false),
            Some(LaunchSignal::Running)
        );
        let exit = GameEvent::Exited {
            code: Some(1),
            ran_for: Duration::ZERO,
        };
        assert_eq!(
            exit.signal(true),
            Some(LaunchSignal::Exited { code: Some(1) })
        );
        assert_eq!(
            exit.signal(false),
            Some(LaunchSignal::Failed(LaunchFailure::ExitedEarly {
                code: Some(1)
            }))
        );
        assert_eq!(GameEvent::Spawned { pid: None }.signal(false), None);
    }

    #[cfg(unix)]
    mod supervised {
        use super::*;
        use std::os::unix::fs::PermissionsExt;

        fn script(dir: &Path, body: &str) -> Vec<String> {
            let path = dir.join("fake-java.sh");
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            vec![path.to_string_lossy().into_owned()]
        }

        async fn collect(mut rx: mpsc::UnboundedReceiver<GameEvent>) -> Vec<GameEvent> {
            let mut all = Vec::new();
            while let Some(event) = rx.recv().await {
                all.push(event);
            }
            all
        }

        #[tokio::test]
        async fn reports_start_marker_output_and_exit_code() {
            let dir = tempfile::tempdir().unwrap();
            let line = script(
                dir.path(),
                "echo 'Setting user: Steve'\necho 'oops' >&2\nexit 3",
            );
            let (tx, rx) = mpsc::unbounded_channel();
            let exit = run(
                &line,
                &dir.path().join("game"),
                tx,
                Duration::from_secs(30),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
            assert_eq!(exit.code, Some(3));
            assert!(exit.was_running && !exit.killed);

            let events = collect(rx).await;
            assert!(matches!(events[0], GameEvent::Spawned { pid: Some(_) }));
            let running = events
                .iter()
                .position(|e| *e == GameEvent::Running)
                .unwrap();
            let marker = events
                .iter()
                .position(
                    |e| matches!(e, GameEvent::Line { text, .. } if text.contains("Setting user")),
                )
                .unwrap();
            assert!(
                running < marker,
                "Running is announced with the marker line"
            );
            assert!(events.iter().any(|e| matches!(
                e,
                GameEvent::Line { stream: LogStream::Stderr, text } if text == "oops"
            )));
            assert!(matches!(
                events.last(),
                Some(GameEvent::Exited { code: Some(3), .. })
            ));
        }

        /// Output that is only collected after the process ended (the reader
        /// tasks can lag behind the exit under load) must still be able to mark
        /// the game as started. A background subshell prints the marker after
        /// its parent is already gone, which forces exactly that path.
        #[tokio::test]
        async fn a_marker_collected_after_the_exit_still_counts_as_running() {
            let dir = tempfile::tempdir().unwrap();
            let line = script(
                dir.path(),
                "(sleep 0.2; echo 'Setting user: Steve') &\nexit 0",
            );
            let (tx, rx) = mpsc::unbounded_channel();
            let exit = run(
                &line,
                dir.path(),
                tx,
                Duration::from_secs(30),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
            assert!(exit.was_running);
            let events = collect(rx).await;
            let running = events
                .iter()
                .position(|e| *e == GameEvent::Running)
                .unwrap();
            let exited = events
                .iter()
                .position(|e| matches!(e, GameEvent::Exited { .. }))
                .unwrap();
            assert!(running < exited, "Running precedes the exit event");
        }

        #[tokio::test]
        async fn an_early_exit_never_counts_as_running() {
            let dir = tempfile::tempdir().unwrap();
            let line = script(dir.path(), "echo boom >&2\nexit 1");
            let (tx, rx) = mpsc::unbounded_channel();
            let exit = run(
                &line,
                dir.path(),
                tx,
                Duration::from_secs(30),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
            assert!(!exit.was_running);
            let events = collect(rx).await;
            assert!(!events.contains(&GameEvent::Running));
            let last = events.last().unwrap();
            assert_eq!(
                last.signal(exit.was_running),
                Some(LaunchSignal::Failed(LaunchFailure::ExitedEarly {
                    code: Some(1)
                }))
            );
        }

        #[tokio::test]
        async fn surviving_the_settle_time_counts_as_running_and_cancel_kills() {
            let dir = tempfile::tempdir().unwrap();
            let line = script(dir.path(), "sleep 30");
            let (tx, rx) = mpsc::unbounded_channel();
            let cancel = CancellationToken::new();
            let stopper = cancel.clone();
            let canceller = tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(400)).await;
                stopper.cancel();
            });
            let exit = run(&line, dir.path(), tx, Duration::from_millis(100), &cancel)
                .await
                .unwrap();
            canceller.await.unwrap();
            assert!(exit.killed && exit.was_running);
            assert!(exit.ran_for < Duration::from_secs(10));
            assert!(collect(rx).await.contains(&GameEvent::Running));
        }

        #[tokio::test]
        async fn a_missing_program_is_a_spawn_error() {
            let dir = tempfile::tempdir().unwrap();
            let (tx, _rx) = mpsc::unbounded_channel();
            let error = run(
                &["/definitely/not/here".to_owned()],
                dir.path(),
                tx,
                Duration::from_secs(1),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
            assert!(matches!(error, ProcessError::Spawn { .. }));
            let (tx, _rx) = mpsc::unbounded_channel();
            assert!(matches!(
                run(
                    &[],
                    dir.path(),
                    tx,
                    Duration::ZERO,
                    &CancellationToken::new()
                )
                .await
                .unwrap_err(),
                ProcessError::EmptyCommand
            ));
        }
    }

    #[test]
    fn tuning_puts_the_wrapper_first_and_the_window_and_user_game_arguments_last() {
        let tuning = LaunchTuning {
            window_width: Some(1280),
            window_height: Some(720),
            fullscreen: Some(true),
            game_arguments: vec!["--demo".into()],
            wrapper: Some("nice -n 5".into()),
            ..LaunchTuning::default()
        };
        let line = command_line(
            &plan(),
            &GameOptions::new("/jdk/bin/java").with_tuning(&tuning),
        );
        assert_eq!(&line[..4], ["nice", "-n", "5", "/jdk/bin/java"]);
        let tail = &line[line.len() - 6..];
        assert_eq!(
            tail,
            [
                "--width",
                "1280",
                "--height",
                "720",
                "--fullscreen",
                "--demo"
            ]
        );
        assert!(position(&line, "--width") > position(&line, "--username"));
        assert!(position(&line, "--demo") > position(&line, "--fullscreen"));
    }

    #[test]
    fn a_window_flag_the_user_already_gave_is_not_added_twice() {
        let tuning = LaunchTuning {
            window_width: Some(1280),
            window_height: Some(720),
            game_arguments: vec!["--width".into(), "640".into()],
            ..LaunchTuning::default()
        };
        let line = command_line(&plan(), &GameOptions::new("java").with_tuning(&tuning));
        assert_eq!(line.iter().filter(|item| *item == "--width").count(), 1);
        assert_eq!(line.iter().filter(|item| *item == "--height").count(), 1);
        assert!(!line.contains(&"1280".to_owned()), "the user's width wins");
        // Off or unset fullscreen adds nothing.
        let off = LaunchTuning {
            fullscreen: Some(false),
            ..LaunchTuning::default()
        };
        let line = command_line(&plan(), &GameOptions::new("java").with_tuning(&off));
        assert!(!line.contains(&"--fullscreen".to_owned()));
    }

    #[tokio::test]
    async fn a_hook_reports_its_exit_code_output_and_stops_on_cancel() {
        let dir = tempfile::tempdir().unwrap();
        let never = CancellationToken::new();
        let outcome = run_hook(
            "echo out; echo err 1>&2; echo $GREETING; exit 4",
            dir.path(),
            &[("GREETING".to_owned(), "hi".to_owned())],
            None,
            &never,
        )
        .await
        .unwrap();
        assert_eq!(outcome.code, Some(4));
        assert!(!outcome.succeeded());
        let texts: Vec<&str> = outcome
            .lines
            .iter()
            .map(|(_, text)| text.as_str())
            .collect();
        assert!(texts.contains(&"out") && texts.contains(&"err") && texts.contains(&"hi"));

        let ok = run_hook("true", dir.path(), &[], None, &never)
            .await
            .unwrap();
        assert!(ok.succeeded());

        let slow = run_hook(
            "sleep 30",
            dir.path(),
            &[],
            Some(Duration::from_millis(100)),
            &never,
        )
        .await
        .unwrap();
        assert!(slow.stopped && !slow.succeeded());
        let cancel = CancellationToken::new();
        cancel.cancel();
        let stopped = run_hook("sleep 30", dir.path(), &[], None, &cancel)
            .await
            .unwrap();
        assert!(stopped.stopped);
    }
}
