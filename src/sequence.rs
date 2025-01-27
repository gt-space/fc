use common::comm::{NodeMapping, SensorType, Sequence};
use std::{collections::HashMap, io, process::{Child, Command}};

fn run(mappings: &Vec<NodeMapping>, sequence: &Sequence) -> io::Result<Child> {
    let mut script = String::from("from sequences import *;");
    for mapping in mappings {
        let definition = match mapping.sensor_type {
            SensorType::Valve => format!("{0} = Valve('{0}');", mapping.text_id),
            _ => format!("{0} = Sensor('{0}');", mapping.text_id),
        };

        script.push_str(&definition);
    }
    
    script.push_str(&sequence.script);
    Command::new("python3").args(["-c", &script]).spawn()
}

pub(crate) fn execute(mappings: &Vec<NodeMapping>, sequence: Sequence, sequences: &mut HashMap<String, Child>) {
    if let Some(running) = sequences.get_mut(&sequence.name) {
        match running.try_wait() {
            Ok(Some(_)) => {},
            Ok(None) => {
                println!("The '{}' sequence is already running. Stop it before re-attempting execution.", sequence.name);
                return;
            },
            Err(e) => {
                eprintln!("Another '{}' sequence was previously ran, but it's status couldn't be determined: {e}", sequence.name);
                return;
            },
        }
    }
    
    let process = match run(mappings, &sequence) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error in running python3: {e}");
            return;
        }
    };

    sequences.insert(sequence.name, process);
}

pub(crate) fn abort(sequences: &mut HashMap<String, Child>, name: &String) -> io::Result<()> {
    let sequence = match sequences.get_mut(name) {
        Some(c) => {
            if let Ok(None) = c.try_wait() {
                println!("A sequence named '{name}' isn't running.");
                return Ok(());
            }

            c
        },
        None => {
            println!("A sequence named '{name}' isn't running.");
            return Ok(());
        }
    };

    sequence.kill()
}