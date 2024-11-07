const { Connection, SystemProgram, TransactionInstruction, Keypair, PublicKey, Transaction, sendAndConfirmTransaction } = require('@solana/web3.js');
const BufferLayout = require('@solana/buffer-layout');
const { Buffer } = require('buffer');

let connection = new Connection("https://api.testnet.v1.sonic.game", "confirmed")
// let connection = new Connection("http://127.0.0.1:8899", "confirmed")

//read keypair from local file
const fs = require('fs');
const data = fs.readFileSync('/home/ubuntu/.config/solana/id.json');
const feePayer = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(data)));

const sonic_program_id = new PublicKey('SonicAccountMigrater11111111111111111111111');
const sonic_data_account = new PublicKey("SonicMigratedAccounts1111111111111111111112")

//get "pubkey" from https://exapi.testnet.hssn.sonic.game/hypergrid-ssn/hypergridssn/hypergrid_node
const baselayer_node_id = new PublicKey('4uhcVJyU9pJkvQyS88uRDiswHXSCkY3zQawwpjk2NsNY'); //Solana testnet
const sonic_grid1_node_id = new PublicKey('E8nY8PG8PEdzANRsv91C2w28Dbw9w3AhLqRYfn5tNv2C'); //Sonic testnet grid1

const account_to_migrate = new PublicKey("Hb2F1pTk9oNfoCpi8WDRvVNaZKedE4KHZp34uFo1rWdJ"); //demo program account
/**
 * you can get the account address from the explorer
 * Solana Testnet:
 *    https://explorer.solana.com/address/Hb2F1pTk9oNfoCpi8WDRvVNaZKedE4KHZp34uFo1rWdJ?cluster=testnet
 * 
 * Sonic Testnet:
 *    https://explorer.sonic.game/address/Hb2F1pTk9oNfoCpi8WDRvVNaZKedE4KHZp34uFo1rWdJ?cluster=custom&customUrl=https%3A%2F%2Fapi.testnet.v1.sonic.game%2F
 */


/**
 * build instruction data of migrate remote accounts
 * 
 * @param {*} node_id source node id to migrate from.
 * @param {*} accounts array of accounts to migrate.
 * @returns instruction data bytes
 */
function instruction_migrateSourceAccounts(node_id, accounts) {
  const dataLayout = BufferLayout.struct([
    BufferLayout.u32('instruction'),
    new BufferLayout.Blob(32, 'node_id'),
    BufferLayout.nu64('len'),
  ]);

  var data1 = Buffer.alloc(dataLayout.span);
  dataLayout.encode({
    instruction: 2,
    node_id: node_id.toBuffer(),
    len: accounts.length,
  }, data1);

  let buffers = [];
  buffers.push(data1);
  accounts.forEach(account => {
    buffers.push(new PublicKey(account).toBuffer());
  });
  let data = Buffer.concat(buffers);

  console.log("instruction_migrateSourceAccounts:", data.length, data);
  return data;
}

/**
 * build instruction data of disactivate accounts
 * @param {*} accounts array of accounts to disactivate.
 * @returns instruction data bytes
 */
function instruction_disactivateAccounts(accounts) {
  const dataLayout = BufferLayout.struct([
    BufferLayout.u32('instruction'),
    BufferLayout.nu64('len'),
  ]);

  var data1 = Buffer.alloc(dataLayout.span);
  dataLayout.encode({
    instruction: 1,
    len: accounts.length,
  }, data1);

  let buffers = [];
  buffers.push(data1);
  accounts.forEach(account => {
    buffers.push(new PublicKey(account).toBuffer());
  });
  let data = Buffer.concat(buffers);

  console.log("instruction_disactivateAccounts:", data.length, data);
  return data;
}

/**
 * migrate account from remote network to local network
 * @param {*} node_id source node id to migrate from.
 * @param {*} account account to migrate.
 * @returns migrated account info
 */
async function migrate_account_from_remote(node_id, account) {
  console.log('Migrating account:', account);

  const transaction = new Transaction()

  const instruction = new TransactionInstruction({
    keys: [
      { pubkey: sonic_data_account, isSigner: false, isWritable: true },
    ],
    programId: sonic_program_id,
    data: instruction_migrateSourceAccounts(node_id, [ account ]), //instruction data
  })

  transaction.add(instruction)

  const transactionSignature = await sendAndConfirmTransaction(connection, transaction, [feePayer]);
  console.log('tx signature', transactionSignature);
  let tx = await connection.getTransaction(transactionSignature);
  console.log('Transaction:', tx);

  let account_info = await get_account_info(account);
  console.log('Account info:', account_info);
  return account_info;
}

/**
 * sleep for a while
 * @param {*} time how long to delay in ms 
 * @returns promise
 */
function delay(time) {
  return new Promise(resolve => setTimeout(resolve, time));
}

/**
 * get account info with max retry 100 times
 * @param {*} account address of account
 * @returns account info
 */
async function get_account_info(account) {
  console.log('Get account info:', account);
  try { 
    for (let i = 0; i < 100; i++) {
      let account_info = await connection.getAccountInfo(account);
      if (account_info != null) {
        console.log('getAccountInfo:', i+1, account_info);
        return account_info;
      }
      await delay(100);
    }
    console.warn('Retry too many times, give up');
    return null;
  } catch (error) {
    console.error("Fail to get account:", error);
    return null;
  }
}


/**
 * disactivate remote account on local network
 * @param {*} account account to disactivate.
 * @returns transaction signature
 */
async function disactivate_retmote_account(account) {
  console.log('Disactivating account:', account);
  
  const transaction = new Transaction()
  const instruction = new TransactionInstruction({
    keys: [
      { pubkey: sonic_data_account, isSigner: false, isWritable: true },
    ],
    programId: sonic_program_id,
    data: instruction_disactivateAccounts([ account]), //instruction data
  })

  transaction.add(instruction)

  const transactionSignature = await sendAndConfirmTransaction(connection, transaction, [feePayer]);
  console.log('tx signature:', transactionSignature);

  let tx = await connection.getTransaction(transactionSignature);
  console.log('Transaction:', tx);
  return tx;
}

/**
 * execute remote program on local network
 * @param {*} program_id program id to execute.
 * @returns transaction signature
 */
async function execute_remote_program(program_id) {
  console.log('Executing program:', program_id);
  /**
  * The expected size of each greeting account.
  */
  const GREETING_SIZE = 4;

  // Create greetings account instruction
  const greetingAccountKp = new Keypair();
  const lamports = await connection.getMinimumBalanceForRentExemption(
    GREETING_SIZE
  );
  const createGreetingAccountIx = SystemProgram.createAccount({
    fromPubkey: feePayer.publicKey,
    lamports,
    newAccountPubkey: greetingAccountKp.publicKey,
    programId: program_id,
    space: GREETING_SIZE,
  });

  // Create greet instruction
  const greetIx = new TransactionInstruction({
    keys: [
      {
        pubkey: greetingAccountKp.publicKey,
        isSigner: false,
        isWritable: true,
      },
    ],
    programId: program_id,
  });


  const transaction = new Transaction()
  transaction.add(createGreetingAccountIx, greetIx);

  const transactionSignature = await sendAndConfirmTransaction(connection, transaction, [feePayer, greetingAccountKp]);
  console.log('tx signature:', transactionSignature);

  let tx = await connection.getTransaction(transactionSignature);
  console.log('Transaction:', tx);

  // Fetch the greetings account
  const greetingAccount = await connection.getAccountInfo(
    greetingAccountKp.publicKey
  );
  console.log('Greeting account: ', greetingAccountKp.publicKey, "\n", greetingAccount);
}

//Main function
async function main() {
  //get arguments from command line
  const args = process.argv.slice(2)
  // console.log('arguments:', args);
  if (args.length < 1) {
    console.error('Please provide the function to run:\n sync | interact | deactivate');
    return;
  }

  let command = args[0];

  const node_id = baselayer_node_id; //sonic_grid1_node_id
  const account = account_to_migrate; // new PublicKey("2u6tRsXfMxoWttrqAe5JXgYdoK9GEzjt4MkYutigAYMo");
  let st = new Date().getTime();
  if (command == 'sync') {
    await migrate_account_from_remote(node_id, account);
  } else if (command == 'interact') {
    await execute_remote_program(account);
  } else if (command == 'deactivate') {
    await disactivate_retmote_account(account);
  } else {
    console.error('Invalid command:', command);
  }
  let et = new Date().getTime();
  console.log('\n** Takes time **:', et-st, "ms");
}

//Run the main
main().then(() => {
  // console.log("Done")
}).catch((err) => {
  console.error(err)
});
