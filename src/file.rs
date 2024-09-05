use std::io::{
    BufRead, BufReader, BufWriter, Error as IOError, Lines, Result as IOResult, Seek, SeekFrom,
    Write,
};
use std::path::Path;

use tempfile::NamedTempFile;

use crate::action::{Action, ActionType};

#[derive(Debug, Copy, Clone)]
pub enum FsItemType {
    File,
    Directory,
}

#[derive(Debug)]
pub struct FsItem {
    pub item_type: FsItemType,
    pub name: String,
}

pub fn write_file_names<W: Write>(writer: &mut W, files: &Vec<FsItem>) -> IOResult<()> {
    let mut first = true;
    for x in files {
        let name = &x.name;

        if !first {
            writer.write_all("\n".as_bytes())?;
        }
        writer.write_all(name.as_bytes())?;
        first = false;
    }
    Ok(())
}

pub struct FilesFile(NamedTempFile);

impl FilesFile {
    pub fn path(&self) -> &Path {
        self.0.path()
    }

    pub fn write_new(mut file: NamedTempFile, files: &Vec<FsItem>) -> IOResult<Self> {
        write_file_names(&mut BufWriter::new(&mut file), files)?;
        Ok(FilesFile(file))
    }

    pub fn read<'a>(&mut self, files: &'a [FsItem]) -> Result<Vec<Action<'a>>, ReadFileError> {
        self.0.seek(SeekFrom::Start(0))?;
        read_file_names(BufReader::new(&mut self.0).lines(), files)
    }
}

#[derive(Debug)]
pub enum ReadFileError {
    Io(#[allow(unused)] IOError),
    Parse(&'static str),
}

impl From<IOError> for ReadFileError {
    fn from(error: IOError) -> Self {
        ReadFileError::Io(error)
    }
}

fn create_action<'a>(f: &str, item: &'a FsItem) -> Option<Action<'a>> {
    let line = f.trim();
    let delete = line.starts_with("#");
    let action_type = if delete {
        ActionType::Delete
    } else if line != item.name {
        ActionType::Rename(line.to_string())
    } else {
        return None;
    };
    Some(Action::new(action_type, item))
}

pub fn read_file_names<R: BufRead>(
    mut reader: Lines<R>,
    files: &[FsItem],
) -> Result<Vec<Action>, ReadFileError> {
    let mut actions = vec![];
    let mut files_it = files.iter();
    let mut lines_it = reader
        .by_ref()
        .filter_map(|r| r.ok())
        .filter(|str| !str.trim().is_empty());
    loop {
        if let Some(line) = lines_it.next() {
            if let Some(fs_item) = files_it.next() {
                if let Some(action) = create_action(line.as_str(), fs_item) {
                    actions.push(action)
                }
            } else {
                return Err(ReadFileError::Parse("file contained too many file names"));
            }
        } else if files_it.next().is_some() {
            return Err(ReadFileError::Parse(
                "file did not contain enough file names",
            ));
        } else {
            break;
        }
    }
    Ok(actions)
}
