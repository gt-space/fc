use common::comm::VehicleState;

struct Data;
// TODO: Maybe we could make this a method of VehicleState for 
//       the common refactor?
/// Updates the vehicle state with the new data recieved from devices
pub(super) fn ingest(state: &mut VehicleState, data: Vec<Data>) {
  todo!()
}