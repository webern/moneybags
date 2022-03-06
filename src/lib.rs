use anyhow::Result;
use fixed::types::I52F12;
use serde::{Deserialize, Deserializer, Serialize};
use serde_plain::{derive_display_from_serialize, derive_fromstr_from_deserialize};
use std::collections::BTreeMap;
use std::io::Read;
use std::str::FromStr;

/// Represents the type of record found in input CSV data.
#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordType {
    /// A deposit is a credit to the client’s asset account, meaning it should increase the
    /// available and total funds of the client account
    Deposit,

    /// A withdraw is a debit to the client’s asset account, meaning it should decrease the
    /// available and total funds of the client account
    Withdrawal,

    /// A dispute represents a client’s claim that a transaction was erroneous and should be
    /// reversed. The transaction shouldn’t be reversed yet but the associated funds should be held.
    /// This means that the clients available funds should decrease by the amount disputed, their
    /// held funds should increase by the amount disputed, while their total funds should remain the
    /// same.
    Dispute,

    /// A resolve represents a resolution to a dispute, releasing the associated held funds. Funds
    /// that were previously disputed are no longer disputed. This means that the clients held funds
    /// should decrease by the amount no longer disputed, their available funds should increase by
    /// the amount no longer disputed, and their total funds should remain the same.
    Resolve,

    /// A chargeback is the final state of a dispute and represents the client reversing a
    /// transaction. Funds that were held have now been withdrawn. This means that the clients held
    /// funds and total funds should decrease by the amount previously disputed. If a chargeback
    /// occurs the client’s account should be immediately frozen.
    Chargeback,
}

impl Default for RecordType {
    fn default() -> Self {
        Self::Deposit
    }
}

derive_fromstr_from_deserialize!(RecordType);
derive_display_from_serialize!(RecordType);

/// Represents a row in an input CSV data.
#[derive(Debug, Default, Clone, Eq, Ord, PartialEq, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Record {
    #[serde(rename = "type")]
    /// The type of record.
    record_type: RecordType,

    /// The client ID will be unique per client though are not guaranteed to be ordered.
    client: u16,

    /// Transaction IDs (tx) are globally unique and not guaranteed to be ordered.
    tx: u16,

    /// The amount of the transaction, stored in a signed, fixed-decimal type using 52 pits for the
    /// integral part of the number and 12 bits for the fractional part of the number.
    #[serde(default)]
    #[serde(deserialize_with = "parse_decimal")]
    amount: I52F12,
}

/// A custom deserializer for the fixed decimal type.
fn parse_decimal<'de, D>(d: D) -> Result<I52F12, D::Error>
where
    D: Deserializer<'de>,
{
    let value: String = match Option::deserialize(d)? {
        Some(value) => value,
        // We do not need to distinguish between nulls and zeros.
        None => return Ok(Default::default()),
    };
    let parsed =
        I52F12::from_str(&value).map_err(|e| serde::de::Error::custom(format!("{}", e)))?;
    Ok(parsed)
}

/// Although we could use serde `Deserialize` with `csv`, I find `csv`'s constraints on the file to
/// be too strict. I want to make sure that test data which is well-formed does not fail to parse,
/// so I have handcrafted some parsing using this wrapper struct. Using `serde` deserialize would
/// fail due to whitespace and/or incorrect number of commas in a record.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct Records(BTreeMap<u16, Record>);

impl Records {
    /// Parse the CSV data in a `reader`.
    pub fn from_reader(reader: impl Read) -> Result<Self> {
        let mut csv_reader = csv::Reader::from_reader(reader);
        let mut records = BTreeMap::new();

        for result in csv_reader.deserialize() {
            let record: Record = result?;
            records.insert(record.tx, record);
        }

        Ok(Self(records))
    }
}

/// Represents the status of a client/account.
#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Client {
    client: u16,
    available: I52F12,
    held: I52F12,
    total: I52F12,
    locked: bool,
}

impl Client {
    // /// Update the `Client` account status based on the transaction `record`.
    // fn process_record(&mut self, record: &Record) -> Result<()> {
    //     debug_assert!(transaction.client == self.client);
    //     match transaction.record_type {
    //         RecordType::Deposit => {}
    //         RecordType::Withdrawal => {}
    //         RecordType::Dispute => {}
    //         RecordType::Resolve => {}
    //         RecordType::Chargeback => {}
    //     }
    //     Ok(())
    // }
}

// Tests ///////////////////////////////////////////////////////////////////////////////////////////
#[cfg(test)]
mod test {
    use super::*;

    const DATA: &str = r#"type,client,tx,amount
deposit,1,1,1.0111
"withdrawal",2,22,2.0222
dispute,"33",3,
resolve,4,44,
chargeback,555,"55","#;

    #[test]
    fn input_parsing() {
        let records = Records::from_reader(DATA.as_bytes()).unwrap();

        assert_eq!(
            *records.0.get(&1).unwrap(),
            Record {
                record_type: RecordType::Deposit,
                client: 1,
                tx: 1,
                amount: I52F12::from_num(1.0111),
            }
        );

        assert_eq!(
            *records.0.get(&22).unwrap(),
            Record {
                record_type: RecordType::Withdrawal,
                client: 2,
                tx: 22,
                amount: I52F12::from_num(2.0222),
            }
        );

        assert_eq!(
            *records.0.get(&3).unwrap(),
            Record {
                record_type: RecordType::Dispute,
                client: 33,
                tx: 3,
                amount: Default::default(),
            }
        );

        assert_eq!(
            *records.0.get(&44).unwrap(),
            Record {
                record_type: RecordType::Resolve,
                client: 4,
                tx: 44,
                amount: Default::default(),
            }
        );

        assert_eq!(
            *records.0.get(&55).unwrap(),
            Record {
                record_type: RecordType::Chargeback,
                client: 555,
                tx: 55,
                amount: Default::default(),
            }
        );
    }
}
