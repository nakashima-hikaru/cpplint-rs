use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;

thread_local! {
    static VCS_ROOT_CACHE: RefCell<FxHashMap<PathBuf, PathBuf>> = RefCell::new(FxHashMap::default());
}

pub fn find_vcs_root(dir: &Path) -> PathBuf {
    VCS_ROOT_CACHE.with(|cache_cell| {
        let mut cache = cache_cell.borrow_mut();
        if let Some(root) = cache.get(dir) {
            return root.clone();
        }

        let mut current = dir;
        let mut project_root = current.to_path_buf();
        loop {
            if current.join(".git").exists()
                || current.join(".hg").exists()
                || current.join(".svn").exists()
            {
                project_root = current.to_path_buf();
                break;
            }
            let Some(parent) = current.parent() else {
                break;
            };
            if parent == current {
                break;
            }
            current = parent;
        }

        cache.insert(dir.to_path_buf(), project_root.clone());
        project_root
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathContext {
    pub absolute: Arc<Path>,
    pub display_name: Arc<str>,
    pub repository_relative: Arc<Path>,
    pub root_relative: Arc<Path>,
}

impl PathContext {
    pub fn new(file: &Path, repository: &Path, root: &Path) -> Self {
        let display_name: Arc<str> = Arc::from(file.to_string_lossy());
        Self::new_with_display_name(file, repository, root, display_name)
    }

    pub fn new_with_display_name(
        file: &Path,
        repository: &Path,
        root: &Path,
        display_name: Arc<str>,
    ) -> Self {
        if file == Path::new("-") {
            let dash_path: Arc<Path> = Path::new("-").into();
            return Self {
                absolute: dash_path.clone(),
                display_name,
                repository_relative: dash_path.clone(),
                root_relative: dash_path,
            };
        }

        let absolute_buf = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
        let repository_relative_buf =
            compute_repository_relative(&absolute_buf, file, repository);
        let root_relative_buf = compute_root_relative(&repository_relative_buf, root);

        Self {
            absolute: absolute_buf.into(),
            display_name,
            repository_relative: repository_relative_buf.into(),
            root_relative: root_relative_buf.into(),
        }
    }
}

fn compute_repository_relative(file_abs: &Path, file_raw: &Path, repository: &Path) -> PathBuf {
    if !repository.as_os_str().is_empty() {
        let repo_abs =
            std::fs::canonicalize(repository).unwrap_or_else(|_| repository.to_path_buf());
        if let Ok(relative) = file_abs.strip_prefix(&repo_abs) {
            return relative.to_path_buf();
        }
    }

    if std::fs::canonicalize(file_raw).is_err() {
        return file_raw.to_path_buf();
    }

    let parent_dir = file_abs.parent().unwrap_or(file_abs);
    let project_root = find_vcs_root(parent_dir);

    file_abs
        .strip_prefix(&project_root)
        .unwrap_or(file_abs)
        .to_path_buf()
}

fn compute_root_relative(repo_rel: &Path, root: &Path) -> PathBuf {
    if root.as_os_str().is_empty() {
        return repo_rel.to_path_buf();
    }
    let root_str = root.to_string_lossy();
    if root_str.is_empty() || root_str == "." {
        return repo_rel.to_path_buf();
    }
    repo_rel
        .strip_prefix(root)
        .unwrap_or(repo_rel)
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_context_dash() {
        let ctx = PathContext::new(Path::new("-"), Path::new(""), Path::new(""));
        assert_eq!(&*ctx.display_name, "-");
        assert_eq!(ctx.absolute.as_os_str(), "-");
        assert_eq!(ctx.repository_relative.as_os_str(), "-");
        assert_eq!(ctx.root_relative.as_os_str(), "-");
    }
}
