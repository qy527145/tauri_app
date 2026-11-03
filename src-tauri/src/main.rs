// release 下不弹出额外的控制台窗口（Windows），不要删。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    {{crate_name}}_lib::run();
}
