//! Native application icon and menu bar status items through the Objective-C
//! runtime. Every call must happen on the main thread, which is where GPUI
//! runs application code.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::OnceLock;

type Id = *mut c_void;
type Sel = *const c_void;
type Class = *mut c_void;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CGSize {
    pub width: f64,
    pub height: f64,
}

/// A rectangle in Cocoa screen coordinates: origin at the bottom-left of the
/// primary display, y pointing up.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    static NSFontAttributeName: Id;
    static NSForegroundColorAttributeName: Id;
}

#[link(name = "objc")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> Class;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
    fn objc_allocateClassPair(superclass: Class, name: *const c_char, extra_bytes: usize) -> Class;
    fn objc_registerClassPair(class: Class);
    fn class_addMethod(class: Class, name: Sel, imp: *const c_void, types: *const c_char) -> bool;
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(pool: *mut c_void);
}

/// `NSViewWidthSizable | NSViewHeightSizable`.
const RESIZE_WITH_BUTTON: u64 = 2 | 16;
/// Space at each end of an item.
const PADDING: f64 = 4.;
const COLUMN_GAP: f64 = 4.;
/// Space between stacked lines, from the baseline of one to the cap height of the next.
const LINE_GAP: f64 = 2.5;

fn class(name: &CStr) -> Class {
    unsafe { objc_getClass(name.as_ptr()) }
}

fn sel(name: &CStr) -> Sel {
    unsafe { sel_registerName(name.as_ptr()) }
}

/// `objc_msgSend` must be called through a pointer of the exact method type on arm64.
macro_rules! msg_send {
    ($receiver:expr, $sel:expr $(, $arg:expr => $ty:ty)* ; -> $ret:ty) => {{
        let send: unsafe extern "C" fn(Id, Sel $(, $ty)*) -> $ret =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        send($receiver, sel($sel) $(, $arg)*)
    }};
}

/// Sets the Dock and app switcher icon, including when run with `cargo run`.
/// The ICNS asset contains both standard and Retina representations.
pub fn set_application_icon() {
    const ICON: &[u8] = include_bytes!("../assets/AppIcon.icns");
    unsafe {
        let data: Id = msg_send!(
            class(c"NSData"),
            c"dataWithBytes:length:",
            ICON.as_ptr() => *const u8,
            ICON.len() => usize;
            -> Id
        );
        let image: Id = msg_send!(class(c"NSImage"), c"alloc"; -> Id);
        let image: Id = msg_send!(image, c"initWithData:", data => Id; -> Id);
        if image.is_null() {
            eprintln!("could not load the bundled application icon");
            return;
        }
        let app: Id = msg_send!(class(c"NSApplication"), c"sharedApplication"; -> Id);
        let _: () = msg_send!(app, c"setApplicationIconImage:", image => Id; -> ());
        let _: () = msg_send!(image, c"release"; -> ());
    }
}

/// A text style sized for the menu bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Font {
    /// The small label above a value.
    Caption,
    /// A value alone, or under a caption.
    Value,
    /// One of two values stacked in a column.
    Stacked,
}

impl Font {
    /// Point size and `NSFontWeight`.
    fn size_and_weight(self) -> (f64, f64) {
        match self {
            Self::Caption => (8.5, 0.3),
            Self::Value => (13., 0.),
            Self::Stacked => (10., 0.),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tint {
    Primary,
    Muted,
    Upload,
    Download,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub text: String,
    pub font: Font,
    pub tint: Tint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Center,
    Trailing,
}

/// Lines stacked top to bottom, centred vertically in the menu bar.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub lines: Vec<Line>,
    /// The widest text the column shows. It fixes the column's width, so the
    /// item keeps its size as values change.
    pub widest: &'static str,
    pub align: Align,
}

struct Content {
    columns: Vec<Column>,
    widths: Vec<f64>,
}

/// Receives the clicked item's tag.
type ClickHandler = Box<dyn Fn(isize)>;

thread_local! {
    static ON_CLICK: RefCell<Option<ClickHandler>> = const { RefCell::new(None) };
    /// What each status item view draws, by view address.
    static CONTENTS: RefCell<HashMap<usize, Content>> = RefCell::new(HashMap::new());
}

/// Sets what a click on any status item does; it receives the item's tag.
/// It runs on the main thread outside any GPUI update.
pub fn on_click(handler: impl Fn(isize) + 'static) {
    ON_CLICK.with(|on_click| *on_click.borrow_mut() = Some(Box::new(handler)));
}

extern "C" fn clicked(_this: Id, _cmd: Sel, sender: Id) {
    let tag: isize = unsafe { msg_send!(sender, c"tag"; -> isize) };
    ON_CLICK.with(|handler| {
        if let Some(handler) = handler.borrow().as_ref() {
            handler(tag);
        }
    });
}

extern "C" fn draw_rect(this: Id, _cmd: Sel, _dirty: CGRect) {
    CONTENTS.with(|contents| {
        if let Some(content) = contents.borrow().get(&(this as usize)) {
            let bounds: CGRect = unsafe { msg_send!(this, c"bounds"; -> CGRect) };
            autoreleased(|| draw(content, bounds.size.height));
        }
    });
}

extern "C" fn is_flipped(_this: Id, _cmd: Sel) -> bool {
    true
}

/// Lets clicks through to the button underneath.
extern "C" fn hit_test(_this: Id, _cmd: Sel, _point: CGPoint) -> Id {
    std::ptr::null_mut()
}

/// The button target: an `NSObject` subclass whose `clicked:` calls `ON_CLICK`.
fn target_class() -> Class {
    static CLASS: OnceLock<usize> = OnceLock::new();
    *CLASS.get_or_init(|| unsafe {
        let class = objc_allocateClassPair(class(c"NSObject"), c"TopStatusItemTarget".as_ptr(), 0);
        class_addMethod(
            class,
            sel(c"clicked:"),
            clicked as *const c_void,
            c"v@:@".as_ptr(),
        );
        objc_registerClassPair(class);
        class as usize
    }) as Class
}

/// A flipped `NSView` that draws its entry in `CONTENTS`.
fn view_class() -> Class {
    static CLASS: OnceLock<usize> = OnceLock::new();
    *CLASS.get_or_init(|| unsafe {
        let class = objc_allocateClassPair(class(c"NSView"), c"TopStatusItemView".as_ptr(), 0);
        class_addMethod(
            class,
            sel(c"drawRect:"),
            draw_rect as *const c_void,
            c"v@:{CGRect={CGPoint=dd}{CGSize=dd}}".as_ptr(),
        );
        class_addMethod(
            class,
            sel(c"isFlipped"),
            is_flipped as *const c_void,
            c"B@:".as_ptr(),
        );
        class_addMethod(
            class,
            sel(c"hitTest:"),
            hit_test as *const c_void,
            c"@@:{CGPoint=dd}".as_ptr(),
        );
        objc_registerClassPair(class);
        class as usize
    }) as Class
}

/// Runs `f` in an autorelease pool, which drains the objects that AppKit's
/// convenience constructors return.
fn autoreleased<R>(f: impl FnOnce() -> R) -> R {
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let result = f();
        objc_autoreleasePoolPop(pool);
        result
    }
}

/// A retained `NSString`; release it after use.
fn ns_string(text: &str) -> Id {
    let text = CString::new(text).unwrap_or_default();
    unsafe {
        let string: Id = msg_send!(class(c"NSString"), c"alloc"; -> Id);
        msg_send!(string, c"initWithUTF8String:", text.as_ptr() => *const c_char; -> Id)
    }
}

fn release(object: Id) {
    unsafe {
        let _: () = msg_send!(object, c"release"; -> ());
    }
}

fn ns_font(font: Font) -> Id {
    let (size, weight) = font.size_and_weight();
    unsafe {
        msg_send!(
            class(c"NSFont"),
            c"monospacedDigitSystemFontOfSize:weight:",
            size => f64,
            weight => f64;
            -> Id
        )
    }
}

fn font_metric(font: Font, metric: &CStr) -> f64 {
    unsafe { msg_send!(ns_font(font), metric; -> f64) }
}

fn ns_color(tint: Tint) -> Id {
    let name = match tint {
        Tint::Primary => c"labelColor",
        Tint::Muted => c"secondaryLabelColor",
        Tint::Upload => c"systemRedColor",
        Tint::Download => c"systemBlueColor",
    };
    unsafe { msg_send!(class(c"NSColor"), name; -> Id) }
}

/// A retained `NSAttributedString`; release it after use.
fn attributed(text: &str, font: Font, tint: Tint) -> Id {
    unsafe {
        let keys = [NSFontAttributeName, NSForegroundColorAttributeName];
        let values = [ns_font(font), ns_color(tint)];
        let attributes: Id = msg_send!(
            class(c"NSDictionary"),
            c"dictionaryWithObjects:forKeys:count:",
            values.as_ptr() => *const Id,
            keys.as_ptr() => *const Id,
            keys.len() => usize;
            -> Id
        );
        let text = ns_string(text);
        let string: Id = msg_send!(class(c"NSAttributedString"), c"alloc"; -> Id);
        let string: Id = msg_send!(
            string,
            c"initWithString:attributes:",
            text => Id,
            attributes => Id;
            -> Id
        );
        release(text);
        string
    }
}

fn text_size(string: Id) -> CGSize {
    unsafe { msg_send!(string, c"size"; -> CGSize) }
}

fn text_width(text: &str, font: Font) -> f64 {
    let string = attributed(text, font, Tint::Primary);
    let width = text_size(string).width.ceil();
    release(string);
    width
}

fn column_width(column: &Column) -> f64 {
    column
        .lines
        .iter()
        .flat_map(|line| {
            [
                text_width(column.widest, line.font),
                text_width(&line.text, line.font),
            ]
        })
        .fold(0., f64::max)
}

fn draw(content: &Content, height: f64) {
    let mut x = PADDING;
    for (column, &width) in content.columns.iter().zip(&content.widths) {
        let caps: Vec<f64> = column
            .lines
            .iter()
            .map(|line| font_metric(line.font, c"capHeight"))
            .collect();
        let total = caps.iter().sum::<f64>() + LINE_GAP * caps.len().saturating_sub(1) as f64;
        let mut cap_top = ((height - total) / 2.).round();
        for (line, cap) in column.lines.iter().zip(caps) {
            let string = attributed(&line.text, line.font, line.tint);
            let size = text_size(string);
            let dx = match column.align {
                Align::Center => (width - size.width) / 2.,
                Align::Trailing => width - size.width,
            };
            // In a flipped view the string's top-left corner goes at the
            // point, with its baseline one ascender below.
            let point = CGPoint {
                x: x + dx,
                y: cap_top - (font_metric(line.font, c"ascender") - cap),
            };
            unsafe {
                let _: () = msg_send!(string, c"drawAtPoint:", point => CGPoint; -> ());
            }
            release(string);
            cap_top += cap + LINE_GAP;
        }
        x += width + COLUMN_GAP;
    }
}

pub struct StatusItem {
    item: Id,
    button: Id,
    view: Id,
    target: Id,
    length: Cell<f64>,
}

impl StatusItem {
    /// Adds an item left of the app's existing ones. A click reports `tag`
    /// to the handler set with [`on_click`].
    pub fn new(tag: isize) -> Option<Self> {
        unsafe {
            let bar: Id = msg_send!(class(c"NSStatusBar"), c"systemStatusBar"; -> Id);
            let length = PADDING * 2.;
            let item: Id = msg_send!(bar, c"statusItemWithLength:", length => f64; -> Id);
            if item.is_null() {
                return None;
            }
            // The status bar does not keep the item alive.
            let item: Id = msg_send!(item, c"retain"; -> Id);
            let button: Id = msg_send!(item, c"button"; -> Id);
            let _: () = msg_send!(button, c"setTag:", tag => isize; -> ());
            let target: Id = msg_send!(target_class(), c"new"; -> Id);
            let _: () = msg_send!(button, c"setTarget:", target => Id; -> ());
            let _: () = msg_send!(button, c"setAction:", sel(c"clicked:") => Sel; -> ());
            let bounds: CGRect = msg_send!(button, c"bounds"; -> CGRect);
            let view: Id = msg_send!(view_class(), c"alloc"; -> Id);
            let view: Id = msg_send!(view, c"initWithFrame:", bounds => CGRect; -> Id);
            let _: () = msg_send!(view, c"setAutoresizingMask:", RESIZE_WITH_BUTTON => u64; -> ());
            let _: () = msg_send!(button, c"addSubview:", view => Id; -> ());
            release(view);
            Some(Self {
                item,
                button,
                view,
                target,
                length: Cell::new(length),
            })
        }
    }

    /// Shows `columns`, resizing the item to fit them. VoiceOver reads `label`.
    pub fn set_content(&self, columns: Vec<Column>, label: &str) {
        let unchanged = CONTENTS.with(|contents| {
            contents
                .borrow()
                .get(&(self.view as usize))
                .is_some_and(|content| content.columns == columns)
        });
        if unchanged {
            return;
        }
        autoreleased(|| {
            let widths: Vec<f64> = columns.iter().map(column_width).collect();
            let length = PADDING * 2.
                + widths.iter().sum::<f64>()
                + COLUMN_GAP * widths.len().saturating_sub(1) as f64;
            CONTENTS.with(|contents| {
                contents
                    .borrow_mut()
                    .insert(self.view as usize, Content { columns, widths })
            });
            unsafe {
                if length != self.length.replace(length) {
                    let _: () = msg_send!(self.item, c"setLength:", length => f64; -> ());
                }
                let label = ns_string(label);
                let _: () = msg_send!(self.button, c"setAccessibilityLabel:", label => Id; -> ());
                release(label);
                let _: () = msg_send!(self.view, c"setNeedsDisplay:", true => bool; -> ());
            }
        });
    }

    /// The item's frame on screen, in Cocoa coordinates.
    pub fn frame(&self) -> CGRect {
        unsafe {
            let window: Id = msg_send!(self.button, c"window"; -> Id);
            if window.is_null() {
                return CGRect::default();
            }
            msg_send!(window, c"frame"; -> CGRect)
        }
    }

    /// Writes what the item shows to a PNG, without screen-recording access.
    #[cfg(feature = "screenshot")]
    pub fn save_png(&self, path: &std::path::Path) -> bool {
        /// `NSBitmapImageFileTypePNG`.
        const PNG: usize = 4;
        autoreleased(|| unsafe {
            let bounds: CGRect = msg_send!(self.view, c"bounds"; -> CGRect);
            let rep: Id = msg_send!(
                self.view,
                c"bitmapImageRepForCachingDisplayInRect:",
                bounds => CGRect;
                -> Id
            );
            if rep.is_null() {
                return false;
            }
            let _: () = msg_send!(
                self.view,
                c"cacheDisplayInRect:toBitmapImageRep:",
                bounds => CGRect,
                rep => Id;
                -> ()
            );
            let properties: Id = msg_send!(class(c"NSDictionary"), c"dictionary"; -> Id);
            let data: Id = msg_send!(
                rep,
                c"representationUsingType:properties:",
                PNG => usize,
                properties => Id;
                -> Id
            );
            let path = ns_string(&path.to_string_lossy());
            let saved: bool =
                msg_send!(data, c"writeToFile:atomically:", path => Id, true => bool; -> bool);
            release(path);
            saved
        })
    }
}

impl Drop for StatusItem {
    fn drop(&mut self) {
        CONTENTS.with(|contents| contents.borrow_mut().remove(&(self.view as usize)));
        unsafe {
            let bar: Id = msg_send!(class(c"NSStatusBar"), c"systemStatusBar"; -> Id);
            let _: () = msg_send!(bar, c"removeStatusItem:", self.item => Id; -> ());
        }
        release(self.item);
        release(self.target);
    }
}
