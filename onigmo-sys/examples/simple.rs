extern crate onigmo_sys;

use onigmo_sys::*;
use std::mem;
use std::str::from_utf8_unchecked;

fn main() {
    unsafe {
        let pattern = b"a(.*)b|[e-f]+";
        let s = b"zzzzaffffffffb";
        let mut reg: regex_t = mem::zeroed();
        let mut einfo: OnigErrorInfo = mem::zeroed();
        let r = onig_new_without_alloc(&mut reg as *mut _,
                                       pattern as *const OnigUChar,
                                       (pattern as *const OnigUChar).offset(pattern.len() as
                                                                            isize),
                                       ONIG_OPTION_NONE,
                                       &OnigEncodingASCII,
                                       OnigDefaultSyntax as *mut _,
                                       &mut einfo);
        if (r as ::std::os::raw::c_uint) != ONIG_NORMAL {
            let s: &mut [OnigUChar] = &mut [0; ONIG_MAX_ERROR_MESSAGE_LEN as usize];
            onig_error_code_to_str(s as *mut _ as *mut _, r as OnigPosition, &einfo);
            println!("ERROR: {}\n", from_utf8_unchecked(s));
        }

        let region = onig_region_new();

        let end = (s as *const _).offset(s.len() as isize);
        let start = s as *const _;
        let range = end;
        let mut r = onig_search(&mut reg,
                                s as *const _,
                                end,
                                start,
                                range,
                                region,
                                ONIG_OPTION_NONE);
        if 0 <= r {
            println!("match at {}", r);
            let region = region.as_ref().unwrap();
            for i in 0..(region.num_regs) {
                println!("{}: ({}-{})",
                         i,
                         *region.beg.offset(i as isize),
                         *region.end.offset(i as isize));
            }
            r = 0;
        } else if (r as ::std::os::raw::c_int) == ONIG_MISMATCH {
            println!("search fail");
            r = -1;
        } else {
            let s: &mut [OnigUChar] = &mut [0; ONIG_MAX_ERROR_MESSAGE_LEN as usize];
            onig_error_code_to_str(s as *mut _ as *mut _, r as OnigPosition, &einfo);
            println!("ERROR: {}\n", from_utf8_unchecked(s));
            std::process::exit(-1);
        }
        onig_region_free(region, 1);
        onig_free_body(&mut reg);
        onig_end();
    }

}
