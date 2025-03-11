use heed::RwTxn;

use super::UpgradeIndex;
use crate::progress::Progress;
use crate::{make_enum_progress, Index, Result};

#[allow(non_camel_case_types)]
pub(super) struct Latest_V1_13_To_Latest_V1_14();

impl UpgradeIndex for Latest_V1_13_To_Latest_V1_14 {
    fn upgrade(
        &self,
        wtxn: &mut RwTxn,
        index: &Index,
        _original: (u32, u32, u32),
        progress: Progress,
    ) -> Result<bool> {
        make_enum_progress! {
            enum VectorStore {
                UpdateInternalVersions,
            }
        };

        progress.update_progress(VectorStore::UpdateInternalVersions);

        let rtxn = index.read_txn()?;
        arroy::upgrade::cosine_from_0_5_to_0_6(
            &rtxn,
            index.vector_arroy,
            &mut wtxn,
            index.vector_arroy,
        )?;

        Ok(true)
    }

    fn target_version(&self) -> (u32, u32, u32) {
        (1, 14, 0)
    }
}
