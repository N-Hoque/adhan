# Adhan

Adhan is an audio playback program. It wait for the next prayer and plays the Adhan when the time for prayer arrives.

## How to use

When running the program for the first time, in the user's default config directory, there will be a folder called `adhan`. In here will be:

- `settings.yaml`: Stores calculation data for timekeeping
- `audio` folder: The audio files for the adhan
  - `normal.mp3`: For the standard adhan
  - `fajr.mp3`: For the Fajr-specific adhan

These files must be present to fully run the program. Audio files must be sourced by the user.

The program can be called via the CLI: `./adhan`. Doing so shows the following commands available to use:

Command | Description
--:|:--
`run` | Runs the program. Uses the default audio device, but can specify others as well
`test` | Test audio playback. Can pass in the `--use-fajr` flag to specifically play the Fajr adhan
`generate` | Generates configuration file. Defaults to `Other` for full control, but can use a specific method as well
`timetable` | Displays prayer timetable. On Linux, can be combined with built-in `watch` command to repeatedly update the timetable view i.e. `watch -n 1 ./adhan timetable`
`list` | Lists audio elements. Can either list `devices` or `hosts`

## Note on Raspberry Pi

This program was designed to run on a Raspberry Pi 2 (Model B) and Raspberry Pi 3+ (Raspberry Pi 0/1 are not officially supported). To facilitate this, a `compile.sh` script has been provided along with several Dockerfiles to perform cross-compilation onto these devices. The deploy script will build and copy the binary application onto the PI via SSH. As such, the PI must be accessible via SSH.

To use it:

- Run `./compile.sh -t 2` to build for Raspberry PI 2.
- Run `./compile.sh -t 2 -u <PI_USERNAME> -a <PI_IP> -d` to build and deploy onto Raspberry PI 2.
- Run `./compile.sh -t 3 -u <PI_USERNAME> -a <PI_IP> -d` to build and deploy onto Raspberry PI 3 and higher.

Once copied, you can SSH to the PI and run `adhan_player --help` for help info.

## Contributors

N-Hoque <Naimul.Hoque@outlook.com> (Author)
