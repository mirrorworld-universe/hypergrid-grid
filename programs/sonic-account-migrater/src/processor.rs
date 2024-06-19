use {
    solana_program_runtime::{declare_process_instruction, ic_msg, invoke_context::InvokeContext},
    solana_sdk::{
        sonic_account_migrater::{
            instruction::ProgramInstruction,
            program::check_id,
        },
        instruction::InstructionError,
        program_utils::limited_deserialize,
    },
};

pub const DEFAULT_COMPUTE_UNITS: u64 = 750;

// /// The maximum number of addresses that a lookup table can hold
// pub const MAX_ADDRESSES: usize = 256;

declare_process_instruction!(Entrypoint, DEFAULT_COMPUTE_UNITS, |invoke_context| {
    let transaction_context = &invoke_context.transaction_context;
    let instruction_context = transaction_context.get_current_instruction_context()?;
    let instruction_data = instruction_context.get_instruction_data();
    match limited_deserialize(instruction_data)? {
        ProgramInstruction::MigrateRemoteAccounts => Processor::migrate_remote_accounts(invoke_context),
        ProgramInstruction::DeactivateRemoteAccounts => Processor::deactivate_remote_accounts(invoke_context),
    }
});

pub struct Processor;
impl Processor {
    fn migrate_remote_accounts(
        invoke_context: &mut InvokeContext,
    ) -> Result<(), InstructionError> {
        let transaction_context = &invoke_context.transaction_context;
        let instruction_context = transaction_context.get_current_instruction_context()?;

        let n = instruction_context.get_number_of_instruction_accounts();
        if n < 1 {
            ic_msg!(invoke_context, "No accounts provided");
            return Err(InstructionError::NotEnoughAccountKeys);
        }

        let mut addresses_len = 0;
        for i in 0..n {
            let account = instruction_context.try_borrow_instruction_account(transaction_context, i)?;
            let key = *account.get_key();
            if !account.is_signer() && !account.is_writable() && !check_id(&key) {
                ic_msg!(invoke_context, "Account {:?} is migrated from remote.", key);
                addresses_len += 1;
            }
        }

        let clock = invoke_context.get_sysvar_cache().get_clock()?;
        ic_msg!(invoke_context, "{} Remote Accounts are migrated at slot {}.", addresses_len, clock.slot);

        Ok(())
    }

    fn deactivate_remote_accounts(invoke_context: &mut InvokeContext) -> Result<(), InstructionError> {
        let transaction_context = &invoke_context.transaction_context;
        let instruction_context = transaction_context.get_current_instruction_context()?;
        
        let n = instruction_context.get_number_of_instruction_accounts();
        if n < 1 {
            ic_msg!(invoke_context, "No accounts provided");
            return Err(InstructionError::NotEnoughAccountKeys);
        }

        let mut addresses_len = 0;
        for i in 0..n {
            let account = instruction_context.try_borrow_instruction_account(transaction_context, i)?;
            let key = *account.get_key();
            if !account.is_signer() && !account.is_writable() && !check_id(&key) {
                ic_msg!(invoke_context, "Account {:?} is deactivated in cache.", key);
                addresses_len += 1;
            }
        }

        let clock = invoke_context.get_sysvar_cache().get_clock()?;
        ic_msg!(invoke_context, "{} Remote Accounts are already deactivated at slot {}.", addresses_len, clock.slot);

        Ok(())
    }
}
