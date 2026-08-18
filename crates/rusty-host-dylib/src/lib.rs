
use std::ffi::{CStr, CString};
use std::fmt;
use std::os::raw::{c_char, c_int, c_void};
use std::path::Path;
use std::ptr::NonNull;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DylibError {
    UnsupportedPlatform,
    InvalidPath,
    InvalidSymbolName,
    Open(String),
    Symbol(String),
    Close(String),
}

impl fmt::Display for DylibError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DylibError::UnsupportedPlatform => write!(f, "dynamic loading is unsupported"),
            DylibError::InvalidPath => write!(f, "dynamic library path contains an interior NUL"),
            DylibError::InvalidSymbolName => write!(f, "symbol name contains an interior NUL"),
            DylibError::Open(msg) => write!(f, "{msg}"),
            DylibError::Symbol(msg) => write!(f, "{msg}"),
            DylibError::Close(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for DylibError {}

#[cfg(unix)]
mod imp {
    use super::*;

    const RTLD_NOW: c_int = 2;
    #[cfg(target_os = "macos")]
    const RTLD_LOCAL: c_int = 0x4;
    #[cfg(not(target_os = "macos"))]
    const RTLD_LOCAL: c_int = 0;

    #[cfg_attr(target_os = "linux", link(name = "dl"))]
    extern "C" {
        fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlclose(handle: *mut c_void) -> c_int;
        fn dlerror() -> *const c_char;
    }

    #[derive(Debug)]
    pub struct Library {
        handle: NonNull<c_void>,
    }

    impl Library {
        pub unsafe fn open<P: AsRef<Path>>(path: P) -> Result<Self, DylibError> {
            let path = path.as_ref().as_os_str().to_string_lossy();
            let c_path = CString::new(path.as_bytes()).map_err(|_| DylibError::InvalidPath)?;
            clear_error();
            let raw = dlopen(c_path.as_ptr(), RTLD_NOW | RTLD_LOCAL);
            let handle = NonNull::new(raw).ok_or_else(|| {
                DylibError::Open(take_error().unwrap_or_else(|| "dlopen failed".into()))
            })?;
            Ok(Self { handle })
        }

        pub unsafe fn symbol(&self, name: &[u8]) -> Result<*mut c_void, DylibError> {
            let c_name = CString::new(name).map_err(|_| DylibError::InvalidSymbolName)?;
            clear_error();
            let ptr = dlsym(self.handle.as_ptr(), c_name.as_ptr());
            if let Some(err) = take_error() {
                return Err(DylibError::Symbol(err));
            }
            Ok(ptr)
        }

        pub unsafe fn close(mut self) -> Result<(), DylibError> {
            let handle = self.handle.as_ptr();
            self.handle = NonNull::dangling();
            std::mem::forget(self);
            close_handle(handle)
        }
    }

    impl Drop for Library {
        fn drop(&mut self) {
            let _ = unsafe { close_handle(self.handle.as_ptr()) };
        }
    }

    unsafe fn close_handle(handle: *mut c_void) -> Result<(), DylibError> {
        clear_error();
        if dlclose(handle) == 0 {
            Ok(())
        } else {
            Err(DylibError::Close(
                take_error().unwrap_or_else(|| "dlclose failed".into()),
            ))
        }
    }

    unsafe fn clear_error() {
        let _ = dlerror();
    }

    unsafe fn take_error() -> Option<String> {
        let ptr = dlerror();
        if ptr.is_null() {
            None
        } else {
            Some(CStr::from_ptr(ptr).to_string_lossy().into_owned())
        }
    }
}

#[cfg(unix)]
pub use imp::Library;

#[cfg(not(unix))]
pub struct Library;

#[cfg(not(unix))]
impl Library {
    pub unsafe fn open<P: AsRef<Path>>(_path: P) -> Result<Self, DylibError> {
        Err(DylibError::UnsupportedPlatform)
    }

    pub unsafe fn symbol(&self, _name: &[u8]) -> Result<*mut c_void, DylibError> {
        Err(DylibError::UnsupportedPlatform)
    }

    pub unsafe fn close(self) -> Result<(), DylibError> {
        Err(DylibError::UnsupportedPlatform)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn missing_library_errors() {
        let err = unsafe { Library::open("/definitely/missing/libcruft_fixture.so") }.unwrap_err();
        assert!(matches!(err, DylibError::Open(_)));
    }

    #[test]
    fn fixture_open_symbol_and_close() {
        let Some(path) = build_fixture() else {
            eprintln!("cc unavailable; skipping dynamic-library fixture test");
            return;
        };
        let lib = unsafe { Library::open(&path) }.unwrap();
        let missing = unsafe { lib.symbol(b"not_the_symbol") }.unwrap_err();
        assert!(matches!(missing, DylibError::Symbol(_)));

        let sym = unsafe { lib.symbol(b"rusty_host_dylib_fixture_answer") }.unwrap();
        assert!(!sym.is_null());
        let f: extern "C" fn() -> i32 = unsafe { std::mem::transmute(sym) };
        assert_eq!(f(), 42);
        unsafe { lib.close() }.unwrap();
    }

    fn build_fixture() -> Option<std::path::PathBuf> {
        let mut dir = std::env::temp_dir();
        dir.push(format!("rusty-host-dylib-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).ok()?;
        let src = dir.join("fixture.c");
        std::fs::write(
            &src,
            b"int rusty_host_dylib_fixture_answer(void) { return 42; }\n",
        )
        .ok()?;
        let out = dir.join(if cfg!(target_os = "macos") {
            "fixture.dylib"
        } else {
            "fixture.so"
        });
        let mut cmd = Command::new("cc");
        if cfg!(target_os = "macos") {
            cmd.arg("-dynamiclib").arg(&src).arg("-o").arg(&out);
        } else {
            cmd.arg("-shared")
                .arg("-fPIC")
                .arg(&src)
                .arg("-o")
                .arg(&out);
        }
        let status = cmd.status().ok()?;
        status.success().then_some(out)
    }
}
