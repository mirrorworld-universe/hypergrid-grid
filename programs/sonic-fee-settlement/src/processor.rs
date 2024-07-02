use {
    solana_program_runtime::{declare_process_instruction, ic_msg, invoke_context::InvokeContext},
    solana_sdk::{
        instruction::InstructionError, program_utils::limited_deserialize, pubkey::Pubkey, signer, sonic_fee_settlement::{
            instruction::{ProgramInstruction, SettlementBillParam},
            program::check_id,
            state::{
                SettlementAccount, SettlementState, SettlementAccountType
            },
        }
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


        let mut data_acount = instruction_context.try_borrow_instruction_account(transaction_context, 0)?;
        if !check_id(&(*data_acount.get_owner())) {
            return Err(InstructionError::InvalidAccountOwner);
        }

        ic_msg!(invoke_context, "Start Initializing Account {:?} {:?}.", data_acount.get_key(), owner);

        if let SettlementState::Uninitialized = data_acount.get_state()? {
            let state = SettlementState::FeeBillSettled(SettlementAccount {
                owner,
                account_type,
                amount: 0,
                withdrawable: 0,
                withdrawed: 0,
            });

            data_acount.set_state(&state)?;
            ic_msg!(invoke_context, "Initialized Account: {:?} {:?}.", data_acount.get_key(), owner);
        } else {
            ic_msg!(invoke_context, "data account is initialized.");
            return Err(InstructionError::InvalidAccountData);
        }
        
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

        // deal with bills
        //convert bills to map
        let mut bill_map: std::collections::HashMap<Pubkey, u64> = std::collections::HashMap::new();
        bills.iter().for_each(|bill| {
            ic_msg!(invoke_context, "bill: {:?} {:?}", bill.key, bill.amount);
            bill_map.insert(bill.key, bill.amount);
        });

        for i in 0..n {
            let account = instruction_context.try_borrow_instruction_account(transaction_context, i)?;
            let key = *account.get_key();
            if !account.is_signer() && account.is_writable() && check_id(account.get_owner()){
                if let SettlementState::FeeBillSettled(mut state) = account.get_state()? {
                    ic_msg!(invoke_context, "data account {} is initialized.", key);
                    if let Some(amount) = bill_map.remove(&state.owner) {
                        let mut amount = amount;
                        match state.account_type {
                            SettlementAccountType::BurnAccount => {
                                amount = amount;
                                ic_msg!(invoke_context, "BurnAccount {} settle {}.", key, amount);
                            },
                            SettlementAccountType::HSSNAccount => {
                                amount = amount / 4;
                                ic_msg!(invoke_context, "HSSNAccount {} settle {}.", key, amount);
                            },
                            SettlementAccountType::SonicGridAccount => {
                                amount = amount / 4;
                                ic_msg!(invoke_context, "SonicGridAccount {} settle {}.", key, amount);
                            },
                            SettlementAccountType::GridAccount => {
                                amount = amount / 2;
                                ic_msg!(invoke_context, "GridAccount {} settle {}.", key, amount);
                            },
                        }
                        state.amount += amount;
                        state.withdrawable += amount;
                    }
                } else {
                    ic_msg!(invoke_context, "data account {} is not initialized.", key);
                    return Err(InstructionError::InvalidAccountData);
                }
            }
        }

        if bill_map.len() > 0 {
            ic_msg!(invoke_context, "Some bills are not settled.");
            return Err(InstructionError::NotEnoughAccountKeys);
        }
        
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

        let data_acount = instruction_context.try_borrow_instruction_account(transaction_context, 0)?;
        if !check_id(&(*data_acount.get_owner())) {
            return Err(InstructionError::InvalidAccountOwner);
        }

        // let mut signer: Option<Pubkey> = None;
        // for i in 0..n {
        //     let account = instruction_context.try_borrow_instruction_account(transaction_context, i)?;
        //     let key = *account.get_key();
        //     if account.is_signer() {
        //         signer = Some(key);
        //         break;
        //     }
        // }
        // let address = signer.unwrap();

        if let SettlementState::FeeBillSettled(mut state) = data_acount.get_state()? {
            ic_msg!(invoke_context, "data account is initialized.");
            if address.eq(&state.owner) {
                if amount > state.withdrawable {
                    ic_msg!(invoke_context, "Account {:?} withdrawed {}.", address, amount);
                    return Err(InstructionError::InvalidInstructionData);
                }
                state.withdrawed += amount;
                state.withdrawable -= amount;
                ic_msg!(invoke_context, "Account {:?} withdrawed {}.", address, amount);
            } else {
                ic_msg!(invoke_context, "Account {:?} is not the owner.", address);
                return Err(InstructionError::InvalidAccountData);
            }
        } else {
            ic_msg!(invoke_context, "data account is not initialized.");
            return Err(InstructionError::InvalidAccountData);
        }

        Ok(())
    }
}
