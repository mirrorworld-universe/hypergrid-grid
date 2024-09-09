use {
    serde::Serialize, solana_program_runtime::{declare_process_instruction, ic_msg, invoke_context::InvokeContext}, solana_sdk::{
        instruction::InstructionError, program_utils::limited_deserialize, pubkey::Pubkey, sonic_account_migrater::{
            instruction::ProgramInstruction, migrated_accounts, program, state::{MigratedAccount, MigratedAccountsState}
        }, transaction_context::BorrowedAccount
    }, std::{borrow::Borrow, collections::{HashMap, HashSet}}
};

pub const DEFAULT_COMPUTE_UNITS: u64 = 1500;

// /// The maximum number of addresses that a lookup table can hold
// pub const MAX_ADDRESSES: usize = 256;

declare_process_instruction!(Entrypoint, DEFAULT_COMPUTE_UNITS, |invoke_context| {
    let transaction_context = &invoke_context.transaction_context;
    let instruction_context = transaction_context.get_current_instruction_context()?;
    let instruction_data = instruction_context.get_instruction_data();
    match limited_deserialize(instruction_data)? {
        ProgramInstruction::MigrateRemoteAccounts{
            addresses,
        } => Processor::migrate_remote_accounts(invoke_context, addresses),
        ProgramInstruction::DeactivateRemoteAccounts{
            addresses,
        } => Processor::deactivate_remote_accounts(invoke_context, addresses),
        ProgramInstruction::MigrateSourceAccounts{
            node_id,
            addresses,
        } => Processor::migrate_source_accounts(invoke_context, node_id, addresses),
    }
});

pub struct Processor;
impl Processor {
    fn migrate_remote_accounts(
        invoke_context: &mut InvokeContext,
        addresses: Vec<Pubkey>,
    ) -> Result<(), InstructionError> {
        if addresses.is_empty() {
            ic_msg!(invoke_context, "Must provide at least one address");
            return Err(InstructionError::InvalidInstructionData);
        }

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
            if migrated_accounts::check_id(account.get_key()) && !account.is_signer() && account.is_writable() {
                ic_msg!(invoke_context, "Account {:?} is not signer or writable.", account.get_key());
                has_data_acount = true;
                data_account_index = i;
            }
        }

        if !has_data_acount {
            ic_msg!(invoke_context, "No valid accounts provided");
            return Err(InstructionError::NotEnoughAccountKeys);
        }

        let mut accouts: HashMap<Pubkey, MigratedAccount> = HashMap::new();
        let mut data_account = instruction_context.try_borrow_instruction_account(transaction_context, data_account_index)?;
        if let MigratedAccountsState::MigratedAccounts(accounts2) = data_account.get_state()? {
            accounts2.iter().for_each(|account| {
                accouts.insert(account.address, account.clone());
            });
        } else {
            ic_msg!(invoke_context, "data account is not initialized."); 
        }

        let clock = invoke_context.get_sysvar_cache().get_clock()?;
        let slot = clock.slot;

        for address in addresses.iter() {
            ic_msg!(invoke_context, "Account {:?} is migrated at slot {:?} from remote.", address, slot);
            accouts.insert(address.clone(), MigratedAccount {
                address: address.clone(),
                source: None,
                slot: slot,
            });
        }

        let state = MigratedAccountsState::MigratedAccounts(accouts.values().cloned().collect::<Vec<MigratedAccount>>());
        let serialized_data = bincode::serialize(&state).map_err(|_| InstructionError::GenericError)?;
        data_account.set_data_from_slice(&serialized_data)?;

        // let serialized_size =
        //     bincode::serialized_size(&state).map_err(|_| InstructionError::GenericError)?;
        
        // if serialized_size > data_account.capacity() as u64 {
        //     data_account.can_data_be_resized(serialized_size)
        //     return Err(InstructionError::AccountDataTooSmall);
        // }
        // data_account.set_state(&state)?;

        // let clock = invoke_context.get_sysvar_cache().get_clock()?;
        ic_msg!(invoke_context, "{} Remote Accounts are migrated at slot {}.", addresses.len(), clock.slot);

        Ok(())
    }

    fn migrate_source_accounts(
        invoke_context: &mut InvokeContext,
        node_id: Pubkey,
        addresses: Vec<Pubkey>,
    ) -> Result<(), InstructionError> {
        if addresses.is_empty() {
            ic_msg!(invoke_context, "Must provide at least one address");
            return Err(InstructionError::InvalidInstructionData);
        }

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
            if migrated_accounts::check_id(account.get_key()) && !account.is_signer() && account.is_writable() {
                ic_msg!(invoke_context, "Data account is {:?}.", account.get_key());
                has_data_acount = true;
                data_account_index = i;
            }
        }

        if !has_data_acount {
            ic_msg!(invoke_context, "No valid data account provided");
            return Err(InstructionError::NotEnoughAccountKeys);
        }

        let mut accouts: HashMap<Pubkey, MigratedAccount> = HashMap::new();
        let mut data_account = instruction_context.try_borrow_instruction_account(transaction_context, data_account_index)?;
        if let MigratedAccountsState::MigratedAccounts(accounts2) = data_account.get_state()? {
            accounts2.iter().for_each(|account: &MigratedAccount| {
                ic_msg!(invoke_context, "Accout migrated: {:?} at slot {:?}.", account.address, account.slot); 
                accouts.insert(account.address, account.clone());
            });
        } else {
            ic_msg!(invoke_context, "Data account is not initialized."); 
        }
        
        let clock = invoke_context.get_sysvar_cache().get_clock()?;
        let slot = clock.slot;

        for address in addresses.iter() {
            ic_msg!(invoke_context, "Account {:?} is migrated at slot {:?} from {:?}.", address, slot, node_id);
            accouts.insert(address.clone(), MigratedAccount {
                address: address.clone(),
                source: Some(node_id),
                slot,
            });
        }

        let state = MigratedAccountsState::MigratedAccounts(accouts.values().cloned().collect::<Vec<MigratedAccount>>());
        let serialized_data = bincode::serialize(&state).map_err(|_| InstructionError::GenericError)?;
        data_account.set_data_from_slice(&serialized_data)?;
        // data_account.set_state(&MigratedAccountsState::MigratedAccounts(accouts.values().cloned().collect::<Vec<MigratedAccount>>()))?;

        // let clock = invoke_context.get_sysvar_cache().get_clock()?;
        ic_msg!(invoke_context, "{} Remote Accounts are migrated from {} at slot {}.", addresses.len(), node_id, clock.slot);

        Ok(())
    }

    fn deactivate_remote_accounts(
        invoke_context: &mut InvokeContext,
        addresses: Vec<Pubkey>,
    ) -> Result<(), InstructionError> {
        
        if addresses.is_empty() {
            ic_msg!(invoke_context, "Must provide at least one address");
            return Err(InstructionError::InvalidInstructionData);
        }

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
            if migrated_accounts::check_id(account.get_key()) && !account.is_signer() && account.is_writable() {
                // ic_msg!(invoke_context, "Account {:?} is not signer or writable.", account.get_key());
                has_data_acount = true;
                data_account_index = i;
            }
        }

        if !has_data_acount {
            ic_msg!(invoke_context, "No valid data account provided");
            return Err(InstructionError::NotEnoughAccountKeys);
        }

        let mut accouts: HashMap<Pubkey, MigratedAccount> = HashMap::new();
        let mut data_account = instruction_context.try_borrow_instruction_account(transaction_context, data_account_index)?;
        if let MigratedAccountsState::MigratedAccounts(accounts2) = data_account.get_state()? {
            accounts2.iter().for_each(|account| {
                accouts.insert(account.address, account.clone());
            });
        } else {
            ic_msg!(invoke_context, "data account is not initialized."); 
            return Err(InstructionError::InvalidAccountData);
        }

        for address in addresses.iter() {
            ic_msg!(invoke_context, "Account {:?} is deactivated in cache.", address);
            accouts.remove(address);
        }
        
        data_account.set_state(&MigratedAccountsState::MigratedAccounts(accouts.values().cloned().collect::<Vec<MigratedAccount>>()))?;

        let clock = invoke_context.get_sysvar_cache().get_clock()?;
        ic_msg!(invoke_context, "{} Remote Accounts are already deactivated at slot {}.", addresses.len(), clock.slot);

        Ok(())
    }
}
