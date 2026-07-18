use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Up,
    Down,
}

static LAST_STATUS: Mutex<Option<Status>> = Mutex::new(None);

pub fn get_last_status() -> Option<Status> {
    *LAST_STATUS.lock().expect("status lock poisoned")
}

pub fn set_last_status(status: Status) {
    *LAST_STATUS.lock().expect("status lock poisoned") = Some(status);
}
