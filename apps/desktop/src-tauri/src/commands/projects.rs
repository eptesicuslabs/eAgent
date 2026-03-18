use crate::dto::{
    ProjectSearchEntriesInput, ProjectSearchEntry, ProjectWriteFileInput, ProjectWriteFileResult,
};
use crate::DesktopShellState;
use ecode_desktop_app::UiAction;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;

const SEARCH_LIMIT: usize = 200;
const SKIP_DIRS: &[&str] = &[".git", "node_modules", "target", "dist", ".next"];

#[tauri::command]
pub fn projects_open(path: String, state: State<'_, DesktopShellState>) -> Result<(), String> {
    state
        .app
        .send(UiAction::OpenProject(path))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn projects_search_entries(
    input: ProjectSearchEntriesInput,
) -> Result<Vec<ProjectSearchEntry>, String> {
    let root = PathBuf::from(&input.cwd);
    if !root.exists() {
        return Ok(Vec::new());
    }

    let query = input.query.to_lowercase();
    let mut entries = Vec::new();
    walk_entries(&root, &root, &query, input.limit.unwrap_or(SEARCH_LIMIT), &mut entries)?;
    Ok(entries)
}

#[tauri::command]
pub fn projects_write_file(
    input: ProjectWriteFileInput,
) -> Result<ProjectWriteFileResult, String> {
    let root = PathBuf::from(&input.cwd);
    let destination = root.join(&input.relative_path);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(&destination, input.contents).map_err(|error| error.to_string())?;
    Ok(ProjectWriteFileResult {
        relative_path: input.relative_path,
    })
}

fn walk_entries(
    root: &Path,
    current: &Path,
    query: &str,
    limit: usize,
    entries: &mut Vec<ProjectSearchEntry>,
) -> Result<(), String> {
    if entries.len() >= limit {
        return Ok(());
    }

    let read_dir = fs::read_dir(current).map_err(|error| error.to_string())?;
    for item in read_dir {
        if entries.len() >= limit {
            break;
        }
        let item = item.map_err(|error| error.to_string())?;
        let path = item.path();
        let file_name = item.file_name().to_string_lossy().to_string();
        if path.is_dir() && SKIP_DIRS.iter().any(|entry| *entry == file_name) {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let matches_query = query.is_empty() || relative.to_lowercase().contains(query);
        if matches_query {
            entries.push(ProjectSearchEntry {
                path: relative.clone(),
                is_directory: path.is_dir(),
            });
        }

        if path.is_dir() {
            walk_entries(root, &path, query, limit, entries)?;
        }
    }

    Ok(())
}
