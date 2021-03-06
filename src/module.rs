//! Module loader and resolver.
use std::{
    env,
    path::{Path, PathBuf},
};

pub trait ModuleResolver {
    /// If the module name cannot be resolved, return `None` to
    /// abort the fiber.
    fn resolve(&mut self, importer: &str, module: &str) -> Option<String>;
}

pub trait ModuleLoader {
    fn load(&mut self, name: &str) -> Option<String>;
    fn on_complete(&mut self) { unimplemented!("on_complete is not supported yet") }
}

/// Basic module resolver that just returns the
/// module name as is.
#[derive(Debug)]
pub struct UnitModuleResolver;

impl Default for UnitModuleResolver {
    fn default() -> Self {
        UnitModuleResolver
    }
}

impl UnitModuleResolver {
    pub fn new() -> Self {
        UnitModuleResolver
    }
}

impl ModuleResolver for UnitModuleResolver {
    fn resolve(&mut self, importer: &str, name: &str) -> Option<String> {
        log::debug!("Resolve module: importer={} name={}", importer, name);
        Some(name.to_string())
    }
}

/// Simple module loader that reads source code from files.
///
/// Module import names are used as file paths relative to
/// a root directory.
#[derive(Debug)]
pub struct FileModuleLoader {
    root: PathBuf,
}

impl Default for FileModuleLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl FileModuleLoader {
    /// Create a new loader, using the directory containing the executable
    /// as the root path.
    pub fn new() -> Self {
        let mut dir_path = env::current_exe().expect("Retrieving executable filepath failed");

        // Remove the filename from the path so we just have the
        // containing directory left.
        dir_path.pop();

        Self { root: dir_path }
    }

    pub fn with_root<P: AsRef<Path>>(root: P) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }
}

impl ModuleLoader for FileModuleLoader {
    fn load(&mut self, name: &str) -> Option<String> {
        let mut name = name.to_string();
        if !name.ends_with(".wren") {
            name.push_str(".wren");
        }

        let path = self.root.join(name);
        log::debug!("Importing: {}", path.to_string_lossy());

        match std::fs::read_to_string(path) {
            Ok(source) => Some(source),
            Err(err) => {
                log::error!("Load module source error: {}", err);
                None
            }
        }
    }

    fn on_complete(&mut self) {}
}
