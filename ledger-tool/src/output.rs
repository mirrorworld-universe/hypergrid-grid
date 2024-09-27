use {
    crate::ledger_utils::get_program_ids,
    chrono::{Local, TimeZone},
    serde::{Deserialize, Serialize},
    solana_cli_output::{display::writeln_transaction, OutputFormat, QuietDisplay, VerboseDisplay},
    solana_entry::entry::Entry,
    solana_ledger::blockstore::Blockstore,
    solana_sdk::{
        clock::{Slot, UnixTimestamp},
        hash::Hash,
        native_token::lamports_to_sol,
        pubkey::Pubkey,
    },
    solana_transaction_status::{
        EncodedConfirmedBlock, EncodedTransactionWithStatusMeta, EntrySummary, Rewards,
    },
    std::{
        collections::HashMap,
        fmt::{self, Display, Formatter},
        io::{stdout, Write},
        result::Result,
    },
};

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SlotInfo {
    pub total: usize,
    pub first: Option<u64>,
    pub last: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_after_last_root: Option<usize>,
}

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SlotBounds<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all_slots: Option<&'a Vec<u64>>,
    pub slots: SlotInfo,
    pub roots: SlotInfo,
}

impl VerboseDisplay for SlotBounds<'_> {}
impl QuietDisplay for SlotBounds<'_> {}

impl Display for SlotBounds<'_> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        if self.slots.total > 0 {
            let first = self.slots.first.unwrap();
            let last = self.slots.last.unwrap();

            if first != last {
                writeln!(
                    f,
                    "Ledger has data for {:?} slots {:?} to {:?}",
                    self.slots.total, first, last
                )?;

                if let Some(all_slots) = self.all_slots {
                    writeln!(f, "Non-empty slots: {all_slots:?}")?;
                }
            } else {
                writeln!(f, "Ledger has data for slot {first:?}")?;
            }

            if self.roots.total > 0 {
                let first_rooted = self.roots.first.unwrap_or_default();
                let last_rooted = self.roots.last.unwrap_or_default();
                let num_after_last_root = self.roots.num_after_last_root.unwrap_or_default();
                writeln!(
                    f,
                    "  with {:?} rooted slots from {:?} to {:?}",
                    self.roots.total, first_rooted, last_rooted
                )?;

                writeln!(f, "  and {num_after_last_root:?} slots past the last root")?;
            } else {
                writeln!(f, "  with no rooted slots")?;
            }
        } else {
            writeln!(f, "Ledger is empty")?;
        }

        Ok(())
    }
}

fn writeln_entry(f: &mut dyn fmt::Write, i: usize, entry: &CliEntry, prefix: &str) -> fmt::Result {
    writeln!(
        f,
        "{prefix}Entry {} - num_hashes: {}, hash: {}, transactions: {}, starting_transaction_index: {}",
        i, entry.num_hashes, entry.hash, entry.num_transactions, entry.starting_transaction_index,
    )
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliEntries {
    pub entries: Vec<CliEntry>,
    #[serde(skip_serializing)]
    pub slot: Slot,
}

impl QuietDisplay for CliEntries {}
impl VerboseDisplay for CliEntries {}

impl fmt::Display for CliEntries {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Slot {}", self.slot)?;
        for (i, entry) in self.entries.iter().enumerate() {
            writeln_entry(f, i, entry, "  ")?;
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliEntry {
    num_hashes: u64,
    hash: String,
    num_transactions: u64,
    starting_transaction_index: usize,
}

impl From<EntrySummary> for CliEntry {
    fn from(entry_summary: EntrySummary) -> Self {
        Self {
            num_hashes: entry_summary.num_hashes,
            hash: entry_summary.hash.to_string(),
            num_transactions: entry_summary.num_transactions,
            starting_transaction_index: entry_summary.starting_transaction_index,
        }
    }
}

impl From<&CliPopulatedEntry> for CliEntry {
    fn from(populated_entry: &CliPopulatedEntry) -> Self {
        Self {
            num_hashes: populated_entry.num_hashes,
            hash: populated_entry.hash.clone(),
            num_transactions: populated_entry.num_transactions,
            starting_transaction_index: populated_entry.starting_transaction_index,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliPopulatedEntry {
    num_hashes: u64,
    hash: String,
    num_transactions: u64,
    starting_transaction_index: usize,
    transactions: Vec<EncodedTransactionWithStatusMeta>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliBlockWithEntries {
    #[serde(flatten)]
    pub encoded_confirmed_block: EncodedConfirmedBlockWithEntries,
    #[serde(skip_serializing)]
    pub slot: Slot,
}

impl QuietDisplay for CliBlockWithEntries {}
impl VerboseDisplay for CliBlockWithEntries {}

impl fmt::Display for CliBlockWithEntries {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Slot: {}", self.slot)?;
        writeln!(
            f,
            "Parent Slot: {}",
            self.encoded_confirmed_block.parent_slot
        )?;
        writeln!(f, "Blockhash: {}", self.encoded_confirmed_block.blockhash)?;
        writeln!(
            f,
            "Previous Blockhash: {}",
            self.encoded_confirmed_block.previous_blockhash
        )?;
        if let Some(block_time) = self.encoded_confirmed_block.block_time {
            writeln!(
                f,
                "Block Time: {:?}",
                Local.timestamp_opt(block_time, 0).unwrap()
            )?;
        }
        if let Some(block_height) = self.encoded_confirmed_block.block_height {
            writeln!(f, "Block Height: {block_height:?}")?;
        }
        if !self.encoded_confirmed_block.rewards.is_empty() {
            let mut rewards = self.encoded_confirmed_block.rewards.clone();
            rewards.sort_by(|a, b| a.pubkey.cmp(&b.pubkey));
            let mut total_rewards = 0;
            writeln!(f, "Rewards:")?;
            writeln!(
                f,
                "  {:<44}  {:^15}  {:<15}  {:<20}  {:>14}  {:>10}",
                "Address", "Type", "Amount", "New Balance", "Percent Change", "Commission"
            )?;
            for reward in rewards {
                let sign = if reward.lamports < 0 { "-" } else { "" };

                total_rewards += reward.lamports;
                #[allow(clippy::format_in_format_args)]
                writeln!(
                    f,
                    "  {:<44}  {:^15}  {:>15}  {}  {}",
                    reward.pubkey,
                    if let Some(reward_type) = reward.reward_type {
                        format!("{reward_type}")
                    } else {
                        "-".to_string()
                    },
                    format!(
                        "{}◎{:<14.9}",
                        sign,
                        lamports_to_sol(reward.lamports.unsigned_abs())
                    ),
                    if reward.post_balance == 0 {
                        "          -                 -".to_string()
                    } else {
                        format!(
                            "◎{:<19.9}  {:>13.9}%",
                            lamports_to_sol(reward.post_balance),
                            (reward.lamports.abs() as f64
                                / (reward.post_balance as f64 - reward.lamports as f64))
                                * 100.0
                        )
                    },
                    reward
                        .commission
                        .map(|commission| format!("{commission:>9}%"))
                        .unwrap_or_else(|| "    -".to_string())
                )?;
            }

            let sign = if total_rewards < 0 { "-" } else { "" };
            writeln!(
                f,
                "Total Rewards: {}◎{:<12.9}",
                sign,
                lamports_to_sol(total_rewards.unsigned_abs())
            )?;
        }
        for (index, entry) in self.encoded_confirmed_block.entries.iter().enumerate() {
            writeln_entry(f, index, &entry.into(), "")?;
            for (index, transaction_with_meta) in entry.transactions.iter().enumerate() {
                writeln!(f, "  Transaction {index}:")?;
                writeln_transaction(
                    f,
                    &transaction_with_meta.transaction.decode().unwrap(),
                    transaction_with_meta.meta.as_ref(),
                    "    ",
                    None,
                    None,
                )?;
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodedConfirmedBlockWithEntries {
    pub previous_blockhash: String,
    pub blockhash: String,
    pub parent_slot: Slot,
    pub entries: Vec<CliPopulatedEntry>,
    pub rewards: Rewards,
    pub block_time: Option<UnixTimestamp>,
    pub block_height: Option<u64>,
}

impl EncodedConfirmedBlockWithEntries {
    pub fn try_from(
        block: EncodedConfirmedBlock,
        entries_iterator: impl Iterator<Item = EntrySummary>,
    ) -> Result<Self, String> {
        let mut entries = vec![];
        for (i, entry) in entries_iterator.enumerate() {
            let ending_transaction_index = entry
                .starting_transaction_index
                .saturating_add(entry.num_transactions as usize);
            let transactions = block
                .transactions
                .get(entry.starting_transaction_index..ending_transaction_index)
                .ok_or(format!(
                    "Mismatched entry data and transactions: entry {:?}",
                    i
                ))?;
            entries.push(CliPopulatedEntry {
                num_hashes: entry.num_hashes,
                hash: entry.hash.to_string(),
                num_transactions: entry.num_transactions,
                starting_transaction_index: entry.starting_transaction_index,
                transactions: transactions.to_vec(),
            });
        }
        Ok(Self {
            previous_blockhash: block.previous_blockhash,
            blockhash: block.blockhash,
            parent_slot: block.parent_slot,
            entries,
            rewards: block.rewards,
            block_time: block.block_time,
            block_height: block.block_height,
        })
    }
}

pub fn output_slot_rewards(blockstore: &Blockstore, slot: Slot, method: &OutputFormat) {
    // Note: rewards are not output in JSON yet
    if *method == OutputFormat::Display {
        if let Ok(Some(rewards)) = blockstore.read_rewards(slot) {
            if !rewards.is_empty() {
                println!("  Rewards:");
                println!(
                    "    {:<44}  {:^15}  {:<15}  {:<20}  {:>10}",
                    "Address", "Type", "Amount", "New Balance", "Commission",
                );

                for reward in rewards {
                    let sign = if reward.lamports < 0 { "-" } else { "" };
                    println!(
                        "    {:<44}  {:^15}  {}◎{:<14.9}  ◎{:<18.9}   {}",
                        reward.pubkey,
                        if let Some(reward_type) = reward.reward_type {
                            format!("{reward_type}")
                        } else {
                            "-".to_string()
                        },
                        sign,
                        lamports_to_sol(reward.lamports.unsigned_abs()),
                        lamports_to_sol(reward.post_balance),
                        reward
                            .commission
                            .map(|commission| format!("{commission:>9}%"))
                            .unwrap_or_else(|| "    -".to_string())
                    );
                }
            }
        }
    }
}

pub fn output_entry(
    blockstore: &Blockstore,
    method: &OutputFormat,
    slot: Slot,
    entry_index: usize,
    entry: Entry,
) {
    match method {
        OutputFormat::Display => {
            println!(
                "  Entry {} - num_hashes: {}, hash: {}, transactions: {}",
                entry_index,
                entry.num_hashes,
                entry.hash,
                entry.transactions.len()
            );
            for (transactions_index, transaction) in entry.transactions.into_iter().enumerate() {
                println!("    Transaction {transactions_index}");
                let tx_signature = transaction.signatures[0];
                let tx_status_meta = blockstore
                    .read_transaction_status((tx_signature, slot))
                    .unwrap_or_else(|err| {
                        eprintln!(
                            "Failed to read transaction status for {} at slot {}: {}",
                            transaction.signatures[0], slot, err
                        );
                        None
                    })
                    .map(|meta| meta.into());

                solana_cli_output::display::println_transaction(
                    &transaction,
                    tx_status_meta.as_ref(),
                    "      ",
                    None,
                    None,
                );
            }
        }
        OutputFormat::Json => {
            // Note: transaction status is not output in JSON yet
            serde_json::to_writer(stdout(), &entry).expect("serialize entry");
            stdout().write_all(b",\n").expect("newline");
        }
        _ => unreachable!(),
    }
}

pub fn output_slot(
    blockstore: &Blockstore,
    slot: Slot,
    allow_dead_slots: bool,
    method: &OutputFormat,
    verbose_level: u64,
    all_program_ids: &mut HashMap<Pubkey, u64>,
) -> Result<(), String> {
    if blockstore.is_dead(slot) {
        if allow_dead_slots {
            if *method == OutputFormat::Display {
                println!(" Slot is dead");
            }
        } else {
            return Err("Dead slot".to_string());
        }
    }

    let (entries, num_shreds, is_full) = blockstore
        .get_slot_entries_with_shred_info(slot, 0, allow_dead_slots)
        .map_err(|err| format!("Failed to load entries for slot {slot}: {err:?}"))?;

    if *method == OutputFormat::Display {
        if let Ok(Some(meta)) = blockstore.meta(slot) {
            if verbose_level >= 1 {
                println!("  {meta:?} is_full: {is_full}");
            } else {
                println!(
                    "  num_shreds: {}, parent_slot: {:?}, next_slots: {:?}, num_entries: {}, \
                     is_full: {}",
                    num_shreds,
                    meta.parent_slot,
                    meta.next_slots,
                    entries.len(),
                    is_full,
                );
            }
        }
    }

    if verbose_level >= 2 {
        for (entry_index, entry) in entries.into_iter().enumerate() {
            output_entry(blockstore, method, slot, entry_index, entry);
        }

        output_slot_rewards(blockstore, slot, method);
    } else if verbose_level >= 1 {
        let mut transactions = 0;
        let mut num_hashes = 0;
        let mut program_ids = HashMap::new();
        let blockhash = if let Some(entry) = entries.last() {
            entry.hash
        } else {
            Hash::default()
        };

        for entry in entries {
            transactions += entry.transactions.len();
            num_hashes += entry.num_hashes;
            for transaction in entry.transactions {
                for program_id in get_program_ids(&transaction) {
                    *program_ids.entry(*program_id).or_insert(0) += 1;
                }
            }
        }

        println!("  Transactions: {transactions}, hashes: {num_hashes}, block_hash: {blockhash}",);
        for (pubkey, count) in program_ids.iter() {
            *all_program_ids.entry(*pubkey).or_insert(0) += count;
        }
        println!("  Programs:");
        output_sorted_program_ids(program_ids);
    }
    Ok(())
}

pub fn output_ledger(
    blockstore: Blockstore,
    starting_slot: Slot,
    ending_slot: Slot,
    allow_dead_slots: bool,
    method: OutputFormat,
    num_slots: Option<Slot>,
    verbose_level: u64,
    only_rooted: bool,
) {
    let slot_iterator = blockstore
        .slot_meta_iterator(starting_slot)
        .unwrap_or_else(|err| {
            eprintln!("Failed to load entries starting from slot {starting_slot}: {err:?}");
            std::process::exit(1);
        });

    if method == OutputFormat::Json {
        stdout().write_all(b"{\"ledger\":[\n").expect("open array");
    }

    let num_slots = num_slots.unwrap_or(Slot::MAX);
    let mut num_printed = 0;
    let mut all_program_ids = HashMap::new();
    for (slot, slot_meta) in slot_iterator {
        if only_rooted && !blockstore.is_root(slot) {
            continue;
        }
        if slot > ending_slot {
            break;
        }

        match method {
            OutputFormat::Display => {
                println!("Slot {} root?: {}", slot, blockstore.is_root(slot))
            }
            OutputFormat::Json => {
                serde_json::to_writer(stdout(), &slot_meta).expect("serialize slot_meta");
                stdout().write_all(b",\n").expect("newline");
            }
            _ => unreachable!(),
        }

        if let Err(err) = output_slot(
            &blockstore,
            slot,
            allow_dead_slots,
            &method,
            verbose_level,
            &mut all_program_ids,
        ) {
            eprintln!("{err}");
        }
        num_printed += 1;
        if num_printed >= num_slots as usize {
            break;
        }
    }

    if method == OutputFormat::Json {
        stdout().write_all(b"\n]}\n").expect("close array");
    } else {
        println!("Summary of Programs:");
        output_sorted_program_ids(all_program_ids);
    }
}

pub fn output_sorted_program_ids(program_ids: HashMap<Pubkey, u64>) {
    let mut program_ids_array: Vec<_> = program_ids.into_iter().collect();
    // Sort descending by count of program id
    program_ids_array.sort_by(|a, b| b.1.cmp(&a.1));
    for (program_id, count) in program_ids_array.iter() {
        println!("{:<44}: {}", program_id.to_string(), count);
    }
}
