use common::comm::{flight::DataMessage, VehicleState};
use crate::MMAP_GRACE_PERIOD;
use mmap_sync::synchronizer::{Synchronizer, SynchronizerError};

pub(crate) fn sync_sequences(mut sync: Synchronizer, state: &VehicleState) -> Result<(usize, bool), SynchronizerError> {
  sync.write(state, MMAP_GRACE_PERIOD)
}

/// Updates the vehicle state with the new data recieved (flight 1.0 code can be reused)
pub(crate) fn ingest(state: &mut VehicleState, data: Vec<DataMessage>) {
  todo!()
}