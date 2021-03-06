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
    let mut output_bytes = Cursor::new(Vec::<u8>::new());
    Moneybags {
        csv_file: path("given-example.csv"),
    }
    .run(&mut output_bytes)
    .unwrap();

    let output = String::from_utf8(output_bytes.into_inner()).unwrap();
    let expected = r#"client,available,held,total,locked
1,1.5,0,1.5,false
2,2.0,0,2.0,false
"#;
    assert_eq!(output, expected);
}

/// An example containing two clients, one of which has a resolve and the other a chargeback.
// TODO - this test may need to change depending on https://github.com/webern/moneybags/issues/4
// Explanation: the final withdrawal happens when client 2's account is frozen.
#[test]
fn resolve_and_chargeback() {
    let mut output_bytes = Cursor::new(Vec::<u8>::new());
    Moneybags {
        csv_file: path("resolve-and-chargeback.csv"),
    }
    .run(&mut output_bytes)
    .unwrap();

    let output = String::from_utf8(output_bytes.into_inner()).unwrap();
    let expected = r#"client,available,held,total,locked
1,3.4,0.0,3.4,false
2,1.4999,0.0,1.4999,true
"#;
    assert_eq!(output, expected);
}
