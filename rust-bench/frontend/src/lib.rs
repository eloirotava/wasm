use wasm_bindgen::prelude::*;
use core_algo::heavy_computation;

#[wasm_bindgen]
pub fn run_wasm_bench(size: usize) -> String {
    // Console log opcional
    web_sys::console::log_1(&"Iniciando WASM...".into());
    heavy_computation(size)
}