//! The account meta and related structs for hot accounts.

use {
    crate::{
        account_storage::meta::StoredAccountMeta,
        accounts_file::MatchAccountOwnerError,
        accounts_hash::AccountHash,
        tiered_storage::{
            byte_block,
            file::TieredStorageFile,
            footer::{AccountBlockFormat, AccountMetaFormat, TieredStorageFooter},
            index::{AccountOffset, IndexBlockFormat, IndexOffset},
            meta::{AccountMetaFlags, AccountMetaOptionalFields, TieredAccountMeta},
            mmap_utils::{get_pod, get_slice},
            owners::{OwnerOffset, OwnersBlockFormat},
            readable::TieredReadableAccount,
            TieredStorageError, TieredStorageFormat, TieredStorageResult,
        },
    },
    bytemuck::{Pod, Zeroable},
    memmap2::{Mmap, MmapOptions},
    modular_bitfield::prelude::*,
    solana_sdk::{pubkey::Pubkey, stake_history::Epoch},
    std::{fs::OpenOptions, option::Option, path::Path},
};

pub const HOT_FORMAT: TieredStorageFormat = TieredStorageFormat {
    meta_entry_size: std::mem::size_of::<HotAccountMeta>(),
    account_meta_format: AccountMetaFormat::Hot,
    owners_block_format: OwnersBlockFormat::AddressesOnly,
    index_block_format: IndexBlockFormat::AddressesThenOffsets,
    account_block_format: AccountBlockFormat::AlignedRaw,
};

/// An helper function that creates a new default footer for hot
/// accounts storage.
fn new_hot_footer() -> TieredStorageFooter {
    TieredStorageFooter {
        account_meta_format: HOT_FORMAT.account_meta_format,
        account_meta_entry_size: HOT_FORMAT.meta_entry_size as u32,
        account_block_format: HOT_FORMAT.account_block_format,
        index_block_format: HOT_FORMAT.index_block_format,
        owners_block_format: HOT_FORMAT.owners_block_format,
        ..TieredStorageFooter::default()
    }
}

/// The maximum number of padding bytes used in a hot account entry.
const MAX_HOT_PADDING: u8 = 7;

/// The maximum allowed value for the owner index of a hot account.
const MAX_HOT_OWNER_OFFSET: OwnerOffset = OwnerOffset((1 << 29) - 1);

/// The byte alignment for hot accounts.  This alignment serves duo purposes.
/// First, it allows hot accounts to be directly accessed when the underlying
/// file is mmapped.  In addition, as all hot accounts are aligned, it allows
/// each hot accounts file to handle more accounts with the same number of
/// bytes in HotAccountOffset.
pub(crate) const HOT_ACCOUNT_ALIGNMENT: usize = 8;

/// The maximum supported offset for hot accounts storage.
const MAX_HOT_ACCOUNT_OFFSET: usize = u32::MAX as usize * HOT_ACCOUNT_ALIGNMENT;

#[bitfield(bits = 32)]
#[repr(C)]
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Pod, Zeroable)]
struct HotMetaPackedFields {
    /// A hot account entry consists of the following elements:
    ///
    /// * HotAccountMeta
    /// * [u8] account data
    /// * 0-7 bytes padding
    /// * optional fields
    ///
    /// The following field records the number of padding bytes used
    /// in its hot account entry.
    padding: B3,
    /// The index to the owner of a hot account inside an AccountsFile.
    owner_offset: B29,
}

// Ensure there are no implicit padding bytes
const _: () = assert!(std::mem::size_of::<HotMetaPackedFields>() == 4);

/// The offset to access a hot account.
#[repr(C)]
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Pod, Zeroable)]
pub struct HotAccountOffset(u32);

// Ensure there are no implicit padding bytes
const _: () = assert!(std::mem::size_of::<HotAccountOffset>() == 4);

impl AccountOffset for HotAccountOffset {}

impl HotAccountOffset {
    /// Creates a new AccountOffset instance
    pub fn new(offset: usize) -> TieredStorageResult<Self> {
        if offset > MAX_HOT_ACCOUNT_OFFSET {
            return Err(TieredStorageError::OffsetOutOfBounds(
                offset,
                MAX_HOT_ACCOUNT_OFFSET,
            ));
        }

        // Hot accounts are aligned based on HOT_ACCOUNT_ALIGNMENT.
        if offset % HOT_ACCOUNT_ALIGNMENT != 0 {
            return Err(TieredStorageError::OffsetAlignmentError(
                offset,
                HOT_ACCOUNT_ALIGNMENT,
            ));
        }

        Ok(HotAccountOffset((offset / HOT_ACCOUNT_ALIGNMENT) as u32))
    }

    /// Returns the offset to the account.
    fn offset(&self) -> usize {
        self.0 as usize * HOT_ACCOUNT_ALIGNMENT
    }
}

/// The storage and in-memory representation of the metadata entry for a
/// hot account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
#[repr(C)]
pub struct HotAccountMeta {
    /// The balance of this account.
    lamports: u64,
    /// Stores important fields in a packed struct.
    packed_fields: HotMetaPackedFields,
    /// Stores boolean flags and existence of each optional field.
    flags: AccountMetaFlags,
}

// Ensure there are no implicit padding bytes
const _: () = assert!(std::mem::size_of::<HotAccountMeta>() == 8 + 4 + 4);

impl TieredAccountMeta for HotAccountMeta {
    /// Construct a HotAccountMeta instance.
    fn new() -> Self {
        HotAccountMeta {
            lamports: 0,
            packed_fields: HotMetaPackedFields::default(),
            flags: AccountMetaFlags::new(),
        }
    }

    /// A builder function that initializes lamports.
    fn with_lamports(mut self, lamports: u64) -> Self {
        self.lamports = lamports;
        self
    }

    /// A builder function that initializes the number of padding bytes
    /// for the account data associated with the current meta.
    fn with_account_data_padding(mut self, padding: u8) -> Self {
        if padding > MAX_HOT_PADDING {
            panic!("padding exceeds MAX_HOT_PADDING");
        }
        self.packed_fields.set_padding(padding);
        self
    }

    /// A builder function that initializes the owner's index.
    fn with_owner_offset(mut self, owner_offset: OwnerOffset) -> Self {
        if owner_offset > MAX_HOT_OWNER_OFFSET {
            panic!("owner_offset exceeds MAX_HOT_OWNER_OFFSET");
        }
        self.packed_fields.set_owner_offset(owner_offset.0);
        self
    }

    /// A builder function that initializes the account data size.
    fn with_account_data_size(self, _account_data_size: u64) -> Self {
        // Hot meta does not store its data size as it derives its data length
        // by comparing the offets of two consecutive account meta entries.
        self
    }

    /// A builder function that initializes the AccountMetaFlags of the current
    /// meta.
    fn with_flags(mut self, flags: &AccountMetaFlags) -> Self {
        self.flags = *flags;
        self
    }

    /// Returns the balance of the lamports associated with the account.
    fn lamports(&self) -> u64 {
        self.lamports
    }

    /// Returns the number of padding bytes for the associated account data
    fn account_data_padding(&self) -> u8 {
        self.packed_fields.padding()
    }

    /// Returns the index to the accounts' owner in the current AccountsFile.
    fn owner_offset(&self) -> OwnerOffset {
        OwnerOffset(self.packed_fields.owner_offset())
    }

    /// Returns the AccountMetaFlags of the current meta.
    fn flags(&self) -> &AccountMetaFlags {
        &self.flags
    }

    /// Always returns false as HotAccountMeta does not support multiple
    /// meta entries sharing the same account block.
    fn supports_shared_account_block() -> bool {
        false
    }

    /// Returns the epoch that this account will next owe rent by parsing
    /// the specified account block.  None will be returned if this account
    /// does not persist this optional field.
    fn rent_epoch(&self, account_block: &[u8]) -> Option<Epoch> {
        self.flags()
            .has_rent_epoch()
            .then(|| {
                let offset = self.optional_fields_offset(account_block)
                    + AccountMetaOptionalFields::rent_epoch_offset(self.flags());
                byte_block::read_pod::<Epoch>(account_block, offset).copied()
            })
            .flatten()
    }

    /// Returns the account hash by parsing the specified account block.  None
    /// will be returned if this account does not persist this optional field.
    fn account_hash<'a>(&self, account_block: &'a [u8]) -> Option<&'a AccountHash> {
        self.flags()
            .has_account_hash()
            .then(|| {
                let offset = self.optional_fields_offset(account_block)
                    + AccountMetaOptionalFields::account_hash_offset(self.flags());
                byte_block::read_pod::<AccountHash>(account_block, offset)
            })
            .flatten()
    }

    /// Returns the offset of the optional fields based on the specified account
    /// block.
    fn optional_fields_offset(&self, account_block: &[u8]) -> usize {
        account_block
            .len()
            .saturating_sub(AccountMetaOptionalFields::size_from_flags(&self.flags))
    }

    /// Returns the length of the data associated to this account based on the
    /// specified account block.
    fn account_data_size(&self, account_block: &[u8]) -> usize {
        self.optional_fields_offset(account_block)
            .saturating_sub(self.account_data_padding() as usize)
    }

    /// Returns the data associated to this account based on the specified
    /// account block.
    fn account_data<'a>(&self, account_block: &'a [u8]) -> &'a [u8] {
        &account_block[..self.account_data_size(account_block)]
    }
}

/// The reader to a hot accounts file.
#[derive(Debug)]
pub struct HotStorageReader {
    mmap: Mmap,
    footer: TieredStorageFooter,
}

impl HotStorageReader {
    /// Constructs a HotStorageReader from the specified path.
    pub fn new_from_path(path: impl AsRef<Path>) -> TieredStorageResult<Self> {
        let file = OpenOptions::new().read(true).open(path)?;
        let mmap = unsafe { MmapOptions::new().map(&file)? };
        // Here we are copying the footer, as accessing any data in a
        // TieredStorage instance requires accessing its Footer.
        // This can help improve cache locality and reduce the overhead
        // of indirection associated with memory-mapped accesses.
        let footer = *TieredStorageFooter::new_from_mmap(&mmap)?;

        Ok(Self { mmap, footer })
    }

    /// Returns the footer of the underlying tiered-storage accounts file.
    pub fn footer(&self) -> &TieredStorageFooter {
        &self.footer
    }

    /// Returns the number of files inside the underlying tiered-storage
    /// accounts file.
    pub fn num_accounts(&self) -> usize {
        self.footer.account_entry_count as usize
    }

    /// Returns the account meta located at the specified offset.
    fn get_account_meta_from_offset(
        &self,
        account_offset: HotAccountOffset,
    ) -> TieredStorageResult<&HotAccountMeta> {
        let offset = account_offset.offset();

        assert!(
            offset.saturating_add(std::mem::size_of::<HotAccountMeta>())
                <= self.footer.index_block_offset as usize,
            "reading HotAccountOffset ({}) would exceed accounts blocks offset boundary ({}).",
            offset,
            self.footer.index_block_offset,
        );
        let (meta, _) = get_pod::<HotAccountMeta>(&self.mmap, offset)?;
        Ok(meta)
    }

    /// Returns the offset to the account given the specified index.
    fn get_account_offset(
        &self,
        index_offset: IndexOffset,
    ) -> TieredStorageResult<HotAccountOffset> {
        self.footer
            .index_block_format
            .get_account_offset::<HotAccountOffset>(&self.mmap, &self.footer, index_offset)
    }

    /// Returns the address of the account associated with the specified index.
    fn get_account_address(&self, index: IndexOffset) -> TieredStorageResult<&Pubkey> {
        self.footer
            .index_block_format
            .get_account_address(&self.mmap, &self.footer, index)
    }

    /// Returns the address of the account owner given the specified
    /// owner_offset.
    fn get_owner_address(&self, owner_offset: OwnerOffset) -> TieredStorageResult<&Pubkey> {
        self.footer
            .owners_block_format
            .get_owner_address(&self.mmap, &self.footer, owner_offset)
    }

    /// Returns Ok(index_of_matching_owner) if the account owner at
    /// `account_offset` is one of the pubkeys in `owners`.
    ///
    /// Returns Err(MatchAccountOwnerError::NoMatch) if the account has 0
    /// lamports or the owner is not one of the pubkeys in `owners`.
    ///
    /// Returns Err(MatchAccountOwnerError::UnableToLoad) if there is any internal
    /// error that causes the data unable to load, including `account_offset`
    /// causes a data overrun.
    pub fn account_matches_owners(
        &self,
        account_offset: HotAccountOffset,
        owners: &[&Pubkey],
    ) -> Result<usize, MatchAccountOwnerError> {
        let account_meta = self
            .get_account_meta_from_offset(account_offset)
            .map_err(|_| MatchAccountOwnerError::UnableToLoad)?;

        if account_meta.lamports() == 0 {
            Err(MatchAccountOwnerError::NoMatch)
        } else {
            let account_owner = self
                .get_owner_address(account_meta.owner_offset())
                .map_err(|_| MatchAccountOwnerError::UnableToLoad)?;

            owners
                .iter()
                .position(|candidate| &account_owner == candidate)
                .ok_or(MatchAccountOwnerError::NoMatch)
        }
    }

    /// Returns the size of the account block based on its account offset
    /// and index offset.
    ///
    /// The account block size information is omitted in the hot accounts file
    /// as it can be derived by comparing the offset of the next hot account
    /// meta in the index block.
    fn get_account_block_size(
        &self,
        account_offset: HotAccountOffset,
        index_offset: IndexOffset,
    ) -> TieredStorageResult<usize> {
        // the offset that points to the hot account meta.
        let account_meta_offset = account_offset.offset();

        // Obtain the ending offset of the account block.  If the current
        // account is the last account, then the ending offset is the
        // index_block_offset.
        let account_block_ending_offset =
            if index_offset.0.saturating_add(1) == self.footer.account_entry_count {
                self.footer.index_block_offset as usize
            } else {
                self.get_account_offset(IndexOffset(index_offset.0.saturating_add(1)))?
                    .offset()
            };

        // With the ending offset, minus the starting offset (i.e.,
        // the account meta offset) and the HotAccountMeta size, the reminder
        // is the account block size (account data + optional fields).
        Ok(account_block_ending_offset
            .saturating_sub(account_meta_offset)
            .saturating_sub(std::mem::size_of::<HotAccountMeta>()))
    }

    /// Returns the account block that contains the account associated with
    /// the specified index given the offset to the account meta and its index.
    fn get_account_block(
        &self,
        account_offset: HotAccountOffset,
        index_offset: IndexOffset,
    ) -> TieredStorageResult<&[u8]> {
        let (data, _) = get_slice(
            &self.mmap,
            account_offset.offset() + std::mem::size_of::<HotAccountMeta>(),
            self.get_account_block_size(account_offset, index_offset)?,
        )?;

        Ok(data)
    }

    /// Returns the account located at the specified index offset.
    pub fn get_account(
        &self,
        index_offset: IndexOffset,
    ) -> TieredStorageResult<Option<(StoredAccountMeta<'_>, usize)>> {
        if index_offset.0 >= self.footer.account_entry_count {
            return Ok(None);
        }

        let account_offset = self.get_account_offset(index_offset)?;

        let meta = self.get_account_meta_from_offset(account_offset)?;
        let address = self.get_account_address(index_offset)?;
        let owner = self.get_owner_address(meta.owner_offset())?;
        let account_block = self.get_account_block(account_offset, index_offset)?;

        Ok(Some((
            StoredAccountMeta::Hot(TieredReadableAccount {
                meta,
                address,
                owner,
                index: index_offset.0 as usize,
                account_block,
            }),
            index_offset.0.saturating_add(1) as usize,
        )))
    }
}

/// The writer that creates a hot accounts file.
#[derive(Debug)]
pub struct HotStorageWriter {
    storage: TieredStorageFile,
}

impl HotStorageWriter {
    /// Create a new HotStorageWriter with the specified path.
    pub fn new(file_path: impl AsRef<Path>) -> TieredStorageResult<Self> {
        Ok(Self {
            storage: TieredStorageFile::new_writable(file_path)?,
        })
    }
}

#[cfg(test)]
pub mod tests {
    use {
        super::*,
        crate::tiered_storage::{
            byte_block::ByteBlockWriter,
            file::TieredStorageFile,
            footer::{AccountBlockFormat, AccountMetaFormat, TieredStorageFooter, FOOTER_SIZE},
            hot::{HotAccountMeta, HotStorageReader},
            index::{AccountIndexWriterEntry, IndexBlockFormat, IndexOffset},
            meta::{AccountMetaFlags, AccountMetaOptionalFields, TieredAccountMeta},
            owners::OwnersBlockFormat,
        },
        assert_matches::assert_matches,
        memoffset::offset_of,
        rand::{seq::SliceRandom, Rng},
        solana_sdk::{account::ReadableAccount, hash::Hash, pubkey::Pubkey, stake_history::Epoch},
        tempfile::TempDir,
    };

    #[test]
    fn test_hot_account_meta_layout() {
        assert_eq!(offset_of!(HotAccountMeta, lamports), 0x00);
        assert_eq!(offset_of!(HotAccountMeta, packed_fields), 0x08);
        assert_eq!(offset_of!(HotAccountMeta, flags), 0x0C);
        assert_eq!(std::mem::size_of::<HotAccountMeta>(), 16);
    }

    #[test]
    fn test_packed_fields() {
        const TEST_PADDING: u8 = 7;
        const TEST_OWNER_OFFSET: u32 = 0x1fff_ef98;
        let mut packed_fields = HotMetaPackedFields::default();
        packed_fields.set_padding(TEST_PADDING);
        packed_fields.set_owner_offset(TEST_OWNER_OFFSET);
        assert_eq!(packed_fields.padding(), TEST_PADDING);
        assert_eq!(packed_fields.owner_offset(), TEST_OWNER_OFFSET);
    }

    #[test]
    fn test_packed_fields_max_values() {
        let mut packed_fields = HotMetaPackedFields::default();
        packed_fields.set_padding(MAX_HOT_PADDING);
        packed_fields.set_owner_offset(MAX_HOT_OWNER_OFFSET.0);
        assert_eq!(packed_fields.padding(), MAX_HOT_PADDING);
        assert_eq!(packed_fields.owner_offset(), MAX_HOT_OWNER_OFFSET.0);
    }

    #[test]
    fn test_hot_meta_max_values() {
        let meta = HotAccountMeta::new()
            .with_account_data_padding(MAX_HOT_PADDING)
            .with_owner_offset(MAX_HOT_OWNER_OFFSET);

        assert_eq!(meta.account_data_padding(), MAX_HOT_PADDING);
        assert_eq!(meta.owner_offset(), MAX_HOT_OWNER_OFFSET);
    }

    #[test]
    fn test_max_hot_account_offset() {
        assert_matches!(HotAccountOffset::new(0), Ok(_));
        assert_matches!(HotAccountOffset::new(MAX_HOT_ACCOUNT_OFFSET), Ok(_));
    }

    #[test]
    fn test_max_hot_account_offset_out_of_bounds() {
        assert_matches!(
            HotAccountOffset::new(MAX_HOT_ACCOUNT_OFFSET + HOT_ACCOUNT_ALIGNMENT),
            Err(TieredStorageError::OffsetOutOfBounds(_, _))
        );
    }

    #[test]
    fn test_max_hot_account_offset_alignment_error() {
        assert_matches!(
            HotAccountOffset::new(HOT_ACCOUNT_ALIGNMENT - 1),
            Err(TieredStorageError::OffsetAlignmentError(_, _))
        );
    }

    #[test]
    #[should_panic(expected = "padding exceeds MAX_HOT_PADDING")]
    fn test_hot_meta_padding_exceeds_limit() {
        HotAccountMeta::new().with_account_data_padding(MAX_HOT_PADDING + 1);
    }

    #[test]
    #[should_panic(expected = "owner_offset exceeds MAX_HOT_OWNER_OFFSET")]
    fn test_hot_meta_owner_offset_exceeds_limit() {
        HotAccountMeta::new().with_owner_offset(OwnerOffset(MAX_HOT_OWNER_OFFSET.0 + 1));
    }

    #[test]
    fn test_hot_account_meta() {
        const TEST_LAMPORTS: u64 = 2314232137;
        const TEST_PADDING: u8 = 5;
        const TEST_OWNER_OFFSET: OwnerOffset = OwnerOffset(0x1fef_1234);
        const TEST_RENT_EPOCH: Epoch = 7;

        let optional_fields = AccountMetaOptionalFields {
            rent_epoch: Some(TEST_RENT_EPOCH),
            account_hash: Some(AccountHash(Hash::new_unique())),
        };

        let flags = AccountMetaFlags::new_from(&optional_fields);
        let meta = HotAccountMeta::new()
            .with_lamports(TEST_LAMPORTS)
            .with_account_data_padding(TEST_PADDING)
            .with_owner_offset(TEST_OWNER_OFFSET)
            .with_flags(&flags);

        assert_eq!(meta.lamports(), TEST_LAMPORTS);
        assert_eq!(meta.account_data_padding(), TEST_PADDING);
        assert_eq!(meta.owner_offset(), TEST_OWNER_OFFSET);
        assert_eq!(*meta.flags(), flags);
    }

    #[test]
    fn test_hot_account_meta_full() {
        let account_data = [11u8; 83];
        let padding = [0u8; 5];

        const TEST_LAMPORT: u64 = 2314232137;
        const OWNER_OFFSET: u32 = 0x1fef_1234;
        const TEST_RENT_EPOCH: Epoch = 7;

        let optional_fields = AccountMetaOptionalFields {
            rent_epoch: Some(TEST_RENT_EPOCH),
            account_hash: Some(AccountHash(Hash::new_unique())),
        };

        let flags = AccountMetaFlags::new_from(&optional_fields);
        let expected_meta = HotAccountMeta::new()
            .with_lamports(TEST_LAMPORT)
            .with_account_data_padding(padding.len().try_into().unwrap())
            .with_owner_offset(OwnerOffset(OWNER_OFFSET))
            .with_flags(&flags);

        let mut writer = ByteBlockWriter::new(AccountBlockFormat::AlignedRaw);
        writer.write_pod(&expected_meta).unwrap();
        // SAFETY: These values are POD, so they are safe to write.
        unsafe {
            writer.write_type(&account_data).unwrap();
            writer.write_type(&padding).unwrap();
        }
        writer.write_optional_fields(&optional_fields).unwrap();
        let buffer = writer.finish().unwrap();

        let meta = byte_block::read_pod::<HotAccountMeta>(&buffer, 0).unwrap();
        assert_eq!(expected_meta, *meta);
        assert!(meta.flags().has_rent_epoch());
        assert!(meta.flags().has_account_hash());
        assert_eq!(meta.account_data_padding() as usize, padding.len());

        let account_block = &buffer[std::mem::size_of::<HotAccountMeta>()..];
        assert_eq!(
            meta.optional_fields_offset(account_block),
            account_block
                .len()
                .saturating_sub(AccountMetaOptionalFields::size_from_flags(&flags))
        );
        assert_eq!(account_data.len(), meta.account_data_size(account_block));
        assert_eq!(account_data, meta.account_data(account_block));
        assert_eq!(meta.rent_epoch(account_block), optional_fields.rent_epoch);
        assert_eq!(
            *(meta.account_hash(account_block).unwrap()),
            optional_fields.account_hash.unwrap()
        );
    }

    #[test]
    fn test_hot_storage_footer() {
        // Generate a new temp path that is guaranteed to NOT already have a file.
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test_hot_storage_footer");
        let expected_footer = TieredStorageFooter {
            account_meta_format: AccountMetaFormat::Hot,
            owners_block_format: OwnersBlockFormat::AddressesOnly,
            index_block_format: IndexBlockFormat::AddressesThenOffsets,
            account_block_format: AccountBlockFormat::AlignedRaw,
            account_entry_count: 300,
            account_meta_entry_size: 16,
            account_block_size: 4096,
            owner_count: 250,
            owner_entry_size: 32,
            index_block_offset: 1069600,
            owners_block_offset: 1081200,
            hash: Hash::new_unique(),
            min_account_address: Pubkey::default(),
            max_account_address: Pubkey::new_unique(),
            footer_size: FOOTER_SIZE as u64,
            format_version: 1,
        };

        {
            let file = TieredStorageFile::new_writable(&path).unwrap();
            expected_footer.write_footer_block(&file).unwrap();
        }

        // Reopen the same storage, and expect the persisted footer is
        // the same as what we have written.
        {
            let hot_storage = HotStorageReader::new_from_path(&path).unwrap();
            assert_eq!(expected_footer, *hot_storage.footer());
        }
    }

    #[test]
    fn test_hot_storage_get_account_meta_from_offset() {
        // Generate a new temp path that is guaranteed to NOT already have a file.
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test_hot_storage_footer");

        const NUM_ACCOUNTS: u32 = 10;
        let mut rng = rand::thread_rng();

        let hot_account_metas: Vec<_> = (0..NUM_ACCOUNTS)
            .map(|_| {
                HotAccountMeta::new()
                    .with_lamports(rng.gen_range(0..u64::MAX))
                    .with_owner_offset(OwnerOffset(rng.gen_range(0..NUM_ACCOUNTS)))
            })
            .collect();

        let account_offsets: Vec<_>;
        let mut footer = TieredStorageFooter {
            account_meta_format: AccountMetaFormat::Hot,
            account_entry_count: NUM_ACCOUNTS,
            ..TieredStorageFooter::default()
        };
        {
            let file = TieredStorageFile::new_writable(&path).unwrap();
            let mut current_offset = 0;

            account_offsets = hot_account_metas
                .iter()
                .map(|meta| {
                    let prev_offset = current_offset;
                    current_offset += file.write_pod(meta).unwrap();
                    HotAccountOffset::new(prev_offset).unwrap()
                })
                .collect();
            // while the test only focuses on account metas, writing a footer
            // here is necessary to make it a valid tiered-storage file.
            footer.index_block_offset = current_offset as u64;
            footer.write_footer_block(&file).unwrap();
        }

        let hot_storage = HotStorageReader::new_from_path(&path).unwrap();

        for (offset, expected_meta) in account_offsets.iter().zip(hot_account_metas.iter()) {
            let meta = hot_storage.get_account_meta_from_offset(*offset).unwrap();
            assert_eq!(meta, expected_meta);
        }

        assert_eq!(&footer, hot_storage.footer());
    }

    #[test]
    #[should_panic(expected = "would exceed accounts blocks offset boundary")]
    fn test_get_acount_meta_from_offset_out_of_bounds() {
        // Generate a new temp path that is guaranteed to NOT already have a file.
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir
            .path()
            .join("test_get_acount_meta_from_offset_out_of_bounds");

        let footer = TieredStorageFooter {
            account_meta_format: AccountMetaFormat::Hot,
            index_block_offset: 160,
            ..TieredStorageFooter::default()
        };

        {
            let file = TieredStorageFile::new_writable(&path).unwrap();
            footer.write_footer_block(&file).unwrap();
        }

        let hot_storage = HotStorageReader::new_from_path(&path).unwrap();
        let offset = HotAccountOffset::new(footer.index_block_offset as usize).unwrap();
        // Read from index_block_offset, which offset doesn't belong to
        // account blocks.  Expect assert failure here
        hot_storage.get_account_meta_from_offset(offset).unwrap();
    }

    #[test]
    fn test_hot_storage_get_account_offset_and_address() {
        // Generate a new temp path that is guaranteed to NOT already have a file.
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir
            .path()
            .join("test_hot_storage_get_account_offset_and_address");
        const NUM_ACCOUNTS: u32 = 10;
        let mut rng = rand::thread_rng();

        let addresses: Vec<_> = std::iter::repeat_with(Pubkey::new_unique)
            .take(NUM_ACCOUNTS as usize)
            .collect();

        let index_writer_entries: Vec<_> = addresses
            .iter()
            .map(|address| AccountIndexWriterEntry {
                address,
                offset: HotAccountOffset::new(
                    rng.gen_range(0..u32::MAX) as usize * HOT_ACCOUNT_ALIGNMENT,
                )
                .unwrap(),
            })
            .collect();

        let mut footer = TieredStorageFooter {
            account_meta_format: AccountMetaFormat::Hot,
            account_entry_count: NUM_ACCOUNTS,
            // Set index_block_offset to 0 as we didn't write any account
            // meta/data in this test
            index_block_offset: 0,
            ..TieredStorageFooter::default()
        };
        {
            let file = TieredStorageFile::new_writable(&path).unwrap();

            let cursor = footer
                .index_block_format
                .write_index_block(&file, &index_writer_entries)
                .unwrap();
            footer.owners_block_offset = cursor as u64;
            footer.write_footer_block(&file).unwrap();
        }

        let hot_storage = HotStorageReader::new_from_path(&path).unwrap();
        for (i, index_writer_entry) in index_writer_entries.iter().enumerate() {
            let account_offset = hot_storage
                .get_account_offset(IndexOffset(i as u32))
                .unwrap();
            assert_eq!(account_offset, index_writer_entry.offset);

            let account_address = hot_storage
                .get_account_address(IndexOffset(i as u32))
                .unwrap();
            assert_eq!(account_address, index_writer_entry.address);
        }
    }

    #[test]
    fn test_hot_storage_get_owner_address() {
        // Generate a new temp path that is guaranteed to NOT already have a file.
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test_hot_storage_get_owner_address");
        const NUM_OWNERS: usize = 10;

        let addresses: Vec<_> = std::iter::repeat_with(Pubkey::new_unique)
            .take(NUM_OWNERS)
            .collect();

        let footer = TieredStorageFooter {
            account_meta_format: AccountMetaFormat::Hot,
            // meta/data nor index block in this test
            owners_block_offset: 0,
            ..TieredStorageFooter::default()
        };

        {
            let file = TieredStorageFile::new_writable(&path).unwrap();

            footer
                .owners_block_format
                .write_owners_block(&file, &addresses)
                .unwrap();

            // while the test only focuses on account metas, writing a footer
            // here is necessary to make it a valid tiered-storage file.
            footer.write_footer_block(&file).unwrap();
        }

        let hot_storage = HotStorageReader::new_from_path(&path).unwrap();
        for (i, address) in addresses.iter().enumerate() {
            assert_eq!(
                hot_storage
                    .get_owner_address(OwnerOffset(i as u32))
                    .unwrap(),
                address,
            );
        }
    }

    #[test]
    fn test_account_matches_owners() {
        // Generate a new temp path that is guaranteed to NOT already have a file.
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test_hot_storage_get_owner_address");
        const NUM_OWNERS: u32 = 10;

        let owner_addresses: Vec<_> = std::iter::repeat_with(Pubkey::new_unique)
            .take(NUM_OWNERS as usize)
            .collect();

        const NUM_ACCOUNTS: u32 = 30;
        let mut rng = rand::thread_rng();

        let hot_account_metas: Vec<_> = std::iter::repeat_with({
            || {
                HotAccountMeta::new()
                    .with_lamports(rng.gen_range(1..u64::MAX))
                    .with_owner_offset(OwnerOffset(rng.gen_range(0..NUM_OWNERS)))
            }
        })
        .take(NUM_ACCOUNTS as usize)
        .collect();
        let mut footer = TieredStorageFooter {
            account_meta_format: AccountMetaFormat::Hot,
            account_entry_count: NUM_ACCOUNTS,
            owner_count: NUM_OWNERS,
            ..TieredStorageFooter::default()
        };
        let account_offsets: Vec<_>;

        {
            let file = TieredStorageFile::new_writable(&path).unwrap();
            let mut current_offset = 0;

            account_offsets = hot_account_metas
                .iter()
                .map(|meta| {
                    let prev_offset = current_offset;
                    current_offset += file.write_pod(meta).unwrap();
                    HotAccountOffset::new(prev_offset).unwrap()
                })
                .collect();
            footer.index_block_offset = current_offset as u64;
            // Typically, the owners block is stored after index block, but
            // since we don't write index block in this test, so we have
            // the owners_block_offset set to the end of the accounts blocks.
            footer.owners_block_offset = footer.index_block_offset;

            footer
                .owners_block_format
                .write_owners_block(&file, &owner_addresses)
                .unwrap();

            // while the test only focuses on account metas, writing a footer
            // here is necessary to make it a valid tiered-storage file.
            footer.write_footer_block(&file).unwrap();
        }

        let hot_storage = HotStorageReader::new_from_path(&path).unwrap();

        // First, verify whether we can find the expected owners.
        let mut owner_candidates: Vec<_> = owner_addresses.iter().collect();
        owner_candidates.shuffle(&mut rng);

        for (account_offset, account_meta) in account_offsets.iter().zip(hot_account_metas.iter()) {
            let index = hot_storage
                .account_matches_owners(*account_offset, &owner_candidates)
                .unwrap();
            assert_eq!(
                owner_candidates[index],
                &owner_addresses[account_meta.owner_offset().0 as usize]
            );
        }

        // Second, verify the MatchAccountOwnerError::NoMatch case
        const NUM_UNMATCHED_OWNERS: usize = 20;
        let unmatched_owners: Vec<_> = std::iter::repeat_with(Pubkey::new_unique)
            .take(NUM_UNMATCHED_OWNERS)
            .collect();
        let unmatched_candidates: Vec<_> = unmatched_owners.iter().collect();

        for account_offset in account_offsets.iter() {
            assert_eq!(
                hot_storage.account_matches_owners(*account_offset, &unmatched_candidates),
                Err(MatchAccountOwnerError::NoMatch)
            );
        }

        // Thirdly, we mixed two candidates and make sure we still find the
        // matched owner.
        owner_candidates.extend(unmatched_candidates);
        owner_candidates.shuffle(&mut rng);

        for (account_offset, account_meta) in account_offsets.iter().zip(hot_account_metas.iter()) {
            let index = hot_storage
                .account_matches_owners(*account_offset, &owner_candidates)
                .unwrap();
            assert_eq!(
                owner_candidates[index],
                &owner_addresses[account_meta.owner_offset().0 as usize]
            );
        }
    }

    // returns the required number of padding
    fn padding_bytes(data_len: usize) -> u8 {
        ((HOT_ACCOUNT_ALIGNMENT - (data_len % HOT_ACCOUNT_ALIGNMENT)) % HOT_ACCOUNT_ALIGNMENT) as u8
    }

    #[test]
    fn test_hot_storage_get_account() {
        // Generate a new temp path that is guaranteed to NOT already have a file.
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test_hot_storage_get_account");

        let mut rng = rand::thread_rng();

        // create owners
        const NUM_OWNERS: usize = 10;
        let owners: Vec<_> = std::iter::repeat_with(Pubkey::new_unique)
            .take(NUM_OWNERS)
            .collect();

        // create account data
        const NUM_ACCOUNTS: usize = 20;
        let account_datas: Vec<_> = (0..NUM_ACCOUNTS)
            .map(|i| vec![i as u8; rng.gen_range(0..4096)])
            .collect();

        // create account metas that link to its data and owner
        let account_metas: Vec<_> = (0..NUM_ACCOUNTS)
            .map(|i| {
                HotAccountMeta::new()
                    .with_lamports(rng.gen_range(0..u64::MAX))
                    .with_owner_offset(OwnerOffset(rng.gen_range(0..NUM_OWNERS) as u32))
                    .with_account_data_padding(padding_bytes(account_datas[i].len()))
            })
            .collect();

        // create account addresses
        let addresses: Vec<_> = std::iter::repeat_with(Pubkey::new_unique)
            .take(NUM_ACCOUNTS)
            .collect();

        let mut footer = TieredStorageFooter {
            account_meta_format: AccountMetaFormat::Hot,
            account_entry_count: NUM_ACCOUNTS as u32,
            owner_count: NUM_OWNERS as u32,
            ..TieredStorageFooter::default()
        };

        {
            let file = TieredStorageFile::new_writable(&path).unwrap();
            let mut current_offset = 0;

            // write accounts blocks
            let padding_buffer = [0u8; HOT_ACCOUNT_ALIGNMENT];
            let index_writer_entries: Vec<_> = account_metas
                .iter()
                .zip(account_datas.iter())
                .zip(addresses.iter())
                .map(|((meta, data), address)| {
                    let prev_offset = current_offset;
                    current_offset += file.write_pod(meta).unwrap();
                    current_offset += file.write_bytes(data).unwrap();
                    current_offset += file
                        .write_bytes(&padding_buffer[0..padding_bytes(data.len()) as usize])
                        .unwrap();
                    AccountIndexWriterEntry {
                        address,
                        offset: HotAccountOffset::new(prev_offset).unwrap(),
                    }
                })
                .collect();

            // write index blocks
            footer.index_block_offset = current_offset as u64;
            current_offset += footer
                .index_block_format
                .write_index_block(&file, &index_writer_entries)
                .unwrap();

            // write owners block
            footer.owners_block_offset = current_offset as u64;
            footer
                .owners_block_format
                .write_owners_block(&file, &owners)
                .unwrap();

            footer.write_footer_block(&file).unwrap();
        }

        let hot_storage = HotStorageReader::new_from_path(&path).unwrap();

        for i in 0..NUM_ACCOUNTS {
            let (stored_meta, next) = hot_storage
                .get_account(IndexOffset(i as u32))
                .unwrap()
                .unwrap();
            assert_eq!(stored_meta.lamports(), account_metas[i].lamports());
            assert_eq!(stored_meta.data().len(), account_datas[i].len());
            assert_eq!(stored_meta.data(), account_datas[i]);
            assert_eq!(
                *stored_meta.owner(),
                owners[account_metas[i].owner_offset().0 as usize]
            );
            assert_eq!(*stored_meta.pubkey(), addresses[i]);

            assert_eq!(i + 1, next);
        }
        // Make sure it returns None on NUM_ACCOUNTS to allow termination on
        // while loop in actual accounts-db read case.
        assert_matches!(
            hot_storage.get_account(IndexOffset(NUM_ACCOUNTS as u32)),
            Ok(None)
        );
    }

    #[test]
    fn test_hot_storage_writer_twice_on_same_path() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir
            .path()
            .join("test_hot_storage_writer_twice_on_same_path");

        // Expect the first returns Ok
        assert_matches!(HotStorageWriter::new(&path), Ok(_));
        // Expect the second call on the same path returns Err, as the
        // HotStorageWriter only writes once.
        assert_matches!(HotStorageWriter::new(&path), Err(_));
    }
}
