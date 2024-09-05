use io::Stdin;
use std::io;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use regex::Regex;
use structopt::StructOpt;

use crate::action::Action;
use crate::file::{FsItem, FsItemType, ReadFileError};
use std::process::Command;
use walkdir::{DirEntry, WalkDir};

mod action;
mod file;

/// Search for a pattern in a file and display the lines that contain it.
#[derive(StructOpt, Debug)]
struct Cli {
    /// The pattern to look for
    #[structopt(parse(from_os_str), default_value = ".*")]
    pattern: PathBuf,

    #[structopt(long)]
    include_dirs: bool,

    #[structopt(short, long)]
    recursive: bool,
}

fn is_not_hidden(entry: &DirEntry) -> bool {
    !entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}

fn get_items_in_dir(
    directory: &str,
    regex: &Regex,
    recursive: bool,
    include_dirs: bool,
) -> Vec<FsItem> {
    let mut vec = WalkDir::new(directory)
        .max_depth(if recursive { usize::MAX } else { 1 })
        .into_iter()
        .filter_entry(is_not_hidden)
        .inspect(|e| {
            if let Err(e) = e {
                eprintln!("{e:?}");
            }
        })
        .filter_map(Result::ok)
        .filter_map(|e| {
            let is_dir = e.file_type().is_dir();
            if include_dirs || !is_dir {
                let file_name = e.path();
                let str = file_name.to_str().unwrap();
                let str = str.strip_prefix(directory).unwrap_or(str);
                let str = str.trim_start_matches("/").trim_start_matches("\\");
                if regex.is_match(str) {
                    let item_type = if is_dir {
                        FsItemType::Directory
                    } else {
                        FsItemType::File
                    };
                    Some(FsItem {
                        item_type,
                        name: str.to_string(),
                    })
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    vec.sort_unstable_by(|a, b| natural_sort::natural_cmp(&a.name, &b.name));
    vec
}

#[derive(PartialEq)]
enum InputResult {
    Yes,
    No,
    Edit,
}

fn read_user_input(stdin: &Stdin, buffer: &mut String) -> String {
    buffer.clear();
    stdin.read_line(buffer).unwrap();
    return buffer.trim().to_lowercase();
}

fn read_confirmation_user_input(stdin: &Stdin, buffer: &mut String) -> InputResult {
    let mut out = io::stdout();
    loop {
        write!(out, "Do you want to continue? (y/n/e) ").unwrap();
        out.flush().unwrap();

        let res = match read_user_input(stdin, buffer).as_str() {
            "n" => InputResult::No,
            "y" => InputResult::Yes,
            "e" => InputResult::Edit,
            _ => continue,
        };
        return res;
    }
}

enum InputErrorResult {
    Yes,
    No,
}

fn read_error_confirmation_user_input(stdin: &Stdin, buffer: &mut String) -> InputErrorResult {
    let mut out = io::stdout();
    loop {
        write!(out, "Do you want to retry editing? (y/n) ").unwrap();
        out.flush().unwrap();

        return match read_user_input(stdin, buffer).as_str() {
            "n" => InputErrorResult::No,
            "y" => InputErrorResult::Yes,
            _ => continue,
        };
    }
}

fn run_editor(editor_cmd: &mut Command, editor: &str) {
    match editor_cmd.spawn() {
        Ok(mut s) => {
            s.wait().unwrap();
        }
        Err(err) => {
            panic!("Failed to start editor \"{}\": {:?}", editor, err)
        }
    }
}

fn run_edit_process<'a>(
    editor: &str,
    outfile: &mut file::FilesFile,
    files: &'a [FsItem],
) -> Option<Vec<Action<'a>>> {
    let stdin = io::stdin();

    let mut editor_cmd = Command::new(editor);
    editor_cmd.arg(outfile.path());

    let mut input = String::new();
    loop {
        run_editor(&mut editor_cmd, editor);
        match outfile.read(files) {
            Ok(actions) => {
                if actions.is_empty() {
                    println!("Nothing to do");
                    return Some(Vec::new());
                } else {
                    println!("=========Actions=========");
                    for x in actions.iter() {
                        println!("{}", &x)
                    }
                    println!("=========================");
                }

                let result = read_confirmation_user_input(&stdin, &mut input);
                if result != InputResult::Edit {
                    return if result == InputResult::Yes {
                        Some(actions)
                    } else {
                        None
                    };
                }
            }
            Err(err) => match err {
                ReadFileError::Io(_) => panic!("{err:?}"),
                ReadFileError::Parse(str) => {
                    println!("Failed to parse file: {}", str);
                    match read_error_confirmation_user_input(&stdin, &mut input) {
                        InputErrorResult::Yes => (),
                        InputErrorResult::No => return None,
                    }
                }
            },
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Config {
    editor: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            editor: "vim".into(),
        }
    }
}

fn main() {
    let Cli {
        pattern,
        include_dirs,
        recursive,
    }: Cli = Cli::from_args();
    let config: Config = confy::load("renamer", None).unwrap();

    let regex = Regex::new(pattern.to_str().unwrap()).unwrap();
    let files = get_items_in_dir(
        std::env::current_dir().unwrap().to_str().unwrap(),
        &regex,
        recursive,
        include_dirs,
    );

    let mut file = file::FilesFile::write_new(
        tempfile::Builder::new()
            .prefix("renamer")
            .suffix(".ini")
            .tempfile()
            .unwrap(),
        &files,
    )
        .unwrap();

    if let Some(vec) = run_edit_process(config.editor.as_str(), &mut file, &files) {
        for action in vec.iter() {
            if let Err(k) = action.apply() {
                eprintln!(
                    "Failed to apply action for file \"{}\": {}",
                    action.target().name,
                    k
                )
            }
        }
        println!("Applied actions")
    } else {
        println!("Aborted")
    }
}
