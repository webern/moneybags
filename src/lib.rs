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
use std::path::PathBuf;
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
        let f = BufReader::new(
            File::open(&self.csv_file)
                .context(format!("Unable to open file '{}'", self.csv_file.display()))?,
        );
        let clients = process_records(f)?;
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
    client: u32,

    /// Transaction IDs (tx) are globally unique and not guaranteed to be ordered.
    tx: u32,

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

/// Represents the status of a client/account.
#[derive(
    Debug, Default, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub struct Client {
    #[serde(rename = "client")]
    id: u32,
    available: Decimal,
    held: Decimal,
    total: Decimal,
    locked: bool,
}

impl Client {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            ..Default::default()
        }
    }
}

fn process_records(reader: impl Read) -> Result<Vec<Client>> {
    let mut csv_reader = csv::Reader::from_reader(reader);
    let mut records = BTreeMap::new();
    let mut clients = BTreeMap::new();

    for result in csv_reader.deserialize() {
        let record: Record = match result {
            Ok(ok) => ok,
            Err(e) => {
                eprintln!("Error parsing csv line: {}", e);
                continue;
            }
        };

        if let Err(e) = process_record(&record, &records, &mut clients) {
            eprintln!("Error processing record: {}", e);
        }

        // We need to store transactions because they may become disputed later. We do not need to
        // store dispute, resolve or chargeback records because these can not be further referenced.
        if matches!(
            record.record_type,
            RecordType::Deposit | RecordType::Withdrawal
        ) {
            records.insert(record.tx, record);
        }
    }

    Ok(clients.into_iter().map(|(_, client)| client).collect())
}

fn process_record(
    record: &Record,
    records: &BTreeMap<u32, Record>,
    clients: &mut BTreeMap<u32, Client>,
) -> Result<()> {
    // We take a copy of the `Client` and overwrite it later to ensure atomicity.
    let mut client = *clients
        .entry(record.client)
        .or_insert_with(|| Client::new(record.client));

    // TODO - what if it is locked? https://github.com/webern/moneybags/issues/4
    // In the absence of guidance on locked accounts, we will assume that we
    // should not process records for accounts that are locked. Note that there
    // is no way for an account to become unlocked.
    ensure!(!client.locked, "Client account is locked");

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
            let disputed_record = records.get(&record.tx).context(format!(
                "Disputed record tx {} could not be found",
                record.tx
            ))?;
            ensure!(
                disputed_record.client == record.client,
                "Disputed record and current record have different client IDs"
            );
            client.available -= disputed_record.amount;
            client.held += disputed_record.amount;
        }
        RecordType::Resolve => {
            let resolved_record = records.get(&record.tx).context(format!(
                "Resolved record tx {} could not be found",
                record.tx
            ))?;
            ensure!(
                resolved_record.client == record.client,
                "Resolved record and current record have different client IDs"
            );
            // TODO - what happens if held is less than resolved amount?
            client.available += resolved_record.amount;
            client.held -= resolved_record.amount;
        }
        RecordType::Chargeback => {
            let chargeback_record = records.get(&record.tx).context(format!(
                "Chargeback record tx {} could not be found",
                record.tx
            ))?;
            ensure!(
                chargeback_record.client == record.client,
                "Chargeback record and current record have different client IDs"
            );
            // TODO - what happens if available/held are less than chargeback amount?
            client.total -= chargeback_record.amount;
            client.held -= chargeback_record.amount;
            client.locked = true;
        }
    }

    // Atomically update the map with our transaction by copying over the value in the map.
    clients.insert(client.id, client);

    Ok(())
}
