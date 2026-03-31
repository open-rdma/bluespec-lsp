extern crate cc;

fn main() {
    // 编译tree-sitter-bsv
    cc::Build::new()
        .include("src")
        .file("src/tree_sitter_bsv.c")
        .compile("tree-sitter-bsv");
    
    // 重新编译的触发条件
    println!("cargo:rerun-if-changed=src/tree_sitter_bsv.c");
    println!("cargo:rerun-if-changed=src/tree_sitter_bsv.h");
}
