use clap::Parser;
use js_watcher::{
  path_clean::PathClean,
  watch_info::WatchInfo,
  watcher::{SetupOptions, Watcher},
};
use owo_colors::OwoColorize;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
  /// paths or globs of entries to watch, relative to project's path
  entries: Vec<String>,
  /// project's path. Defaults to current working directory
  #[clap(short, long, default_value_t = std::env::current_dir().unwrap().to_str().unwrap().to_string())]
  project_path: String,
  /// command to execute when a change occur
  #[clap(short('x'), long("exec"))]
  exec: Option<String>,
  /// run a command and execute it again for subsequent changes
  #[clap(short('r'), long("run"))]
  run: Option<String>,
  /// suppress output from js_watcher
  #[clap(short, long)]
  silent: bool,
}

fn serialize_watch_info(info: &WatchInfo) -> String {
  let json = json!({
    "event": info.event_to_string(),
    "affectedFile": info.affected_file,
    "affectedEntries": info.affected_entries.as_ref().unwrap_or(&vec![]).iter().map(|item| json!({
      "path": item.path.to_str().unwrap(),
      "deps": item.deps
    })).collect::<serde_json::Value>()
  });

  json.to_string()
}

fn main() {
  let cli = Cli::parse();

  let project_path = Path::new(&cli.project_path);
  let project_root = if project_path.is_absolute() {
    project_path.to_str().unwrap().to_string()
  } else {
    PathBuf::from(std::env::current_dir().unwrap())
      .join(project_path)
      .clean()
      .to_str()
      .unwrap()
      .to_string()
  };
  println!("{} Project: {}", "!".blue(), project_root.blue().bold());
  let entries_input = match &cli.run {
    Some(wexec) => wexec
      .split_ascii_whitespace()
      .filter(|x| x.starts_with(".") || x.starts_with("/"))
      .take(1)
      .map(String::from)
      .collect(),
    _ => {
      if cli.entries.is_empty() {
        panic!("No entry was specified");
      }
      cli.entries.clone()
    }
  };
  if entries_input.is_empty() {
    println!("{} No paths to watch", "!".yellow().bold());
  }

  let mut watcher = Watcher::setup(SetupOptions {
    cache_dir: None,
    debug: None,
    entries: None,
    glob_entries: Some(entries_input),
    project: "test".into(),
    project_root: project_root.clone(),
    supported_paths: None,
  });
  let entries = watcher.get_entries();
  if !entries.is_empty() {
    let mut message = format!("{} Watching for\n", "!".blue());
    for entry in entries.iter().take(4) {
      message += &format!(
        "{}\n",
        entry
          .path
          .to_str()
          .unwrap()
          .replace(project_root.as_str(), ".")
          .blue()
      );
    }
    message = message.trim_end().to_string();
    if entries.len() > 4 {
      message += &format!("and {} other files", entries.len().blue());
    }
    println!("{}", message);
  } else {
    println!(
      "{} There is no entry yet matching \"{}\". They will be picked up once created",
      "!".yellow().bold(),
      cli
        .entries
        .iter()
        .map(String::from)
        .collect::<Vec<String>>()
        .join(" ")
        .blue()
    );
  }

  let cli_exec = match cli.run {
    Some(_) => cli.run.clone(),
    _ => cli.exec.clone(),
  };
  let cwd = project_root.clone();
  watcher.watch(true, move |info| {
    if let Some(cmd_str) = &cli_exec {
      let json_str = serialize_watch_info(&info);
      std::env::set_var("JS_WATCHER_INFO", json_str.clone());
      let args = shellwords::split(&cmd_str.replace("[info]", &json_str)).unwrap();
      Command::new(&args[0])
        .args(&args[1..])
        .current_dir(&cwd)
        .spawn()
        .expect("Failed to execute command")
        .wait()
        .expect("failed to wait for commend");
    }
    Ok(())
  });

  if let Some(cmd_str) = cli.run {
    println!("");
    let args = shellwords::split(&cmd_str).unwrap();
    Command::new(&args[0])
      .args(&args[1..])
      .current_dir(&project_root)
      .spawn()
      .expect("Failed to execute command");
  }

  let (tx, rx) = std::sync::mpsc::channel();
  ctrlc::set_handler(move || tx.send(()).expect("Could not send signal on channel."))
    .expect("Error setting Ctrl-C handler");
  rx.recv().expect("Could not receive from channel.");
  println!("");
}
