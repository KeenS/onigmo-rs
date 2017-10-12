extern crate bindgen;

use std::env;
use std::path::PathBuf;

fn main() {
    // onigmoの共有ライブラリを使うことをcargoがrustcに伝えるように伝えます
    println!("cargo:rustc-link-lib=onigmo");

    // `binden::Builder`がbindgenを使うときのメインのエントリーポイントです
    // オプションを設定できます。
    let bindings = bindgen::Builder::default()
        // featureを要求したりnightlyでしか動かないような
        // unstableなコード使いません
        //.rust_target(LATEST_STABLE_RUST)
        // バインディングを作る基になるヘッダファイルです
        .header("wrapper.h")
        // ビルダーを完了してバインディングを生成します
        .generate()
        .expect("Unable to generate bindings");

    // バインディングを`$OUT_DIR/bindings.rs`に書き出します。
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
