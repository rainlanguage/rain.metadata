use wasm_bindgen::prelude::*;
use rain_metadata::meta::{
                             RainMetaDocumentV1Item                         };
use serde_wasm_bindgen::to_value;

#[wasm_bindgen]
pub fn cbor_decode_wasm(data: &[u8]) -> JsValue {
    // Decode the data
    let cbor_decoded = RainMetaDocumentV1Item::cbor_decode(&data);

    // Convert the result to JsValue
    match cbor_decoded {
        Ok(decoded) => {
            let js_value = to_value(&decoded);
            match js_value {
                Ok(value) => value,
                Err(_) => JsValue::from_str("Error converting to JsValue"),
            }
        }
        Err(err) => JsValue::from_str(&format!("CBOR decoding error: {:?}", err)),
    }
}
