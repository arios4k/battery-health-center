//! Translations through gettext. Strings are looked up in the
//! `battery-health-center` domain; without a translation (or in English) the
//! original text is shown.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_char, CString};
use std::path::{Path, PathBuf};

use gtk::glib;

const DOMAIN: &str = "battery-health-center";

extern "C" {
    fn bindtextdomain(domain: *const c_char, dir: *const c_char) -> *mut c_char;
    fn bind_textdomain_codeset(domain: *const c_char, codeset: *const c_char) -> *mut c_char;
}

/// Finds the folder holding `<lang>/LC_MESSAGES/battery-health-center.mo`:
/// `BHC_LOCALEDIR`, the `share/locale` next to the install prefix (which also
/// covers `/usr/local`), or the system one.
fn locale_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("BHC_LOCALEDIR") {
        return PathBuf::from(dir);
    }
    let from_exe = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.parent()?.join("share/locale")));
    match from_exe {
        Some(dir) if has_translations(&dir) => dir,
        _ => PathBuf::from("/usr/share/locale"),
    }
}

fn has_translations(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|langs| langs.flatten().any(|l| l.path().join("LC_MESSAGES").join(format!("{DOMAIN}.mo")).exists()))
}

/// Call once at startup, before any text is built.
pub fn init() {
    // SAFETY: plain libc calls with NUL-terminated strings that outlive them.
    unsafe {
        libc::setlocale(libc::LC_ALL, c"".as_ptr());
        let domain = CString::new(DOMAIN).unwrap();
        if let Ok(dir) = CString::new(locale_dir().to_string_lossy().as_bytes()) {
            bindtextdomain(domain.as_ptr(), dir.as_ptr());
        }
        bind_textdomain_codeset(domain.as_ptr(), c"UTF-8".as_ptr());
    }
}

/// Translates a message. The result lives for the whole run (translations are
/// looked up once and kept), so it can go wherever a `&'static str` is needed.
pub fn tr(msgid: &'static str) -> &'static str {
    thread_local! {
        static CACHE: RefCell<HashMap<&'static str, &'static str>> = RefCell::default();
    }
    CACHE.with(|cache| {
        *cache.borrow_mut().entry(msgid).or_insert_with(|| {
            let text = glib::dgettext(Some(DOMAIN), msgid);
            if text == msgid {
                msgid
            } else {
                Box::leak(text.to_string().into_boxed_str())
            }
        })
    })
}

/// Translates a message that needs a context to tell it from the same English
/// text used elsewhere (for example a page title and a status).
pub fn trc(context: &'static str, msgid: &'static str) -> &'static str {
    thread_local! {
        static CACHE: RefCell<HashMap<(&'static str, &'static str), &'static str>> = RefCell::default();
    }
    CACHE.with(|cache| {
        *cache.borrow_mut().entry((context, msgid)).or_insert_with(|| {
            let text = glib::dpgettext2(Some(DOMAIN), context, msgid);
            if text == msgid {
                msgid
            } else {
                Box::leak(text.to_string().into_boxed_str())
            }
        })
    })
}

/// Translates a message that depends on a count, for example "1 hour" / "2 hours".
pub fn trn(singular: &'static str, plural: &'static str, n: u64) -> String {
    glib::dngettext(Some(DOMAIN), singular, plural, n as _).to_string()
}

/// Translates a message and fills in its `{name}` placeholders, so translators
/// can move them around.
pub fn trf(msgid: &'static str, args: &[(&str, String)]) -> String {
    fill(tr(msgid).to_string(), args)
}

/// Like [`trn`], with placeholders.
pub fn trnf(singular: &'static str, plural: &'static str, n: u64, args: &[(&str, String)]) -> String {
    fill(trn(singular, plural, n), args)
}

fn fill(mut text: String, args: &[(&str, String)]) -> String {
    for (name, value) in args {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_placeholders_in_any_order() {
        let args = [("a", "1".to_string()), ("b", "2".to_string())];
        assert_eq!(fill("{b} then {a}".into(), &args), "2 then 1");
    }

    #[test]
    fn untranslated_text_comes_back_unchanged() {
        assert_eq!(tr("Nothing translates this"), "Nothing translates this");
        assert_eq!(trn("1 thing", "{n} things", 1), "1 thing");
        assert_eq!(trn("1 thing", "{n} things", 3), "{n} things");
    }
}
