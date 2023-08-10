#[allow(unused_imports)] // This JsValue is needed for some reason
use wasm_bindgen::prelude::{wasm_bindgen, JsValue};

#[wasm_bindgen(module = "/js/utils.js")]
extern "C" {
    pub async fn wait_for_pointer_down();
    pub async fn wait_for_pointer_up();
    pub fn get_pointer_x() -> i32;
    pub fn get_pointer_y() -> i32;
    pub async fn wait_for_pointer_move();
    pub async fn wait_for_right_button_down();
}

pub fn get_pointer_position() -> (i32, i32) {
    let x = get_pointer_x();
    let y = get_pointer_y();

    (x, y)
}
