# Adhan

Adhan is an audio playback program. It wait for the next prayer and plays the Adhan when the time for prayer arrives.

## How to use

This program was designed to run on a Raspberry Pi 2 (Model B) and Raspberry Pi 3+ (Raspberry Pi 0/1 are not officially supported). To facilitate this, a `compile.sh` script has been provided along with several Dockerfiles to perform cross-compilation onto these devices. The deploy script will build and copy the binary application onto the PI via SSH. As such, the PI must be accessible via SSH.

To use it:

- Run `./compile.sh -t 2 -u <PI_USERNAME> -a <PI_IP> -d` to copy onto a Raspberry PI 2.
- Run `./compile.sh -t 3 -u <PI_USERNAME> -a <PI_IP> -d` to copy onto a Raspberry PI 3 and higher.

Once copied, you can SSH to the PI and run `adhan_player --help` for help info.

- Run `adhan_player -p` to test audio output. If no output is heard, you'll want to specify the output device
- Run `adhan_player -l` to list all audio devices
- Run `adhan_player -t <DEVICE_NAME>` to play using the specified audio device. This can be combined with `-p` for testing

## Contributors

N-Hoque <Naimul.Hoque@outlook.com> (Author)
