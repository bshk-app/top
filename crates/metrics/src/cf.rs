//! Minimal CoreFoundation and IOKit bindings plus safe owning wrappers.
#![allow(non_camel_case_types, non_upper_case_globals, dead_code)]

use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr;

pub type CFTypeRef = *const c_void;
pub type CFAllocatorRef = *const c_void;
pub type CFStringRef = *const c_void;
pub type CFDictionaryRef = *const c_void;
pub type CFMutableDictionaryRef = *mut c_void;
pub type CFArrayRef = *const c_void;
pub type CFNumberRef = *const c_void;
pub type CFDataRef = *const c_void;
pub type CFIndex = isize;
pub type CFTypeID = usize;
pub type CFStringEncoding = u32;
pub type CFNumberType = CFIndex;

pub const kCFStringEncodingUTF8: CFStringEncoding = 0x0800_0100;
pub const kCFNumberSInt32Type: CFNumberType = 3;
pub const kCFNumberSInt64Type: CFNumberType = 4;
pub const kCFNumberDoubleType: CFNumberType = 13;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    pub static kCFAllocatorDefault: CFAllocatorRef;
    pub static kCFTypeDictionaryKeyCallBacks: c_void;
    pub static kCFTypeDictionaryValueCallBacks: c_void;
    pub static kCFBooleanTrue: CFTypeRef;

    pub fn CFRelease(cf: CFTypeRef);
    pub fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
    pub fn CFGetTypeID(cf: CFTypeRef) -> CFTypeID;
    pub fn CFStringGetTypeID() -> CFTypeID;
    pub fn CFNumberGetTypeID() -> CFTypeID;
    pub fn CFDictionaryGetTypeID() -> CFTypeID;
    pub fn CFArrayGetTypeID() -> CFTypeID;
    pub fn CFDataGetTypeID() -> CFTypeID;
    pub fn CFBooleanGetTypeID() -> CFTypeID;

    pub fn CFStringCreateWithCString(
        alloc: CFAllocatorRef,
        c_str: *const c_char,
        encoding: CFStringEncoding,
    ) -> CFStringRef;
    pub fn CFStringGetCString(
        string: CFStringRef,
        buffer: *mut c_char,
        buffer_size: CFIndex,
        encoding: CFStringEncoding,
    ) -> u8;
    pub fn CFStringGetCStringPtr(string: CFStringRef, encoding: CFStringEncoding) -> *const c_char;

    pub fn CFDictionaryGetValue(dict: CFDictionaryRef, key: CFTypeRef) -> CFTypeRef;
    pub fn CFDictionaryGetCount(dict: CFDictionaryRef) -> CFIndex;
    pub fn CFDictionaryCreateMutableCopy(
        alloc: CFAllocatorRef,
        capacity: CFIndex,
        dict: CFDictionaryRef,
    ) -> CFMutableDictionaryRef;
    pub fn CFDictionaryCreateMutable(
        alloc: CFAllocatorRef,
        capacity: CFIndex,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> CFMutableDictionaryRef;
    pub fn CFDictionarySetValue(dict: CFMutableDictionaryRef, key: CFTypeRef, value: CFTypeRef);

    pub fn CFArrayGetCount(array: CFArrayRef) -> CFIndex;
    pub fn CFArrayGetValueAtIndex(array: CFArrayRef, index: CFIndex) -> CFTypeRef;

    pub fn CFNumberGetValue(number: CFNumberRef, kind: CFNumberType, value_ptr: *mut c_void) -> u8;
    pub fn CFNumberCreate(
        alloc: CFAllocatorRef,
        kind: CFNumberType,
        value_ptr: *const c_void,
    ) -> CFNumberRef;

    pub fn CFDataGetLength(data: CFDataRef) -> CFIndex;
    pub fn CFDataGetBytePtr(data: CFDataRef) -> *const u8;
}

/// An owned CoreFoundation object, released on drop.
pub struct CFOwned(CFTypeRef);

impl CFOwned {
    /// Takes ownership of a +1 reference (from a `Create`/`Copy` call).
    pub fn new(cf: CFTypeRef) -> Option<Self> {
        // `then`, not `then_some`: an eagerly built owner would CFRelease(NULL) on drop.
        (!cf.is_null()).then(|| Self(cf))
    }

    pub fn string(text: &str) -> Self {
        let text = CString::new(text).expect("CF string without NUL");
        let cf = unsafe {
            CFStringCreateWithCString(kCFAllocatorDefault, text.as_ptr(), kCFStringEncodingUTF8)
        };
        Self(cf)
    }

    pub fn as_ptr(&self) -> CFTypeRef {
        self.0
    }
}

impl Drop for CFOwned {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) };
    }
}

/// Reads a CFString into a Rust `String`.
pub fn string(cf: CFTypeRef) -> Option<String> {
    if cf.is_null() || unsafe { CFGetTypeID(cf) != CFStringGetTypeID() } {
        return None;
    }
    unsafe {
        let direct = CFStringGetCStringPtr(cf, kCFStringEncodingUTF8);
        if !direct.is_null() {
            return Some(CStr::from_ptr(direct).to_string_lossy().into_owned());
        }
        let mut buf = [0 as c_char; 256];
        (CFStringGetCString(
            cf,
            buf.as_mut_ptr(),
            buf.len() as CFIndex,
            kCFStringEncodingUTF8,
        ) != 0)
            .then(|| CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned())
    }
}

/// Reads a CFNumber as `i64`.
pub fn number_i64(cf: CFTypeRef) -> Option<i64> {
    if cf.is_null() || unsafe { CFGetTypeID(cf) != CFNumberGetTypeID() } {
        return None;
    }
    let mut value = 0i64;
    (unsafe { CFNumberGetValue(cf, kCFNumberSInt64Type, (&mut value as *mut i64).cast()) } != 0)
        .then_some(value)
}

pub fn is_true(cf: CFTypeRef) -> bool {
    !cf.is_null() && unsafe { cf == kCFBooleanTrue }
}

/// Looks up `key` in a CFDictionary; the result is borrowed from `dict`.
pub fn dict_get(dict: CFDictionaryRef, key: &str) -> CFTypeRef {
    if dict.is_null() {
        return ptr::null();
    }
    let key = CFOwned::string(key);
    unsafe { CFDictionaryGetValue(dict, key.as_ptr()) }
}

pub fn data_bytes<'a>(cf: CFTypeRef) -> Option<&'a [u8]> {
    if cf.is_null() || unsafe { CFGetTypeID(cf) != CFDataGetTypeID() } {
        return None;
    }
    unsafe {
        let len = CFDataGetLength(cf) as usize;
        let ptr = CFDataGetBytePtr(cf);
        (!ptr.is_null()).then(|| std::slice::from_raw_parts(ptr, len))
    }
}

// IOKit
pub type io_object_t = u32;
pub type io_iterator_t = u32;
pub type io_registry_entry_t = u32;
pub type io_connect_t = u32;
pub type IOOptionBits = u32;

pub const kIOMainPortDefault: u32 = 0;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    pub fn IOServiceMatching(name: *const c_char) -> CFMutableDictionaryRef;
    pub fn IOServiceNameMatching(name: *const c_char) -> CFMutableDictionaryRef;
    pub fn IOServiceGetMatchingServices(
        main_port: u32,
        matching: CFDictionaryRef,
        existing: *mut io_iterator_t,
    ) -> i32;
    pub fn IOServiceGetMatchingService(main_port: u32, matching: CFDictionaryRef) -> io_object_t;
    pub fn IOIteratorNext(iterator: io_iterator_t) -> io_object_t;
    pub fn IOObjectRelease(object: io_object_t) -> i32;
    pub fn IORegistryEntryCreateCFProperties(
        entry: io_registry_entry_t,
        properties: *mut CFMutableDictionaryRef,
        allocator: CFAllocatorRef,
        options: IOOptionBits,
    ) -> i32;
    pub fn IORegistryEntryCreateCFProperty(
        entry: io_registry_entry_t,
        key: CFStringRef,
        allocator: CFAllocatorRef,
        options: IOOptionBits,
    ) -> CFTypeRef;
    pub fn IORegistryEntryGetName(entry: io_registry_entry_t, name: *mut c_char) -> i32;
    pub fn IORegistryEntryGetChildIterator(
        entry: io_registry_entry_t,
        plane: *const c_char,
        iterator: *mut io_iterator_t,
    ) -> i32;
    pub fn IOServiceOpen(
        service: io_object_t,
        owning_task: u32,
        kind: u32,
        connect: *mut io_connect_t,
    ) -> i32;
    pub fn IOServiceClose(connect: io_connect_t) -> i32;
    pub fn IOConnectCallStructMethod(
        connection: io_connect_t,
        selector: u32,
        input: *const c_void,
        input_size: usize,
        output: *mut c_void,
        output_size: *mut usize,
    ) -> i32;
}

/// Calls `visit` with every registry entry of class `class`. Entries are
/// released after the call; read only the properties you need with
/// [`property`], because copying a whole property table is costly.
pub fn for_each_service(class: &str, visit: impl FnMut(io_object_t)) {
    let class = CString::new(class).expect("class without NUL");
    let mut iter: io_iterator_t = 0;
    unsafe {
        // `IOServiceGetMatchingServices` consumes the matching dictionary.
        let matching = IOServiceMatching(class.as_ptr());
        if matching.is_null()
            || IOServiceGetMatchingServices(kIOMainPortDefault, matching, &mut iter) != 0
        {
            return;
        }
        visit_iterator(iter, visit);
    }
}

/// Calls `visit` with each child of `entry` in the service plane.
pub fn for_each_child(entry: io_registry_entry_t, visit: impl FnMut(io_object_t)) {
    let mut iter: io_iterator_t = 0;
    unsafe {
        if IORegistryEntryGetChildIterator(entry, c"IOService".as_ptr(), &mut iter) != 0 {
            return;
        }
        visit_iterator(iter, visit);
    }
}

/// Visits and releases every object of `iter`, then the iterator itself.
unsafe fn visit_iterator(iter: io_iterator_t, mut visit: impl FnMut(io_object_t)) {
    unsafe {
        loop {
            let object = IOIteratorNext(iter);
            if object == 0 {
                break;
            }
            visit(object);
            IOObjectRelease(object);
        }
        IOObjectRelease(iter);
    }
}

/// One property of a registry entry.
pub fn property(entry: io_registry_entry_t, key: &str) -> Option<CFOwned> {
    let key = CFOwned::string(key);
    CFOwned::new(unsafe {
        IORegistryEntryCreateCFProperty(entry, key.as_ptr(), kCFAllocatorDefault, 0)
    })
}

/// The whole property table of a registry entry.
pub fn properties(entry: io_registry_entry_t) -> Option<CFOwned> {
    let mut props: CFMutableDictionaryRef = ptr::null_mut();
    let rc =
        unsafe { IORegistryEntryCreateCFProperties(entry, &mut props, kCFAllocatorDefault, 0) };
    if rc == 0 { CFOwned::new(props) } else { None }
}

/// Iterates the items of a CFArray; yields nothing for any other type.
pub fn array_items(array: CFTypeRef) -> impl Iterator<Item = CFTypeRef> {
    let count = if !array.is_null() && unsafe { CFGetTypeID(array) == CFArrayGetTypeID() } {
        unsafe { CFArrayGetCount(array) }
    } else {
        0
    };
    (0..count).map(move |ix| unsafe { CFArrayGetValueAtIndex(array, ix) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_null_result_is_none_without_releasing_it() {
        assert!(CFOwned::new(ptr::null()).is_none());
    }
}
