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
pub struct Error(OnigPosition, Option<OnigErrorInfo>);
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

    pub extern fn cleanup() {
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
            let r = onig_new_without_alloc(&mut reg as *mut _,
                                           pattern.as_ptr() as *const OnigUChar,
                                           (pattern.as_ptr() as *const OnigUChar)
                                               .offset(pattern.len() as isize),
                                           ONIG_OPTION_NONE,
                                           &OnigEncodingUTF_8,
                                           // workaround for current version. fixed at master
                                           OnigDefaultSyntax as *mut _,
                                           &mut einfo);
            if (r as ::std::os::raw::c_uint) == ONIG_NORMAL {
                Ok(Regex(reg))
            } else {
                Err(Error(r as OnigPosition, Some(einfo)))
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

            let r = onig_search(&mut self.0,
                                start,
                                end,
                                start,
                                range,
                                region.0,
                                ONIG_OPTION_NONE);
            if 0 <= r {
                Some(region)
            } else  {
                debug_assert!(r as ::std::os::raw::c_int == ONIG_MISMATCH);
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

            let r = onig_match(&mut self.0,
                       start,
                       end,
                       at,
                       region.0,
                               ONIG_OPTION_NONE);
            if 0 <=r {
                Some(r as usize)
            } else {
                debug_assert!(r as ::std::os::raw::c_int == ONIG_MISMATCH);
                None
            }
        }
    }

    pub fn scan(&mut self, s: &str, mut cb: &mut FnMut(isize, isize, &mut Region) -> std::result::Result<(), i32>) -> std::result::Result<usize, isize> {
        unsafe extern fn callback(start: OnigPosition, end: OnigPosition, region: *mut OnigRegion, f: *mut ::std::os::raw::c_void) -> ::std::os::raw::c_int {
            let f = mem::transmute::<_, &mut &mut FnMut(isize, isize, &mut Region) -> std::result::Result<(), i32>>(f);
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

            let r = onig_scan(&mut self.0,
                              start,
                              end,
                              region.0,
                              ONIG_OPTION_NONE,
                              Some(callback),
                              mem::transmute(&mut cb)
            );
            if 0 <= r {
                Ok(r as usize)
            } else {
                Err(0)
            }
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


impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use std::str::from_utf8_unchecked;
        let s: &mut [OnigUChar] = &mut [0; ONIG_MAX_ERROR_MESSAGE_LEN as usize];
        unsafe {
            onig_error_code_to_str(s as *mut _ as *mut _, self.0 as OnigPosition, self.1);
            write!(f, "ERROR: {}\n", from_utf8_unchecked(s))
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match self.0 as ::std::os::raw::c_int {
            /* normal return */
            //ONIG_NORMAL: = 0;
            ONIG_MISMATCH => "mismatch",
            ONIG_NO_SUPPORT_CONFIG => "no support in this configuration",
            /* internal error */
            ONIGERR_MEMORY => "failed to allocate memory",
            ONIGERR_TYPE_BUG => "undefined type (bug)",
            ONIGERR_PARSER_BUG => "internal parser error (bug)",
            ONIGERR_STACK_BUG => "stack error (bug)",
            ONIGERR_UNDEFINED_BYTECODE => "undefined bytecode (bug)",
            ONIGERR_UNEXPECTED_BYTECODE => "unexpected bytecode (bug)",
            ONIGERR_MATCH_STACK_LIMIT_OVER => "match-stack limit over",
            ONIGERR_PARSE_DEPTH_LIMIT_OVER => "parse depth limit over",
            ONIGERR_DEFAULT_ENCODING_IS_NOT_SET => "default multibyte-encoding is not set",
            ONIGERR_SPECIFIED_ENCODING_CANT_CONVERT_TO_WIDE_CHAR => "can't convert to wide-char on specified multibyte-encoding",
            /* general error */
            ONIGERR_INVALID_ARGUMENT => "invalid argument",
            /* syntax error */
            ONIGERR_END_PATTERN_AT_LEFT_BRACE => "end pattern at left brace",
            ONIGERR_END_PATTERN_AT_LEFT_BRACKET => "end pattern at left bracket",
            ONIGERR_EMPTY_CHAR_CLASS => "empty char-class",
            ONIGERR_PREMATURE_END_OF_CHAR_CLASS => "premature end of char-class",
            ONIGERR_END_PATTERN_AT_ESCAPE => "end pattern at escape",
            ONIGERR_END_PATTERN_AT_META => "end pattern at meta",
            ONIGERR_END_PATTERN_AT_CONTROL => "end pattern at control",
            ONIGERR_META_CODE_SYNTAX => "invalid meta-code syntax",
            ONIGERR_CONTROL_CODE_SYNTAX => "invalid control-code syntax",
            ONIGERR_CHAR_CLASS_VALUE_AT_END_OF_RANGE => "char-class value at end of range",
            ONIGERR_CHAR_CLASS_VALUE_AT_START_OF_RANGE => "char-class value at start of range",
            ONIGERR_UNMATCHED_RANGE_SPECIFIER_IN_CHAR_CLASS => "unmatched range specifier in char-class",
            ONIGERR_TARGET_OF_REPEAT_OPERATOR_NOT_SPECIFIED => "target of repeat operator is not specified",
            ONIGERR_TARGET_OF_REPEAT_OPERATOR_INVALID => "target of repeat operator is invalid",
            ONIGERR_NESTED_REPEAT_OPERATOR => "nested repeat operator",
            ONIGERR_UNMATCHED_CLOSE_PARENTHESIS => "unmatched close parenthesis",
            ONIGERR_END_PATTERN_WITH_UNMATCHED_PARENTHESIS => "end pattern with unmatched parenthesis",
            ONIGERR_END_PATTERN_IN_GROUP => "end pattern in group",
            ONIGERR_UNDEFINED_GROUP_OPTION => "undefined group option",
            ONIGERR_INVALID_POSIX_BRACKET_TYPE => "invalid POSIX bracket type",
            ONIGERR_INVALID_LOOK_BEHIND_PATTERN => "invalid pattern in look-behind",
            ONIGERR_INVALID_REPEAT_RANGE_PATTERN => "invalid repeat range {lower,upper}",
            ONIGERR_INVALID_CONDITION_PATTERN => "invalid conditional pattern",
            /* values error (syntax error) */
            ONIGERR_TOO_BIG_NUMBER => "too big number",
            ONIGERR_TOO_BIG_NUMBER_FOR_REPEAT_RANGE => "too big number for repeat range",
            ONIGERR_UPPER_SMALLER_THAN_LOWER_IN_REPEAT_RANGE => "upper is smaller than lower in repeat range",
            ONIGERR_EMPTY_RANGE_IN_CHAR_CLASS => "empty range in char class",
            ONIGERR_MISMATCH_CODE_LENGTH_IN_CLASS_RANGE => "mismatch multibyte code length in char-class range",
            ONIGERR_TOO_MANY_MULTI_BYTE_RANGES => "too many multibyte code ranges are specified",
            ONIGERR_TOO_SHORT_MULTI_BYTE_STRING => "too short multibyte code string",
            ONIGERR_TOO_BIG_BACKREF_NUMBER => "too big backref number",
            ONIGERR_INVALID_BACKREF => "invalid backref number",
            ONIGERR_NUMBERED_BACKREF_OR_CALL_NOT_ALLOWED => "numbered backref/call is not allowed. (use name)",
            ONIGERR_TOO_MANY_CAPTURE_GROUPS => "too many capture groups are specified",
            ONIGERR_TOO_SHORT_DIGITS => "too short digits",
            ONIGERR_TOO_LONG_WIDE_CHAR_VALUE => "too long wide-char value",
            ONIGERR_EMPTY_GROUP_NAME => "group name is empty",
            ONIGERR_INVALID_GROUP_NAME => "invalid group name <%n>",
            ONIGERR_INVALID_CHAR_IN_GROUP_NAME => "invalid char in group name <%n>",
            ONIGERR_UNDEFINED_NAME_REFERENCE => "undefined name <%n> reference",
            ONIGERR_UNDEFINED_GROUP_REFERENCE => "undefined group <%n> reference",
            ONIGERR_MULTIPLEX_DEFINED_NAME => "multiplex defined name <%n>",
            ONIGERR_MULTIPLEX_DEFINITION_NAME_CALL => "multiplex definition name <%n> call",
            ONIGERR_NEVER_ENDING_RECURSION => "never ending recursion",
            ONIGERR_GROUP_NUMBER_OVER_FOR_CAPTURE_HISTORY => "group number is too big for capture history",
            ONIGERR_INVALID_CHAR_PROPERTY_NAME => "invalid character property name {%n}",
            ONIGERR_INVALID_WIDE_CHAR_VALUE => "invalid code point value",
            // ONIGERR_INVALID_CODE_POINT_VALUE => "invalid code point value",
            ONIGERR_TOO_BIG_WIDE_CHAR_VALUE => "too big wide-char value",
            ONIGERR_NOT_SUPPORTED_ENCODING_COMBINATION => "not supported encoding combination",
            ONIGERR_INVALID_COMBINATION_OF_OPTIONS => "invalid combination of options",
            _ => "undefined error code"
        }
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
            self.1
                .next()
                .map(|i| (*region.beg.offset(i as isize) as usize, *region.end.offset(i as isize) as usize))
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
    let r = reg.scan(s, &mut |start, end, _reg|{
        println!("{} {}", start, end);
        Ok(())
    }).unwrap();
    assert_eq!(r, 3);
}
