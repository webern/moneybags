
# Moneybags

This program was written for a take-home assignment and has no value for any other purpose.

## Usage

Takes a CSV file as input, process the transactions described therein, and outputs a CSV file to `stdout` representing
the resultant account balances.

Example: `moneybags transactions.csv > accounts.csv`

Given this input:

```csv
type, client, tx, amount
deposit, 1, 1, 1.0
deposit, 2, 2, 2.0
deposit, 1, 3, 2.0
withdrawal, 1, 4, 1.5
withdrawal, 2, 5, 3.0
```

Produces this output:

```csv
client, available, held, total, locked
1, 1.5, 0.0, 1.5, false
2, 2.0, 0.0, 2.0, false
```

Specifications for the input, output and behavior were provided and are not repeated here.
These specifications are relatively simple and can be deduced from reading the code easily enough.

## Development

The normal cargo commands should work fine: `cargo build`, `cargo test`, etc.
Continuous integration checks formatting, tests, build, install and clippy linting in strict mode.

To do the same checks locally, run `make check`.

## Discussion

### Correctness

There are cases in the code where the specification may be unclear on desired program behavior.
I have created follow-up GitHub issues for each of these, listed here,
and linked to them in the codebase at their approximate locations.
- [dispute, resolve and chargeback transactions have no IDs](https://github.com/webern/moneybags/issues/2)
- [define min and max values for amount and client id types](https://github.com/webern/moneybags/issues/3)
- [transactions on a frozen account](https://github.com/webern/moneybags/issues/4)
- [can both debit and withdrawal transactions be disputed?](https://github.com/webern/moneybags/issues/5)

Edit: more questions...
- What should we do if the client ID of a Chargeback, Resolve or Dispute does not match the client ID of the original 
  record? Error? (Not handled.)
- What should we do if a Chargeback or Resolve does not have a corresponding Dispute? (Not handled.)

> For the cases you are handling are you handling them correctly?

Probably?
For this I tried to make sure I followed the spec's directions carefully.
I have clarifying questions listed above.

> Did you test against sample data?

I added some sample data and integration tests to `tests` directory.

> Did you write unit tests for the complicated bits?

Yes, I added some unit tests for parsing and record processing.
I wrote integration tests for the end-to-end CSV-in, CSV-out workflow.

> Are you doing something dangerous? Tell us why you chose to do it this way

Types are not checked for wrapping when adding and subtracting.
These could panic if wrapping occurs.
I did it this way for lack of time.
In critical code we would check for wrapping values and return an error instead.

The implementation is not thread safe because there is only one thread.
As such, the implementation does not hold a transaction on the imaginary database.
This is called out in a comment.

> Can you stream values through memory as opposed to loading the entire data set upfront?

During parsing the data is streamed through Serde, but each deserialized value is being loaded in memory.
We are doing it this way because rows of data can reference other rows of data with no way to know which rows may become
"referenced".

## Maintainability

- The code uses flexible input types to facilitate testing.
- The `main` function does very little other than call a function that is itself test-able.
- The code is documented.
