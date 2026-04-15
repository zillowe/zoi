#![allow(dead_code)]

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use zoi::pkg::{offline, pkgdir, sysroot};

fn test_context_mutex() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub struct TestContextGuard {
    _lock: MutexGuard<'static, ()>,
    previous_env: Vec<(String, Option<OsString>)>,
    captured_keys: HashSet<String>,
    previous_cwd: Option<PathBuf>,
    previous_sysroot: Option<std::path::PathBuf>,
    previous_offline: bool,
    previous_pkg_dirs: Vec<PathBuf>,
}

impl TestContextGuard {
    pub fn acquire() -> Self {
        let lock = test_context_mutex()
            .lock()
            .expect("test context lock should not be poisoned");
        let previous_sysroot = sysroot::get_sysroot();
        Self {
            _lock: lock,
            previous_env: Vec::new(),
            captured_keys: HashSet::new(),
            previous_cwd: None,
            previous_sysroot,
            previous_offline: offline::is_offline(),
            previous_pkg_dirs: pkgdir::get_pkg_dirs(),
        }
    }

    pub fn set_env_var(&mut self, key: &str, value: impl AsRef<OsStr>) {
        if self.captured_keys.insert(key.to_string()) {
            self.previous_env
                .push((key.to_string(), std::env::var_os(key)));
        }
        unsafe { std::env::set_var(key, value.as_ref()) };
    }

    pub fn set_sysroot(&self, path: std::path::PathBuf) {
        sysroot::set_sysroot(path);
    }

    pub fn set_current_dir(&mut self, path: &Path) {
        if self.previous_cwd.is_none() {
            self.previous_cwd = std::env::current_dir().ok();
        }
        std::env::set_current_dir(path).expect("test cwd should be set");
    }

    pub fn set_offline(&self, enabled: bool) {
        offline::set_offline(enabled);
    }

    pub fn set_pkg_dirs(&self, dirs: Vec<PathBuf>) {
        pkgdir::set_pkg_dirs(dirs);
    }
}

impl Drop for TestContextGuard {
    fn drop(&mut self) {
        if let Some(previous_cwd) = &self.previous_cwd {
            let _ = std::env::set_current_dir(previous_cwd);
        }
        for (key, value) in self.previous_env.iter().rev() {
            if let Some(previous) = value {
                unsafe { std::env::set_var(key, previous) };
            } else {
                unsafe { std::env::remove_var(key) };
            }
        }
        if let Some(previous) = &self.previous_sysroot {
            sysroot::set_sysroot(previous.clone());
        } else {
            sysroot::clear_sysroot();
        }
        offline::set_offline(self.previous_offline);
        pkgdir::set_pkg_dirs(self.previous_pkg_dirs.clone());
    }
}
