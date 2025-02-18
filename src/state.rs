use common::comm::sam::SamControlMessage;
use common::comm::{ahrs::DataPoint, flight::DataMessage, VehicleState, NodeMapping, sam, Measurement,
  SensorType,
  ValveState, Unit, ChannelType, CompositeValveState};
use common::comm::ahrs;
use std::borrow::Cow;
use std::{collections::HashMap};
use crate::config::{BoardId};
use jeflog::{fail, pass, warn};


/// Updates the vehicle state with the new data recieved (flight 1.0 code can be reused)
trait DataHandler{
  fn update_state(&self, state: &mut VehicleState, mappings: &[NodeMapping]);
}





impl <'a> DataHandler for DataMessage <'a> {
  fn update_state(&self, state: &mut VehicleState, mappings: &[NodeMapping]) {
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
          process_sam_data(state, datapoints, mappings, id);
      }
      DataMessage::Identity(id) => {
      }
      DataMessage::FlightHeartbeat => {
          // Handle heartbeat if needed
      }
    }
  }
}




pub(super) fn ingest(state: &mut VehicleState, data: Vec<DataMessage>, mappings: &[NodeMapping]) {
  for message in data {
    message.update_state(state, mappings);     
  }
}


fn process_sam_data(state: &mut VehicleState, datapoints: &[sam::DataPoint], mappings: &[NodeMapping], board_id: BoardId) {
  for data_point in datapoints {
    for mapping in &*mappings {
      let corresponds = data_point.channel == mapping.channel
        && mapping
          .sensor_type
          .channel_types()
          .contains(&data_point.channel_type)
        && *board_id == mapping.board_id;

      if !corresponds {
        continue;
      }

      let mut text_id = mapping.text_id;

      let measurement = match mapping.sensor_type {
        SensorType::RailVoltage => Measurement {
          value: data_point.value,
          unit: Unit::Volts,
        },
        SensorType::Rtd | SensorType::Tc => Measurement {
          value: data_point.value,
          unit: Unit::Kelvin,
        },
        SensorType::RailCurrent => Measurement {
          value: data_point.value,
          unit: Unit::Amps,
        },
        SensorType::Pt => {
          let value;
          let unit;

          // apply linear transformations to current loop and differential
          // signal channels if the max and min are supplied by the mappings.
          // otherwise, default back to volts.
          if let (Some(max), Some(min)) = (mapping.max, mapping.min) {
            // formula for converting voltage into psi for our PTs
            // TODO: consider precalculating scale and offset on control server
            value = (data_point.value - 0.8) / 3.2 * (max - min) + min
              - mapping.calibrated_offset;
            unit = Unit::Psi;
          } else {
            // if no PT ratings are set, default to displaying raw voltage
            value = data_point.value;
            unit = Unit::Volts;
          }

          Measurement { value, unit }
        }
        SensorType::LoadCell => {
          // if no load cell mappings are set, default to these values
          let mut value = data_point.value;
          let mut unit = Unit::Volts;

          // apply linear transformations to load cell channel if the max and
          // min are supplied by the mappings. otherwise, default back to volts.
          if let (Some(max), Some(min)) = (mapping.max, mapping.min) {
            // formula for converting voltage into pounds for our load cells
            value = (max - min) / 0.03 * (value + 0.015) + min
              - mapping.calibrated_offset;
            unit = Unit::Pounds;
          }

          Measurement { value, unit }
        }
        SensorType::Valve => {
          let voltage;
          let current;
          let measurement;

          match data_point.channel_type {
            ChannelType::ValveVoltage => {
              voltage = data_point.value;
              current = state
                .sensor_readings
                .get(&format!("{text_id}_I"))
                .map(|measurement| measurement.value)
                .unwrap_or(0.0);

              measurement = Measurement {
                value: data_point.value,
                unit: Unit::Volts,
              };
              text_id = format!("{text_id}_V");
            }
            ChannelType::ValveCurrent => {
              current = data_point.value;
              voltage = state
                .sensor_readings
                .get(&format!("{text_id}_V"))
                .map(|measurement| measurement.value)
                .unwrap_or(0.0);

              measurement = Measurement {
                value: data_point.value,
                unit: Unit::Amps,
              };
              text_id = format!("{text_id}_I");
            }
            channel_type => {
              warn!("Measured channel type of '{channel_type:?}' for valve.");
              continue;
            }
          };

          let actual_state = estimate_valve_state(
            voltage,
            current,
            mapping.powered_threshold,
            mapping.normally_closed,
          );

          if let Some(existing) =
            state.valve_states.get_mut(&mapping.text_id)
          {
            existing.actual = actual_state;
          } else {
            state.valve_states.insert(
              mapping.text_id,
              CompositeValveState {
                commanded: ValveState::Undetermined,
                actual: actual_state,
              },
            );
          }

          println!(
            "M: Value: {}, Unit: {}",
            measurement.value, measurement.unit
          );
          measurement
        }
      };

      // replace item without cloning string if already present
      if let Some(existing) = state.sensor_readings.get_mut(&text_id) {
        *existing = measurement;
      } else {
        state.sensor_readings.insert(text_id, measurement);
      }
    }
  }
}

/// Estimates the state of a valve given its voltage, current, and the current
/// threshold at which it is considered powered.
fn estimate_valve_state(
  voltage: f64,
  current: f64,
  powered_threshold: Option<f64>,
  normally_closed: Option<bool>,
) -> ValveState {
  // calculate the actual state of the valve, assuming that it's normally closed
  let mut estimated = match powered_threshold {
    Some(powered) => {
      if current < powered {
        // valve is unpowered
        if voltage < 4.0 {
          ValveState::Closed
        } else {
          ValveState::Disconnected
        }
      } else {
        // valve is powered
        if voltage < 20.0 {
          ValveState::Fault
        } else {
          ValveState::Open
        }
      }
    }
    None => ValveState::Fault,
  };

  if normally_closed == Some(false) {
    estimated = match estimated {
      ValveState::Open => ValveState::Closed,
      ValveState::Closed => ValveState::Open,
      other => other,
    };
  }

  estimated
}
