extern crate clap;

use clap::{App, AppSettings, Arg};
use itm::{packet, Decoder};
use serialport::prelude::*;
use std::io::{self, Write};
use std::time::Duration;

fn main() {
    // get the command line args
    let matches = App::new("Serial Port ITM Decoder")
        .about("Reads ITM encoded bytes off the serial port and prints them to the console")
        .setting(AppSettings::DisableVersion)
        .arg(
            Arg::with_name("comport")
                .help("The device path to a serial port (e.g. COM3)")
                .use_delimiter(false)
                .required(true),
        )
        .arg(
            Arg::with_name("baud")
                .help("The baud rate to connect at (e.g. 1000000)")
                .use_delimiter(false)
                .required(true)
                .default_value("1000000")
                .validator(|s| match s.parse::<u32>() {
                    Ok(_) => Ok(()),
                    Err(e) => Err(e.to_string()),
                }),
        )
        .arg(
            Arg::with_name("itmport")
                .help("The ITM stimulus port number (e.g. 0)")
                .use_delimiter(false)
                .default_value("0")
                .validator(|s| match s.parse::<u8>() {
                    Ok(_) => Ok(()),
                    Err(e) => Err(e.to_string()),
                }),
        )
        .get_matches();
    let com_port_name = matches.value_of("comport").unwrap();
    let baud_rate = matches.value_of("baud").unwrap();
    let itm_port = matches
        .value_of("itmport")
        .unwrap() // We supplied a default value
        .parse::<u8>()
        .expect("Arg validator should ensure this parses");

    // setup the serial port
    let mut settings: SerialPortSettings = Default::default();
    settings.timeout = Duration::from_millis(10);
    if let Ok(rate) = baud_rate.parse::<u32>() {
        settings.baud_rate = rate;
    } else {
        eprintln!("Error: Invalid baud rate '{}' specified", baud_rate);
        ::std::process::exit(1);
    }

    // open the serial port and begin reading ITM packets
    match serialport::open_with_settings(&com_port_name, &settings) {
        Ok(port) => {
            println!(
                "Receiving ITM data (port {}) on {} at {} baud:",
                &itm_port, &com_port_name, &baud_rate
            );
            let mut decoder = Decoder::new(port, true);
            loop {
                let p = decoder.read_packet();
                match p {
                    Ok(p) => match p.kind() {
                        &packet::Kind::Instrumentation(ref i) if i.port() == itm_port => {
                            io::stdout().write_all(&i.payload()).unwrap();
                            io::stdout().flush().unwrap();
                        }
                        _ => (),
                    },
                    Err(_) => {
                        // Do nothing, there are many errors (mostly timeouts when nothing happens)
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to open \"{}\". Error: {}", com_port_name, e);
            ::std::process::exit(1);
        }
    }
}
