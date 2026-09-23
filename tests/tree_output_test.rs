//! Tree output is pcli2's, not the environment's.
//!
//! `ptree` renders the `--format tree` output. With its default features it also
//! read `~/.config/ptree.toml` and `PTREE_*` variables, so a user's ptree settings
//! could change what pcli2 printed. This pins the rendering.

use ptree::TreeBuilder;

#[test]
fn tree_rendering_is_fixed() {
    let tree = TreeBuilder::new("top.asm".to_string())
        .begin_child("sub.asm".to_string())
        .add_empty_child("bolt.prt".to_string())
        .end_child()
        .add_empty_child("plate.prt".to_string())
        .build();
    let mut out = Vec::new();
    ptree::write_tree(&tree, &mut out).unwrap();
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "top.asm\n├─ sub.asm\n│  └─ bolt.prt\n└─ plate.prt\n"
    );
}
