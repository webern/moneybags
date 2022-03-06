/*

The implementation of the `moneybags` program. This library exists to facilitate integration
testing. It is not meant for publication.

*/
use anyhow::{ensure, Context, Result};
use clap::Parser;
use csv::WriterBuilder;
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize};
use serde_plain::{derive_display_from_serialize, derive_fromstr_from_deserialize};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Processes the transactions found in <CSV_FILE> and outputs a CSV to stdout summarizing the
/// end state of the accounts found therein.
#[derive(Parser, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[clap(name = "moneybags")]
#[clap(bin_name = "moneybags")]
pub struct Moneybags {
    /// The path to a CSV file containing transaction records.
    pub csv_file: PathBuf,
}

impl Moneybags {
    /// Writes a csv-formatted summary of the accounts found in `self.csv_file`. By taking a `Write`
    /// instead of writing to `stdout`, we make the program easier to test.
    pub fn run(&self, writer: impl Write) -> Result<()> {
        let records = Records::from_file(&self.csv_file)
            .context(format!("Unable to open file '{}'", self.csv_file.display()))?;
        let clients = process_records(&records)?;
        let mut csv_writer = WriterBuilder::new().has_headers(true).from_writer(writer);
        for client in clients {
            csv_writer.serialize(client)?;
        }
        Ok(())
    }
}

/// Represents the type of record found in input CSV data.
#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordType {
    /// A deposit is a credit to the client’s asset account, meaning it should increase the
    /// available and total funds of the client account.
    Deposit,

    /// A withdraw is a debit to the client’s asset account, meaning it should decrease the
    /// available and total funds of the client account.
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

/// Represents an input row in CSV transaction data.
#[derive(Debug, Default, Clone, Eq, Ord, PartialEq, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Record {
    #[serde(rename = "type")]
    /// The type of record.
    record_type: RecordType,

    /// The client ID will be unique per client though are not guaranteed to be ordered.
    client: u64,

    /// Transaction IDs (tx) are globally unique and not guaranteed to be ordered.
    tx: u64,

    /// The amount of the transaction, in fixed-precision decimal type. This type will not
    /// accumulate errors like a floating point type would.
    #[serde(default)]
    #[serde(deserialize_with = "parse_decimal")]
    amount: Decimal,
}

/// A custom deserializer for the fixed decimal type.
fn parse_decimal<'de, D>(d: D) -> Result<Decimal, D::Error>
where
    D: Deserializer<'de>,
{
    let value: String = match Option::deserialize(d)? {
        Some(value) => value,
        // We do not need to distinguish between nulls and zeros.
        None => return Ok(Default::default()),
    };
    let parsed =
        Decimal::from_str(&value).map_err(|e| serde::de::Error::custom(format!("{}", e)))?;
    Ok(parsed)
}

/// A wrapping type for a collection of records.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct Records(BTreeMap<Id, Record>);

/// Dispute, Resolve and Chargeback records do not have an IDs of there own, but I want to give them
/// some unique ID so they can be stored in the same map as Deposit and Withdrawal transactions.
#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Hash)]
enum Id {
    /// A Deposit or Withdrawal that has a real ID.
    Tx(u64),
    /// A Dispute, Resolve or Chargeback for which we have not been given a real ID. We make one up
    /// instead.
    Fake(u64),
}

impl Records {
    /// Parse the CSV data in a `reader`.
    pub fn from_reader(reader: impl Read) -> Result<Self> {
        let mut csv_reader = csv::Reader::from_reader(reader);
        let mut records = BTreeMap::new();
        let mut fake_id = 0u64;

        for result in csv_reader.deserialize() {
            let record: Record = result?;
            // HACK - Dispute, Resolve and Chargeback records do not have a transaction ID. We want
            // to keep them in a map along with the Deposit and Withdrawal transactions that do have
            // IDs so we create a synthetic "fake" ID for the transactions that do not have one.
            let id = match record.record_type {
                RecordType::Deposit | RecordType::Withdrawal => Id::Tx(record.tx),
                RecordType::Dispute | RecordType::Resolve | RecordType::Chargeback => {
                    fake_id += 1;
                    Id::Fake(fake_id)
                }
            };
            records.insert(id, record);
        }

        Ok(Self(records))
    }

    /// Parse the CSV data from a file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let f = File::open(path.as_ref())?;
        let br = BufReader::new(f);
        Self::from_reader(br)
    }
}

/// Represents the status of a client/account.
#[derive(
    Debug, Default, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub struct Client {
    #[serde(rename = "client")]
    id: u64,
    available: Decimal,
    held: Decimal,
    total: Decimal,
    locked: bool,
}

impl Client {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            ..Default::default()
        }
    }
}

fn process_records(records: &Records) -> Result<Vec<Client>> {
    let mut clients = BTreeMap::new();
    for record in records.0.values() {
        // We take a copy of the `Client` and overwrite it later to ensure atomicity.
        let mut client = *clients
            .entry(record.client)
            .or_insert_with(|| Client::new(record.client));
        // TODO - what should we do if an account is already frozen?
        match record.record_type {
            RecordType::Deposit => {
                client.available += record.amount;
                client.total += record.amount;
            }
            RecordType::Withdrawal => {
                ensure!(
                    client.available >= record.amount,
                    "Withdrawal failed. Available funds insufficient."
                );
                client.available -= record.amount;
                client.total -= record.amount;
            }
            RecordType::Dispute => {
                let disputed_record = records.0.get(&Id::Tx(record.tx)).context(format!(
                    "Disputed record tx {} could not be found",
                    record.tx
                ))?;
                client.available -= disputed_record.amount;
                client.held += disputed_record.amount;
            }
            RecordType::Resolve => {
                let resolved_record = records.0.get(&Id::Tx(record.tx)).context(format!(
                    "Resolved record tx {} could not be found",
                    record.tx
                ))?;
                // TODO - what happens if held is less than resolved amount?
                client.available += resolved_record.amount;
                client.held -= resolved_record.amount;
            }
            RecordType::Chargeback => {
                let chargeback_record = records.0.get(&Id::Tx(record.tx)).context(format!(
                    "Chargeback record tx {} could not be found",
                    record.tx
                ))?;
                // TODO - what happens if available/held are less than chargeback amount?
                client.available -= chargeback_record.amount;
                client.held -= chargeback_record.amount;
                client.locked = true;
            }
        }

        // Atomically update the map with our transaction by copying over the value in the map. If
        // this were a real system we would need to verify that the data in the database has not
        // changed since we first accessed it. This can be done with object version numbers or
        // database transactions. (Watch out for database transactions though!)
        clients.insert(client.id, client);
    }

    Ok(clients.into_iter().map(|(_, client)| client).collect())
}

// Tests ///////////////////////////////////////////////////////////////////////////////////////////
#[cfg(test)]
mod test {
    use super::*;
    use maplit::btreemap;

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
            *records.0.get(&Id::Tx(1)).unwrap(),
            Record {
                record_type: RecordType::Deposit,
                client: 1,
                tx: 1,
                amount: Decimal::new(10111, 4),
            }
        );

        assert_eq!(
            *records.0.get(&Id::Tx(22)).unwrap(),
            Record {
                record_type: RecordType::Withdrawal,
                client: 2,
                tx: 22,
                amount: Decimal::new(20222, 4),
            }
        );

        assert_eq!(
            *records.0.get(&Id::Fake(1)).unwrap(),
            Record {
                record_type: RecordType::Dispute,
                client: 33,
                tx: 3,
                amount: Default::default(),
            }
        );

        assert_eq!(
            *records.0.get(&Id::Fake(2)).unwrap(),
            Record {
                record_type: RecordType::Resolve,
                client: 4,
                tx: 44,
                amount: Default::default(),
            }
        );

        assert_eq!(
            *records.0.get(&Id::Fake(3)).unwrap(),
            Record {
                record_type: RecordType::Chargeback,
                client: 555,
                tx: 55,
                amount: Default::default(),
            }
        );
    }

    #[test]
    fn process_resolve() {
        let records = Records(btreemap![
          Id::Tx(1) => Record {
                record_type: RecordType::Deposit,
                client: 1,
                tx: 1,
                amount: Decimal::new(2009999, 4),
            },
          Id::Tx(2) => Record {
                record_type: RecordType::Withdrawal,
                client: 1,
                tx: 2,
                amount: Decimal::new(1005001, 4),
            },
          Id::Fake(1) => Record {
                record_type: RecordType::Dispute,
                client: 1,
                tx: 2,
                ..Default::default()
            },
          Id::Fake(2) => Record {
                record_type: RecordType::Resolve,
                client: 1,
                tx: 2,
                ..Default::default()
            },
        ]);

        let clients = process_records(&records).unwrap();
        let client = clients.first().unwrap();
        assert_eq!(
            *client,
            Client {
                id: 1,
                // 100.4998
                available: Decimal::new(1004998, 4),
                total: Decimal::new(1004998, 4),
                ..Default::default()
            }
        );
    }

    #[test]
    fn process_chargeback() {
        let records = Records(btreemap![
          Id::Tx(1) => Record {
                record_type: RecordType::Deposit,
                client: 1,
                tx: 1,
                amount: Decimal::new(2009999, 4),
            },
          Id::Tx(2) => Record {
                record_type: RecordType::Withdrawal,
                client: 1,
                tx: 2,
                amount: Decimal::new(1005001, 4),
            },
          Id::Fake(1) => Record {
                record_type: RecordType::Dispute,
                client: 1,
                tx: 2,
                ..Default::default()
            },
          Id::Fake(2) => Record {
                record_type: RecordType::Chargeback,
                client: 1,
                tx: 2,
                ..Default::default()
            },
        ]);

        let clients = process_records(&records).unwrap();
        let client = clients.first().unwrap();
        assert_eq!(
            *client,
            Client {
                id: 1,
                available: Decimal::new(-1005004, 4),
                total: Decimal::new(1004998, 4),
                locked: true,
                ..Default::default()
            }
        );
    }
}
