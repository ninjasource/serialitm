# serialitm
A Rust command line tool used to read ITM packets off the serial port. This tool was specifically written to help out us poor windows users who always struggle with the serial port. The code really just connects two crates together: itm and serialport

Example Usage:

serialitm [comport] [baud]

e.g.

cargo run com3 1000000

or just edit and use run.bat directly.
