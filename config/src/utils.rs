use std::{sync::{Mutex, PoisonError}, ffi::{OsString, OsStr}, mem, collections::HashMap, env};

pub(crate) struct EnvMutex(Mutex<()>);

impl EnvMutex {
    pub const fn new() -> Self {
        Self(Mutex::new(()))
    }

    pub fn lock(&self) -> EnvMutexGuard {
        let guard = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        EnvMutexGuard { redefined_vars: HashMap::new() }
    }
}

pub(crate) struct EnvMutexGuard {
    redefined_vars: HashMap<OsString, Option<OsString>>,
}

impl Drop for EnvMutexGuard {
    fn drop(&mut self) {
        for (env_name, value) in mem::take(&mut self.redefined_vars) {
            if let Some(value) = value {
                env::set_var(env_name, value);
            } else {
                env::remove_var(env_name);
            }
        }
    }
}

impl EnvMutexGuard {
    pub fn set_env(&mut self, fixture: &str) {
        for line in fixture.split('\n').map(str::trim) {
            if line.is_empty() {
                continue;
            }

            let elements: Vec<_> = line.split('=').collect();
            let variable_name: &OsStr = elements[0].as_ref();
            let variable_value: &OsStr = elements[1].trim_matches('"').as_ref();

            if !self.redefined_vars.contains_key(variable_name) {
                let prev_value = env::var_os(variable_name);
                self.redefined_vars.insert(variable_name.to_os_string(), prev_value);
            }
            env::set_var(variable_name, variable_value);
        }
    }

    pub fn remove_env(&mut self, var_names: &[&str]) {
        for &var_name in var_names {
            let variable_name: &OsStr = var_name.as_ref();
            if !self.redefined_vars.contains_key(variable_name) {
                let prev_value = env::var_os(variable_name);
                self.redefined_vars.insert(variable_name.to_os_string(), prev_value);
            }
            env::remove_var(variable_name);
        }
    }
}