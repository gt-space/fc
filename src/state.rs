use common::comm::{ahrs::DataPoint, flight::DataMessage, VehicleState};
use common::comm::ahrs;
use crate::config::{BoardId};


/// Updates the vehicle state with the new data recieved (flight 1.0 code can be reused)
trait DataHandler{
  fn update_state(&self, state: &mut VehicleState, data: DataMessage);
}

struct SamHandler;
struct BmsHandler;
struct AhrsHandler;

impl DataHandler for SamHandler {
  fn update_state(&self, state: &mut VehicleState, data: DataMessage) {
      //need help here 
  }
}

impl DataHandler for BmsHandler {
  fn update_state(&self, state: &mut VehicleState, data: DataMessage) {
    if let DataMessage::Bms(board_id, datapoint) = data {
        state.bms = datapoint.state;  //based on exisitng code
      
    }
  }
}

impl DataHandler for AhrsHandler {
  fn update_state(&self, state: &mut VehicleState, data: DataMessage) {
    if let DataMessage::Ahrs(board_id, datapoints) = data {
      let datapoints_slice: &[ahrs::DataPoint]  = &datapoints;
      for datapoint in datapoints_slice {
        state.ahrs = datapoint.state;  //based on exisitng code
      }
    }
  }
}

pub(super) fn ingest(state: &mut VehicleState, data: Vec<DataMessage>) {
  for message in data {
    match &message {
      DataMessage::Ahrs(id, datapoints) => {
        AhrsHandler.update_state(state, message);
      }
      DataMessage::Bms(id, datapoint) => {
        BmsHandler.update_state(state, message);
      }
      DataMessage::Sam(id, datapoints) => {
        SamHandler.update_state(state, message);
      }
      DataMessage::Identity(id) => {
        //add it to the mappings?
      }
      DataMessage::FlightHeartbeat => {
        //handle heartbeats as needed
      }
    };
  }
}