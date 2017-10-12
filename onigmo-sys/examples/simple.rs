extern crate onigmo_sys;

use onigmo_sys::*;
use std::mem;
use std::str::from_utf8_unchecked;

fn main() {
    unsafe {
        // 正規表現のパターン文字列です
        let pattern = b"a(.*)b|[e-f]+";
        // マッチ対象です
        let s = b"zzzzaffffffffb";
        // `onig_new_without_alloc`で初期化するメモリをスタックに確保します。
        let mut reg: regex_t = mem::uninitialized();
        let mut einfo: OnigErrorInfo = mem::uninitialized();
        // 正規表現文字列をコンパイルし、`reg`に格納します。
        let r = onig_new_without_alloc(&mut reg as *mut _,
                                       // パターン文字列の先頭です
                                       pattern as *const OnigUChar,
                                       // パターン文字列の末尾です
                                       (pattern as *const OnigUChar).offset(pattern.len() as
                                                                            isize),
                                       // 今回、オプションは特には付けません
                                       ONIG_OPTION_NONE,
                                       // Rustの文字列はUTF-8エンコーディングです
                                       &OnigEncodingUTF_8,
                                       OnigDefaultSyntax as *mut _,
                                       &mut einfo);
        // 返り値が正常値でなければエラーです
        if (r as ::std::os::raw::c_uint) != ONIG_NORMAL {
            // エラー情報を取得し印字します
            let s: &mut [OnigUChar] = &mut [0; ONIG_MAX_ERROR_MESSAGE_LEN as usize];
            onig_error_code_to_str(s as *mut _ as *mut _, r as OnigPosition, &einfo);
            println!("ERROR: {}\n", from_utf8_unchecked(s));
            // 正規表現のエラーならそのまま終了します
            return;
        }

        // マッチ情報を表わすデータを準備します。
        let region = onig_region_new();

        // マッチ対象文字列の終端です
        let end = (s as *const _).offset(s.len() as isize);
        // マッチ開始位置です
        let start = s as *const _;
        // マッチ終了位置です
        let range = end;
        // 正規表現でマッチします
        let mut r = onig_search(&mut reg,
                                s as *const _,
                                end,
                                start,
                                range,
                                region,
                                ONIG_OPTION_NONE);
        if 0 <= r {
            // 返り値が0以上ならマッチしています
            println!("match at {}", r);
            let region = region.as_ref().unwrap();
            // グルーピングされた部分正規表現毎にマッチ位置を表示します
            for i in 0..(region.num_regs) {
                println!("{}: ({}-{})",
                         i,
                         *region.beg.offset(i as isize),
                         *region.end.offset(i as isize));
            }
            r = 0;
        } else if (r as ::std::os::raw::c_int) == ONIG_MISMATCH {
            // 返り値が`ONIG_MISMATCH`なら正規表現とマッチしませんでした
            println!("search fail");
            r = -1;
        } else {
            // それ以外ではOnigmoの内部エラーです
            let s: &mut [OnigUChar] = &mut [0; ONIG_MAX_ERROR_MESSAGE_LEN as usize];
            onig_error_code_to_str(s as *mut _ as *mut _, r as OnigPosition, &einfo);
            println!("ERROR: {}\n", from_utf8_unchecked(s));
            std::process::exit(-1);
        }
        // 使ったリソースを手動で解放します。
        onig_region_free(region, 1);
        onig_free_body(&mut reg);
        onig_end();
        std::process::exit(r as i32);
    }

}
