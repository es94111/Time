// 防止 Windows 正式版彈出主控台視窗。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    activity_tracker_lib::run();
}
