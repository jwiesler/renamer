use std::fs;
use std::io;

use crate::file::{FsItem, FsItemType};
use core::fmt;

#[derive(Debug)]
pub enum ActionType {
    Delete,
    Rename(String),
}

pub struct Action<'a> {
    act: ActionType,
    item: &'a FsItem,
}

fn should_rename(from: &str, to: &str) -> bool {
    from.eq_ignore_ascii_case(to) || fs::metadata(to).is_err()
}

impl<'a> Action<'a> {
    pub fn action_type(&self) -> &ActionType {
        &self.act
    }

    pub fn target(&self) -> &FsItem {
        self.item
    }

    pub fn apply(&self) -> io::Result<()> {
        match &self.action_type() {
            ActionType::Delete => match self.item.item_type {
                FsItemType::File => fs::remove_file(&self.item.name),
                FsItemType::Directory => fs::remove_dir_all(&self.item.name),
            },
            ActionType::Rename(path) => {
                if should_rename(&self.item.name, path) {
                    fs::rename(&self.item.name, path)
                } else {
                    println!(
                        "Destination file \"{}\" already exists, skipping rename",
                        path
                    );
                    Ok(())
                }
            }
        }
    }

    pub fn new(act: ActionType, item: &'a FsItem) -> Self {
        Action { act, item }
    }
}

impl<'a> fmt::Display for Action<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.act {
            ActionType::Delete => write!(f, "Remove {}", self.item.name),
            ActionType::Rename(n) => write!(f, "{} -> {}", self.item.name, n),
        }
    }
}
