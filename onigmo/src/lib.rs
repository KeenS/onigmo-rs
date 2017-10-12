extern crate onigmo_sys;
extern crate libc;

use onigmo_sys::*;
use std::mem;
use std::fmt;
use std::error;
use std::ops::Drop;
use std::ops::Range;
use std::sync::{Once, ONCE_INIT};

pub struct Regex(regex_t);

#[derive(Debug, Clone)]
pub struct Error(OnigPosition, Option<OnigErrorInfo>, String);
type Result<T> = ::std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Region(*mut OnigRegion);

#[derive(Debug, Clone)]
pub struct PositionIter<'a>(&'a Region, Range<i32>);
#[derive(Debug, Clone)]
pub struct StrIter<'a>(&'a Region, Range<i32>, &'a str);

fn initialize() {
    static INIT: Once = ONCE_INIT;
    INIT.call_once(|| unsafe {
        onig_init();
        assert_eq!(libc::atexit(cleanup), 0);
    });

    pub extern "C" fn cleanup() {
        unsafe {
            onig_end();
        }
    }
}

impl Regex {
    pub fn new(pattern: String) -> Result<Self> {
        initialize();
        unsafe {
            let mut reg: regex_t = mem::uninitialized();
            let pattern = pattern.as_bytes();
            let mut einfo: OnigErrorInfo = mem::uninitialized();
            let r = onig_new_without_alloc(
                &mut reg as *mut _,
                pattern.as_ptr() as *const OnigUChar,
                (pattern.as_ptr() as *const OnigUChar).offset(pattern.len() as isize),
                ONIG_OPTION_NONE,
                &OnigEncodingUTF_8,
                OnigDefaultSyntax,
                &mut einfo,
            );
            if (r as ::std::os::raw::c_uint) == ONIG_NORMAL {
                Ok(Regex(reg))
            } else {
                Err(Error::new(r as OnigPosition, Some(einfo)))
            }
        }
    }

    pub fn cleanup() {
        unsafe {
            onig_end();
        }
    }
    // TODO reverse search
    pub fn search(&mut self, s: &str) -> Option<Region> {
        unsafe {
            let s = s.as_bytes();
            let start = s.as_ptr();
            let end = start.offset(s.len() as isize);
            let range = end;
            let region = Region::new();

            let pos = onig_search(
                &mut self.0,
                start,
                end,
                start,
                range,
                region.0,
                ONIG_OPTION_NONE,
            );
            if 0 <= pos {
                Some(region)
            } else {
                debug_assert!(pos as ::std::os::raw::c_int == ONIG_MISMATCH);
                None
            }
        }
    }

    pub fn match_at(&mut self, s: &str, at: usize) -> Option<usize> {
        unsafe {
            let s = s.as_bytes();
            let start = s.as_ptr();
            let end = start.offset(s.len() as isize);
            let at = start.offset(at as isize);

            let region = Region::new();

            let r = onig_match(&mut self.0, start, end, at, region.0, ONIG_OPTION_NONE);
            if 0 <= r {
                Some(r as usize)
            } else {
                debug_assert!(r as ::std::os::raw::c_int == ONIG_MISMATCH);
                None
            }
        }
    }

    pub fn scan(
        &mut self,
        s: &str,
        mut cb: &mut FnMut(isize, isize, &mut Region) -> std::result::Result<(), i32>,
    ) -> std::result::Result<usize, isize> {
        unsafe extern "C" fn callback(
            start: OnigPosition,
            end: OnigPosition,
            region: *mut OnigRegion,
            f: *mut ::std::os::raw::c_void,
        ) -> ::std::os::raw::c_int {
            let f = mem::transmute::<
                _,
                &mut &mut FnMut(isize, isize, &mut Region)
                                -> std::result::Result<(), i32>,
            >(f);
            let start = start as isize;
            let end = end as isize;
            let mut region = Region(region);
            let ret = f(start, end, &mut region);
            // not to free the region
            mem::forget(region);
            match ret {
                Ok(_) => 0,
                Err(e) => e as ::std::os::raw::c_int,
            }
        }
        // TODO: check safety when a panic occurred in the callback function
        unsafe {
            let s = s.as_bytes();
            let start = s.as_ptr();
            let end = start.offset(s.len() as isize);
            let region = Region::new();

            let r = onig_scan(
                &mut self.0,
                start,
                end,
                region.0,
                ONIG_OPTION_NONE,
                Some(callback),
                mem::transmute(&mut cb),
            );
            if 0 <= r { Ok(r as usize) } else { Err(0) }
        }
    }
}

impl Drop for Regex {
    fn drop(&mut self) {
        unsafe { onig_free_body(&mut self.0) }
    }
}

impl Region {
    pub fn new() -> Self {
        unsafe {
            let region: *mut OnigRegion = onig_region_new();
            Region(region)
        }
    }

    pub fn positions(&self) -> PositionIter {
        let num_regs;
        unsafe {
            num_regs = (*self.0).num_regs;
        }
        PositionIter(self, 0..num_regs)
    }
}

impl Clone for Region {
    fn clone(&self) -> Self {
        unsafe {
            let to: *mut OnigRegion = mem::uninitialized();
            onig_region_copy(to, self.0);
            Region(to)
        }

    }
}

impl Drop for Region {
    fn drop(&mut self) {
        unsafe { onig_region_free(self.0, 1) }
    }
}


impl Error {
    fn new(pos: OnigPosition, error_info: Option<OnigErrorInfo>) -> Self {
        use std::str::from_utf8;
        let s: &mut [OnigUChar] = &mut [0; ONIG_MAX_ERROR_MESSAGE_LEN as usize];
        unsafe {
            let size = match error_info {
                Some(ei) => onig_error_code_to_str(s as *mut _ as *mut _, pos, ei),
                None => onig_error_code_to_str(s as *mut _ as *mut _, pos),
            };
            let size = size as usize;
            let s = from_utf8(&s[0..size]).unwrap().to_string();
            Error(pos, error_info, s)
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ERROR: {}\n", self.2)
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        &self.2
    }

    fn cause(&self) -> Option<&error::Error> {
        None
    }
}


impl<'a> Iterator for PositionIter<'a> {
    type Item = (usize, usize);
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let region = *(self.0).0;
            self.1.next().map(|i| {
                (
                    *region.beg.offset(i as isize) as usize,
                    *region.end.offset(i as isize) as usize,
                )
            })
        }
    }
}

//pub struct RegexBuilder



#[test]
fn test_search() {
    let mut reg = Regex::new("a(.*)b|[e-f]+".to_string()).unwrap();
    let s = "zzzzaffffffffb";
    let reg = reg.search(s).unwrap();
    assert_eq!(reg.positions().count(), 2);
}


#[test]
fn test_match_at() {
    let mut reg = Regex::new("a(.*)b|[e-f]+".to_string()).unwrap();
    let s = "zzzzaffffffffb";

    assert_eq!(reg.match_at(s, 3), None);
}

#[test]
fn test_scan() {
    let mut reg = Regex::new("ab".to_string()).unwrap();
    let s = "abcdabcdabcd";
    let r = reg.scan(s, &mut |start, end, _reg| {
        println!("{} {}", start, end);
        Ok(())
    }).unwrap();
    assert_eq!(r, 3);
}
