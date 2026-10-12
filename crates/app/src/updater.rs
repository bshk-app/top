//! Sparkle's standard updater UI, loaded from the packaged application's framework.
//! All Cocoa and Sparkle calls happen on GPUI's main thread.

use std::cell::Cell;
use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr::null_mut;

type Id = *mut c_void;
type Sel = *const c_void;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Channel {
    #[default]
    Stable,
    Beta,
}

impl Channel {
    fn parse(value: &str) -> Self {
        if value.trim() == "beta" {
            Self::Beta
        } else {
            Self::Stable
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
        }
    }
}

thread_local! {
    static CONTROLLER: Cell<Id> = const { Cell::new(null_mut()) };
    static CHANNEL: Cell<Channel> = const { Cell::new(Channel::Stable) };
}

#[link(name = "objc")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
    fn objc_allocateClassPair(superclass: Id, name: *const c_char, extra: usize) -> Id;
    fn objc_registerClassPair(class: Id);
    fn objc_getProtocol(name: *const c_char) -> Id;
    fn class_addProtocol(class: Id, protocol: Id) -> bool;
    fn class_addMethod(
        class: Id,
        selector: Sel,
        implementation: *const c_void,
        types: *const c_char,
    ) -> bool;
}

unsafe extern "C" {
    fn dlopen(path: *const c_char, mode: i32) -> *mut c_void;
}

fn class(name: &CStr) -> Id {
    unsafe { objc_getClass(name.as_ptr()) }
}

macro_rules! send {
    ($object:expr, $selector:expr $(, $arg:expr => $ty:ty)* ; $ret:ty) => {{
        let method: unsafe extern "C" fn(Id, Sel $(, $ty)*) -> $ret =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        method($object, sel_registerName($selector.as_ptr()) $(, $arg)*)
    }};
}

/// Initialize only when running from a bundle containing Sparkle.
/// Retained Cocoa objects and the loaded framework live until application exit.
pub fn init() -> bool {
    if !CONTROLLER.get().is_null() {
        return true;
    }
    let saved = crate::settings::path("update-channel");
    CHANNEL.set(Channel::parse(&crate::settings::read(saved.as_deref())));
    unsafe {
        let bundle = send!(class(c"NSBundle"), c"mainBundle"; Id);
        let frameworks = send!(bundle, c"privateFrameworksPath"; Id);
        if frameworks.is_null() {
            return false;
        }
        let path = send!(frameworks, c"UTF8String"; *const c_char);
        let path = CStr::from_ptr(path).to_string_lossy();
        let Ok(path) = CString::new(format!("{path}/Sparkle.framework/Sparkle")) else {
            return false;
        };
        if dlopen(path.as_ptr(), 1).is_null() {
            return false;
        }
        let controller_class = class(c"SPUStandardUpdaterController");
        if controller_class.is_null() {
            return false;
        }
        let delegate_class =
            objc_allocateClassPair(class(c"NSObject"), c"TopUpdaterDelegate".as_ptr(), 0);
        if delegate_class.is_null() {
            return false;
        }
        let protocol = objc_getProtocol(c"SPUUpdaterDelegate".as_ptr());
        if protocol.is_null() || !class_addProtocol(delegate_class, protocol) {
            return false;
        }
        if !class_addMethod(
            delegate_class,
            sel_registerName(c"allowedChannelsForUpdater:".as_ptr()),
            allowed_channels as *const c_void,
            c"@@:@".as_ptr(),
        ) {
            return false;
        }
        objc_registerClassPair(delegate_class);
        let delegate = send!(delegate_class, c"new"; Id);
        let allocated = send!(controller_class, c"alloc"; Id);
        let controller = send!(
            allocated, c"initWithStartingUpdater:updaterDelegate:userDriverDelegate:",
            false => bool, delegate => Id, null_mut() => Id; Id
        );
        if controller.is_null() {
            return false;
        }
        CONTROLLER.set(controller);
        let _: () = send!(controller, c"startUpdater"; ());
        true
    }
}

/// Default Sparkle items are stable; beta subscribers also accept beta items.
unsafe extern "C" fn allowed_channels(_: Id, _: Sel, _: Id) -> Id {
    unsafe {
        if channel() == Channel::Beta {
            let beta = send!(class(c"NSString"), c"stringWithUTF8String:", c"beta".as_ptr() => *const c_char; Id);
            send!(class(c"NSSet"), c"setWithObject:", beta => Id; Id)
        } else {
            send!(class(c"NSSet"), c"set"; Id)
        }
    }
}

pub fn channel() -> Channel {
    CHANNEL.get()
}

pub fn set_channel(channel: Channel) {
    CHANNEL.set(channel);
    if let Some(path) = crate::settings::path("update-channel") {
        crate::settings::write(&path, channel.name());
    }
    unsafe {
        let controller = CONTROLLER.get();
        if !controller.is_null() {
            let updater = send!(controller, c"updater"; Id);
            let _: () = send!(updater, c"resetUpdateCycleAfterShortDelay"; ());
        }
    }
}

pub fn check_for_updates() {
    unsafe {
        let controller = CONTROLLER.get();
        if !controller.is_null() {
            let updater = send!(controller, c"updater"; Id);
            if send!(updater, c"canCheckForUpdates"; bool) {
                let _: () = send!(controller, c"checkForUpdates:", null_mut() => Id; ());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_or_missing_channel_defaults_to_stable() {
        assert_eq!(Channel::parse(""), Channel::Stable);
        assert_eq!(Channel::parse("invalid"), Channel::Stable);
        assert_eq!(Channel::parse("beta\n"), Channel::Beta);
        for channel in [Channel::Stable, Channel::Beta] {
            assert_eq!(Channel::parse(channel.name()), channel);
        }
    }

    #[gpui_kit::test]
    fn only_beta_subscribers_allow_beta_items() {
        unsafe {
            CHANNEL.set(Channel::Stable);
            let stable = allowed_channels(null_mut(), std::ptr::null(), null_mut());
            assert_eq!(send!(stable, c"count"; usize), 0);
            CHANNEL.set(Channel::Beta);
            let beta = allowed_channels(null_mut(), std::ptr::null(), null_mut());
            let name = send!(class(c"NSString"), c"stringWithUTF8String:", c"beta".as_ptr() => *const c_char; Id);
            assert!(send!(beta, c"containsObject:", name => Id; bool));
            assert_eq!(send!(beta, c"count"; usize), 1);
            CHANNEL.set(Channel::Stable);
        }
    }
}
