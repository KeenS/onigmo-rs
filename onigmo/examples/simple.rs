extern crate onigmo as onig;

fn main() {
    let mut reg = onig::Regex::new("a(.*)b|[e-f]+".to_string()).unwrap();
    let s = "zzzzaffffffffb";
    match reg.search(s) {
        Some(reg) => {
            use std::str::from_utf8;
            for (beg, end) in reg.positions() {
                println!("{}", from_utf8(&s.as_bytes()[beg..end]).unwrap());
            }
        }
        None => println!("not match"),
    }

    assert_eq!(reg.match_at(s, 3), None);

}
