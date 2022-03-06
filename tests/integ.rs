use moneybags::Moneybags;
use std::io::Cursor;
use std::path::PathBuf;

fn path(filename: impl AsRef<str>) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join(filename.as_ref())
}

/// The example given in the specification should throw an error because a withdrawal attempt is
/// made with insufficient funds.
#[test]
fn given_example() {
    assert!(Moneybags {
        csv_file: path("given-example.csv"),
    }
    .run(Cursor::new(Vec::<u8>::new()))
    .is_err());
}
