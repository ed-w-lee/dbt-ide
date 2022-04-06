use dbt_project_parser::project::DbtProject;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn find_dbt_project(root_path: &Path) -> Result<DbtProject, String> {
    let mut err_msg = "couldn't find dbt_project.yml".to_string();
    for entry in WalkDir::new(root_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let f_name = entry.file_name().to_string_lossy();
        if f_name == "dbt_project.yml" {
            match DbtProject::from_root(entry.path()) {
                Ok(project) => return Ok(project),
                Err(msg) => err_msg = msg,
            }
        }
    }
    Err(err_msg)
}
