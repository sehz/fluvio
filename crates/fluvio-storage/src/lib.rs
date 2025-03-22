pub mod batch;
pub mod batch_header;
pub mod checkpoint;
mod error;
pub mod records;
mod index;
mod mut_records;
mod mut_index;
mod segments;
mod replica;
pub mod segment;
mod util;
mod validator;
mod file;
pub mod config;
#[cfg(feature = "iterators")]
pub mod iterators;

#[cfg(feature = "fixture")]
pub mod fixture;
mod cleaner;

pub use crate::error::StorageError;
pub use crate::records::FileRecordsSlice;
pub use crate::index::LogIndex;
pub use crate::index::OffsetPosition;
pub use crate::replica::FileReplica;

pub use inner::*;
mod inner {

    use async_trait::async_trait;
    use anyhow::Result;

    use fluvio_protocol::record::BatchRecords;
    use fluvio_protocol::link::ErrorCode;
    use fluvio_protocol::record::{Offset, ReplicaKey, Size64};
    use fluvio_protocol::record::RecordSet;
    use fluvio_future::file_slice::AsyncFileSlice;
    use fluvio_controlplane::replica::Replica;

    use crate::StorageError;

    /// Contain information about slice of Replica
    #[derive(Debug, Default)]
    pub struct ReplicaSlice {
        pub start: Offset, // start offset
        pub end: Offset,   // end offset
        pub file_slice: Option<AsyncFileSlice>,
    }

    /// some storage configuration
    pub trait ReplicaStorageConfig {
        /// update values from replica config
        fn update_from_replica(&mut self, replica: &Replica);
    }

    #[async_trait]
    pub trait ReplicaStorage: Sized {
        type ReplicaConfig: ReplicaStorageConfig;

        /// create new storage area,
        /// if there exists replica state, this should restore state
        async fn create_or_load(
            replica: &ReplicaKey,
            replica_config: Self::ReplicaConfig,
        ) -> Result<Self>;

        /// log end offset ( records that has been stored)
        fn get_leo(&self) -> Offset;

        fn get_log_start_offset(&self) -> Offset;

        /// read partition slice
        /// return hw and leo
        async fn read_partition_slice(
            &self,
            offset: Offset,
            max_len: u32,
        ) -> Result<ReplicaSlice, ErrorCode>;

        fn get_partition_size(&self) -> Size64;

        /// write record set
        async fn write_recordset<R: BatchRecords>(
            &mut self,
            records: &mut RecordSet<R>,
        ) -> Result<usize>;

        /// permanently remove
        async fn remove(&self) -> Result<(), StorageError>;
    }
}
