use {
    solana_program_runtime::{declare_process_instruction, ic_msg, invoke_context::InvokeContext},
    solana_sdk::{
        instruction::InstructionError, program_utils::limited_deserialize, pubkey::Pubkey, sonic_fee_settlement::{
            data_account, instruction::{ProgramInstruction, SettlementBillParam}, state::{
                SettlementAccount, SettlementAccountType, SettlementState
            }
        }
    }, std::collections::HashMap,
};

pub const DEFAULT_COMPUTE_UNITS: u64 = 750;

// /// The maximum number of addresses that a lookup table can hold
// pub const MAX_ADDRESSES: usize = 256;

declare_process_instruction!(Entrypoint, DEFAULT_COMPUTE_UNITS, |invoke_context| {
    let transaction_context = &invoke_context.transaction_context;
    let instruction_context = transaction_context.get_current_instruction_context()?;
    let instruction_data = instruction_context.get_instruction_data();
    match limited_deserialize(instruction_data)? {
        ProgramInstruction::InitializeAccount {
            owner,
            account_type,
        } => Processor::initialize_account(invoke_context, owner, account_type),
        ProgramInstruction::SettleFeeBill {
            from_id,
            end_id,
            bills,
        } => Processor::settle_fee_bill(invoke_context, from_id, end_id, bills),
        ProgramInstruction::WithdrawFeeBill {
            address,
            amount,
        } => Processor::withdraw_fee_bill(invoke_context, address, amount),
    }
});

pub struct Processor;
impl Processor {
    fn initialize_account(
        invoke_context: &mut InvokeContext,
        owner: Pubkey,
        account_type: SettlementAccountType,
    ) -> Result<(), InstructionError> {
        let transaction_context = &invoke_context.transaction_context;
        let instruction_context = transaction_context.get_current_instruction_context()?;

        let n = instruction_context.get_number_of_instruction_accounts();
        if n < 1 {
            ic_msg!(invoke_context, "No accounts provided");
            return Err(InstructionError::NotEnoughAccountKeys);
        }

        let mut has_data_acount = false;
        let mut data_account_index: u16 = 0;
        for i in 0..n {
            let account = instruction_context.try_borrow_instruction_account(transaction_context, i)?;
            if data_account::check_id(account.get_key()) && !account.is_signer() && account.is_writable() {
                ic_msg!(invoke_context, "Data account is {:?}.", account.get_key());
                has_data_acount = true;
                data_account_index = i;
            }
        }

        if !has_data_acount {
            ic_msg!(invoke_context, "No valid data account provided");
            return Err(InstructionError::NotEnoughAccountKeys);
        }

        let mut accouts: HashMap<Pubkey, SettlementAccount> = HashMap::new();
        let mut data_account = instruction_context.try_borrow_instruction_account(transaction_context, data_account_index)?;
        if let SettlementState::FeeBillSettled(accounts2) = data_account.get_state()? {
            accounts2.iter().for_each(|account: &SettlementAccount| {
                accouts.insert(account.owner, account.clone());
            });
        } else {
            ic_msg!(invoke_context, "Data account is not initialized."); 
        }

        if let Some(account) = accouts.get(&owner) {
            ic_msg!(invoke_context, "Account {:?} is initialized.", account.owner);
            return Err(InstructionError::InvalidAccountData);
        } else {
            ic_msg!(invoke_context, "Account {:?} is not initialized.", owner);
            accouts.insert(owner, SettlementAccount {
                owner,
                account_type,
                amount: 0,
                withdrawable: 0,
                withdrawed: 0,
            });
        }

        let state = SettlementState::FeeBillSettled(accouts.values().cloned().collect::<Vec<SettlementAccount>>());
        let serialized_data = bincode::serialize(&state).map_err(|_| InstructionError::GenericError)?;
        data_account.set_data_from_slice(&serialized_data)?;
        
        Ok(())
    }
    
    fn settle_fee_bill(
        invoke_context: &mut InvokeContext,
        from_id: u64,
        end_id: u64,
        bills: Vec<SettlementBillParam>
    ) -> Result<(), InstructionError> {
        let transaction_context = &invoke_context.transaction_context;
        let instruction_context = transaction_context.get_current_instruction_context()?;

        let n = instruction_context.get_number_of_instruction_accounts();
        if n < 1 {
            ic_msg!(invoke_context, "No accounts provided");
            return Err(InstructionError::NotEnoughAccountKeys);
        }

        let mut has_data_acount = false;
        let mut data_account_index: u16 = 0;
        for i in 0..n {
            let account = instruction_context.try_borrow_instruction_account(transaction_context, i)?;
            if data_account::check_id(account.get_key()) && !account.is_signer() && account.is_writable() {
                ic_msg!(invoke_context, "Data account is {:?}.", account.get_key());
                has_data_acount = true;
                data_account_index = i;
            }
        }

        if !has_data_acount {
            ic_msg!(invoke_context, "No valid data account provided");
            return Err(InstructionError::NotEnoughAccountKeys);
        }

        let mut accouts: HashMap<Pubkey, SettlementAccount> = HashMap::new();
        let mut burn_account_id: Option<Pubkey> = None;
        let mut hssn_account_id: Option<Pubkey> = None;
        let mut sonic_account_id: Option<Pubkey> = None;
        let mut data_account = instruction_context.try_borrow_instruction_account(transaction_context, data_account_index)?;
        if let SettlementState::FeeBillSettled(accounts2) = data_account.get_state()? {
            accounts2.iter().for_each(|account: &SettlementAccount| {
                accouts.insert(account.owner, account.clone());
                match account.account_type {
                    SettlementAccountType::BurnAccount => {
                        burn_account_id = Some(account.owner);
                    },
                    SettlementAccountType::HSSNAccount => {
                        hssn_account_id = Some(account.owner);
                    },
                    SettlementAccountType::SonicGridAccount => {
                        sonic_account_id = Some(account.owner);
                    },
                    SettlementAccountType::GridAccount => {},
                }
            });
        } else {
            ic_msg!(invoke_context, "Data account is not initialized."); 
            return Err(InstructionError::InvalidAccountData);
        }

        for bill in &bills {
            ic_msg!(invoke_context, "bill: {:?} {:?}", bill.key, bill.amount);

            if let Some(burn_account_id) = burn_account_id {
                ic_msg!(invoke_context, "BurnAccount {:?} settle {:?}.", bill.key, bill.amount);
                if let Some(account) = accouts.get_mut(&burn_account_id) {
                    account.amount += bill.amount;
                    account.withdrawable += bill.amount;
                }
            }
            if let Some(hssn_account_id) = hssn_account_id {
                if let Some(account) = accouts.get_mut(&hssn_account_id) {
                    let amount = bill.amount / 4;
                    ic_msg!(invoke_context, "HSSNAccount {:?} settle {:?}.", bill.key, amount);
                    account.amount += amount;
                    account.withdrawable += amount;
                }
            }
            if let Some(sonic_account_id) = sonic_account_id {
                if let Some(account) = accouts.get_mut(&sonic_account_id) {
                    let amount = bill.amount / 4;
                    ic_msg!(invoke_context, "SonicGridAccount {:?} settle {:?}.", bill.key, amount);
                    account.amount += amount;
                    account.withdrawable += amount;
                }
            }
            if let Some(account) = accouts.get_mut(&bill.key) {
                let amount = bill.amount / 2;
                ic_msg!(invoke_context, "GridAccount {:?} settle {:?}.", bill.key, amount);
                account.amount += amount;
                account.withdrawable += amount;
            }
        };
        
        let state = SettlementState::FeeBillSettled(accouts.values().cloned().collect::<Vec<SettlementAccount>>());
        let serialized_data = bincode::serialize(&state).map_err(|_| InstructionError::GenericError)?;
        data_account.set_data_from_slice(&serialized_data)?;

        ic_msg!(invoke_context, "Sonic SettleFeeBill from {} to {}.", from_id, end_id);

        Ok(())
    }

    fn withdraw_fee_bill(invoke_context: &mut InvokeContext, address: Pubkey, amount: u64) -> Result<(), InstructionError> {
        let transaction_context = &invoke_context.transaction_context;
        let instruction_context = transaction_context.get_current_instruction_context()?;
        
        let n = instruction_context.get_number_of_instruction_accounts();
        if n < 1 {
            ic_msg!(invoke_context, "No accounts provided");
            return Err(InstructionError::NotEnoughAccountKeys);
        }

        let mut has_data_acount = false;
        let mut data_account_index: u16 = 0;
        for i in 0..n {
            let account = instruction_context.try_borrow_instruction_account(transaction_context, i)?;
            if data_account::check_id(account.get_key()) && !account.is_signer() && account.is_writable() {
                ic_msg!(invoke_context, "Data account is {:?}.", account.get_key());
                has_data_acount = true;
                data_account_index = i;
            }
        }

        if !has_data_acount {
            ic_msg!(invoke_context, "No valid data account provided");
            return Err(InstructionError::NotEnoughAccountKeys);
        }

        let mut accouts: HashMap<Pubkey, SettlementAccount> = HashMap::new();
        let mut data_account = instruction_context.try_borrow_instruction_account(transaction_context, data_account_index)?;
        if let SettlementState::FeeBillSettled(accounts2) = data_account.get_state()? {
            accounts2.iter().for_each(|account: &SettlementAccount| {
                accouts.insert(account.owner, account.clone());
            });
        } else {
            ic_msg!(invoke_context, "Data account is not initialized."); 
            return Err(InstructionError::InvalidAccountData);
        }

        if let Some(account) = accouts.get_mut(&address) {
            if amount > account.withdrawable {
                ic_msg!(invoke_context, "Account {:?} withdrawed {}.", address, amount);
                return Err(InstructionError::InvalidInstructionData);
            }
            account.withdrawed += amount;
            account.withdrawable -= amount;
            ic_msg!(invoke_context, "Account {:?} withdrawed {}.", account, amount);
        } else {
            ic_msg!(invoke_context, "data account is not initialized.");
            return Err(InstructionError::InvalidAccountData);
        }

        let state = SettlementState::FeeBillSettled(accouts.values().cloned().collect::<Vec<SettlementAccount>>());
        let serialized_data = bincode::serialize(&state).map_err(|_| InstructionError::GenericError)?;
        data_account.set_data_from_slice(&serialized_data)?;

        Ok(())
    }
}
