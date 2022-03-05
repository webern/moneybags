
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
