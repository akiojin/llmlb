use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

use minijinja::{context, Environment, ErrorKind, UndefinedBehavior};
use serde_json::Value as JsonValue;
use tokenizers::Tokenizer;

#[repr(C)]
pub struct LlmTokenizerHandle {
    tokenizer: Tokenizer,
}

fn set_error(out_error: *mut *mut c_char, msg: impl AsRef<str>) {
    if out_error.is_null() {
        return;
    }
    let cstr = match CString::new(msg.as_ref()) {
        Ok(s) => s,
        Err(_) => CString::new("error").unwrap(),
    };
    unsafe {
        *out_error = cstr.into_raw();
    }
}

#[no_mangle]
pub extern "C" fn llm_tokenizer_free_string(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(s));
    }
}

#[no_mangle]
pub extern "C" fn llm_tokenizer_load(
    tokenizer_json_path: *const c_char,
    out_error: *mut *mut c_char,
) -> *mut LlmTokenizerHandle {
    if tokenizer_json_path.is_null() {
        set_error(out_error, "tokenizer_json_path is null");
        return ptr::null_mut();
    }

    let path = unsafe { CStr::from_ptr(tokenizer_json_path) }
        .to_string_lossy()
        .to_string();

    match Tokenizer::from_file(&path) {
        Ok(tokenizer) => Box::into_raw(Box::new(LlmTokenizerHandle { tokenizer })),
        Err(e) => {
            set_error(out_error, format!("Failed to load tokenizer.json: {e}"));
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn llm_tokenizer_free(handle: *mut LlmTokenizerHandle) {
    if handle.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle));
    }
}

#[no_mangle]
pub extern "C" fn llm_tokenizer_free_i64_array(ptr: *mut i64, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    unsafe {
        drop(Vec::from_raw_parts(ptr, len, len));
    }
}

#[no_mangle]
pub extern "C" fn llm_tokenizer_encode(
    handle: *const LlmTokenizerHandle,
    text: *const c_char,
    add_special_tokens: bool,
    out_ids_ptr: *mut *mut i64,
    out_ids_len: *mut usize,
    out_error: *mut *mut c_char,
) -> bool {
    if handle.is_null() {
        set_error(out_error, "handle is null");
        return false;
    }
    if text.is_null() {
        set_error(out_error, "text is null");
        return false;
    }
    if out_ids_ptr.is_null() || out_ids_len.is_null() {
        set_error(out_error, "out_ids is null");
        return false;
    }

    let handle = unsafe { &*handle };
    let text = unsafe { CStr::from_ptr(text) }.to_string_lossy();

    let enc = match handle.tokenizer.encode(text.as_ref(), add_special_tokens) {
        Ok(e) => e,
        Err(e) => {
            set_error(out_error, format!("encode failed: {e}"));
            return false;
        }
    };

    let ids: Vec<i64> = enc.get_ids().iter().map(|&v| v as i64).collect();
    let len = ids.len();
    let mut ids = ids;
    let ptr = ids.as_mut_ptr();
    std::mem::forget(ids);

    unsafe {
        *out_ids_ptr = ptr;
        *out_ids_len = len;
    }
    true
}

#[no_mangle]
pub extern "C" fn llm_tokenizer_decode(
    handle: *const LlmTokenizerHandle,
    ids_ptr: *const i64,
    ids_len: usize,
    skip_special_tokens: bool,
    out_text: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> bool {
    if handle.is_null() {
        set_error(out_error, "handle is null");
        return false;
    }
    if out_text.is_null() {
        set_error(out_error, "out_text is null");
        return false;
    }
    if ids_ptr.is_null() && ids_len != 0 {
        set_error(out_error, "ids_ptr is null");
        return false;
    }

    let handle = unsafe { &*handle };
    let ids: &[i64] = unsafe { std::slice::from_raw_parts(ids_ptr, ids_len) };
    let ids_u32: Vec<u32> = ids
        .iter()
        .map(|&v| u32::try_from(v).unwrap_or(0))
        .collect();

    match handle.tokenizer.decode(&ids_u32, skip_special_tokens) {
        Ok(text) => {
            let cstr = match CString::new(text) {
                Ok(s) => s,
                Err(_) => {
                    set_error(out_error, "decode produced interior null byte");
                    return false;
                }
            };
            unsafe {
                *out_text = cstr.into_raw();
            }
            true
        }
        Err(e) => {
            set_error(out_error, format!("decode failed: {e}"));
            false
        }
    }
}

#[no_mangle]
pub extern "C" fn llm_tokenizer_token_to_id(
    handle: *const LlmTokenizerHandle,
    token: *const c_char,
    out_error: *mut *mut c_char,
) -> i64 {
    if handle.is_null() {
        set_error(out_error, "handle is null");
        return -1;
    }
    if token.is_null() {
        set_error(out_error, "token is null");
        return -1;
    }
    let handle = unsafe { &*handle };
    let token = unsafe { CStr::from_ptr(token) }.to_string_lossy();
    match handle.tokenizer.token_to_id(token.as_ref()) {
        Some(id) => id as i64,
        None => -1,
    }
}

#[no_mangle]
pub extern "C" fn llm_chat_template_render(
    template_str: *const c_char,
    messages_json: *const c_char,
    special_tokens_json: *const c_char,
    add_generation_prompt: bool,
    out_text: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> bool {
    if template_str.is_null() {
        set_error(out_error, "template_str is null");
        return false;
    }
    if messages_json.is_null() {
        set_error(out_error, "messages_json is null");
        return false;
    }
    if out_text.is_null() {
        set_error(out_error, "out_text is null");
        return false;
    }

    let template_str = unsafe { CStr::from_ptr(template_str) }.to_string_lossy();
    let messages_json = unsafe { CStr::from_ptr(messages_json) }.to_string_lossy();
    let special_tokens_json = if special_tokens_json.is_null() {
        std::borrow::Cow::Borrowed("")
    } else {
        unsafe { CStr::from_ptr(special_tokens_json) }.to_string_lossy()
    };

    let messages: JsonValue = match serde_json::from_str(messages_json.as_ref()) {
        Ok(v) => v,
        Err(e) => {
            set_error(out_error, format!("messages_json parse failed: {e}"));
            return false;
        }
    };
    if !messages.is_array() {
        set_error(out_error, "messages_json must be a JSON array");
        return false;
    }

    let special_tokens: JsonValue = if special_tokens_json.trim().is_empty() {
        JsonValue::Object(serde_json::Map::new())
    } else {
        match serde_json::from_str(special_tokens_json.as_ref()) {
            Ok(v) => v,
            Err(e) => {
                set_error(out_error, format!("special_tokens_json parse failed: {e}"));
                return false;
            }
        }
    };
    if !special_tokens.is_object() {
        set_error(out_error, "special_tokens_json must be a JSON object");
        return false;
    }

    // Jinja2 default behavior is lenient for undefined variables; match that.
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Lenient);
    env.add_function("raise_exception", |msg: String| -> Result<String, minijinja::Error> {
        Err(minijinja::Error::new(ErrorKind::InvalidOperation, msg))
    });

    if let Err(e) = env.add_template("chat_template", template_str.as_ref()) {
        set_error(out_error, format!("template parse failed: {e}"));
        return false;
    }

    let template = match env.get_template("chat_template") {
        Ok(t) => t,
        Err(e) => {
            set_error(out_error, format!("template lookup failed: {e}"));
            return false;
        }
    };

    // Provide common variables used in HF chat templates.
    let get_tok = |key: &str| -> String {
        special_tokens
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    let ctx = context! {
        messages => messages,
        add_generation_prompt => add_generation_prompt,
        bos_token => get_tok("bos_token"),
        eos_token => get_tok("eos_token"),
        unk_token => get_tok("unk_token"),
        pad_token => get_tok("pad_token"),
        sep_token => get_tok("sep_token"),
        cls_token => get_tok("cls_token"),
        mask_token => get_tok("mask_token"),
        tokenizer => special_tokens,
        tools => Vec::<JsonValue>::new(),
    };

    match template.render(ctx) {
        Ok(text) => {
            let cstr = match CString::new(text) {
                Ok(s) => s,
                Err(_) => {
                    set_error(out_error, "render produced interior null byte");
                    return false;
                }
            };
            unsafe {
                *out_text = cstr.into_raw();
            }
            true
        }
        Err(e) => {
            set_error(out_error, format!("render failed: {e}"));
            false
        }
    }
}
