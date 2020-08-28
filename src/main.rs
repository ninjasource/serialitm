extern crate clap;
use std::str;

extern crate chrono;
use chrono::Local;

use clap::{App, AppSettings, Arg};
use itm::{packet, packet::Packet, Decoder};
use std::{io::Error as StdError, time::Duration};

#[cfg(unix)]
use mio::unix::UnixReady;
use mio::{Events, Poll, PollOpt, Ready, Token};

const SERIAL_TOKEN: Token = Token(0);

#[cfg(unix)]
fn ready_of_interest() -> Ready {
    Ready::readable() | UnixReady::hup() | UnixReady::error()
}

#[cfg(windows)]
fn ready_of_interest() -> Ready {
    Ready::readable()
}

#[cfg(unix)]
fn is_closed(state: Ready) -> bool {
    state.contains(UnixReady::hup() | UnixReady::error())
}

#[cfg(windows)]
fn is_closed(_state: Ready) -> bool {
    false
}

#[derive(Debug)]
enum Error {
    PollError(StdError),
    PortClosed,
    StdError(StdError),
}

impl From<StdError> for Error {
    fn from(e: StdError) -> Error {
        Error::StdError(e)
    }
}

fn handle_packet(p: Packet, itm_port: u8, should_write_newline: &mut bool) -> Result<(), Error> {
    match p.kind() {
        &packet::Kind::Instrumentation(ref i) if i.port() == itm_port => {
            let payload = &i.payload();
            if let Ok(s) = str::from_utf8(payload) {
                // remove the new line from the payload (if it exists)
                // and inject a timestamp and newline in its place
                for (i, line) in s.split("\n").enumerate() {
                    if *should_write_newline {
                        let now = Local::now();

                        // 24 hour format - YYYY-mm-DD HH:MM:SS.FFF
                        print!("{} ", now.format("%Y-%m-%d %H:%M:%S%.3f"));
                        *should_write_newline = false;
                    }

                    print!("{}", line);

                    if i > 0 {
                        println!();
                        *should_write_newline = true;
                    }
                }
            } else {
                println!("Invalid payload: {:?}", payload);
            }
        }
        o => println!("o: {:?}", o),
    }
    Ok(())
}

fn main() -> Result<(), Error> {
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

    let baud_rate = matches
        .value_of("baud")
        .unwrap()
        .parse::<u32>()
        .expect("Invalid baud rate");

    let itm_port = matches
        .value_of("itmport")
        .unwrap() // We supplied a default value
        .parse::<u8>()
        .expect("Arg validator should ensure this parses");

    // Set up mio & mio serialport
    let poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(1024);
    let mut mio_settings = mio_serial::SerialPortSettings::default();

    // Set the baud rate and timout
    mio_settings.timeout = Duration::from_millis(10);
    mio_settings.baud_rate = baud_rate;

    // open the serial port and begin reading ITM packets
    match mio_serial::Serial::from_path(&com_port_name, &mio_settings) {
        Ok(port) => {
            poll.register(&port, SERIAL_TOKEN, ready_of_interest(), PollOpt::edge())
                .map_err(|e| Error::PollError(e))?;

            println!(
                "Receiving ITM data (port {}) on {} at {:?} baud:",
                &itm_port, &com_port_name, &baud_rate
            );
            let mut decoder = Decoder::new(port, false);
            let mut should_write_newline = true;
            loop {
                poll.poll(&mut events, None)
                    .map_err(|e| Error::PollError(e))?;

                if events.is_empty() {
                    // Read times out every couple of seconds - no need to log this
                    continue;
                }

                for event in events.iter() {
                    match event.token() {
                        SERIAL_TOKEN => {
                            let ready = event.readiness();
                            if is_closed(ready) {
                                Err(Error::PortClosed)?;
                            }

                            if ready.is_readable() {
                                // With edge triggered events, we must perform reading until we receive a WouldBlock.
                                // See https://docs.rs/mio/0.6/mio/struct.Poll.html for details.
                                while let Ok(p) = decoder.read_packet() {
                                    handle_packet(p, itm_port, &mut should_write_newline)?;
                                }
                            }
                        }
                        t => unreachable!("Unexpected token: {:?}", t),
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
