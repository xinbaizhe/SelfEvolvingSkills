use chrono::Local;

pub(crate) fn now_string() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}
