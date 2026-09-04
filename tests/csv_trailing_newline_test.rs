//! Regression: CSV output must end with exactly what the writer produced minus
//! its trailing line break, so `println!` adds the only newline.

use pcli2::format::{OutputFormat, OutputFormatOptions, OutputFormatter};
use pcli2::model::{Asset, AssetList};

#[test]
fn asset_list_csv_has_no_trailing_line_break() {
    let options = OutputFormatOptions {
        with_metadata: false,
        with_headers: true,
        pretty: false,
    };
    let out = AssetList::from(Vec::<Asset>::new())
        .format(OutputFormat::Csv(options))
        .unwrap();
    assert_eq!(out, "NAME,PATH,TYPE,STATE,IS_ASSEMBLY,UUID");
}
