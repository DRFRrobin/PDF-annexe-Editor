//! Liaison WebAssembly : `stamp_pdf(bytes, options_json) -> Uint8Array`.
use crate::{stamp, Options};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

#[derive(Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct JsOptions {
    annexe: String,
    template: String,
    start: u32,
    total: Option<u32>,
    font_size: f32,
    pages: String,
    white_fill: bool,
    fx: f32,
    fy: f32,
}

impl Default for JsOptions {
    fn default() -> Self {
        let d = Options::default();
        JsOptions { annexe: d.annexe, template: d.template, start: d.start, total: d.total, font_size: d.font_size,
            pages: d.pages, white_fill: d.white_fill, fx: d.fx, fy: d.fy }
    }
}

#[wasm_bindgen]
pub fn stamp_pdf(input: &[u8], options_json: &str) -> Result<Vec<u8>, JsError> {
    let j: JsOptions = serde_json::from_str(options_json).map_err(|e| JsError::new(&e.to_string()))?;
    let o = Options { annexe: j.annexe, template: j.template, start: j.start, total: j.total, font_size: j.font_size,
        pages: j.pages, white_fill: j.white_fill, fx: j.fx, fy: j.fy };
    stamp(input, &o).map(|(b, _)| b).map_err(|e| JsError::new(&e))
}
