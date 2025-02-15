use common::comm::sam::SamControlMessage;
use common::comm::{ahrs::DataPoint, flight::DataMessage, VehicleState};
use common::comm::ahrs;
use std::{collections::HashMap};
use crate::config::{BoardId};


/// Updates the vehicle state with the new data recieved (flight 1.0 code can be reused)
trait DataHandler{
  fn update_state(&self, state: &mut VehicleState);
}





impl <'a> DataHandler for DataMessage <'a> {
  fn update_state(&self, state: &mut VehicleState) {
    match self {
      DataMessage::Ahrs(id, datapoints) => {
          let datapoints_slice: &[ahrs::DataPoint] = datapoints;
          for datapoint in datapoints_slice {
              state.ahrs = datapoint.state;
          }
      }
      DataMessage::Bms(id, datapoint) => {
          state.bms = datapoint.state;
      }
      DataMessage::Sam(id, datapoints) => {
          // Implement SAM state update logic here
      }
      DataMessage::Identity(id) => {
      }
      DataMessage::FlightHeartbeat => {
          // Handle heartbeat if needed
      }
    }
  }
}




pub(super) fn ingest(state: &mut VehicleState, data: Vec<DataMessage>) {
  for message in data {
    message.update_state(state);     
  }
}
