# MDSORTO - MD Standup Ordering Randomization Tool

MDSORTO is a terminal-based application written in Rust that helps manage standup meetings by randomizing the speaking order and providing a configurable timer for each participant.

## Features

- Random ordering of participants for daily standups
- Configurable time limits for each speaker
- Initial preparation time before starting
- Visual countdown timer with color-coded warnings
- Terminal-based UI using Ratatui
- Configuration via INI file
- Logging support

## Installation

Ensure you have Rust and Cargo installed on your system. Then clone this repository and build the project:

```bash
git clone [repository-url]
cd mdsorto
cargo build --release
```

The compiled binary will be available in `target/release/mdsorto`.

## Configuration

Create a file named `mdsorto.ini` in the same directory as the executable. Example configuration:

```ini
[mdsorto]
timeeach = 60.2    # Time in seconds for each person
preptime = 30.2    # Initial preparation time in seconds
people = Alice, Bob, Charlie, Diana    # Comma-separated list of participants
```

### Configuration Options

- `timeeach`: Time allocated for each person (in seconds)
- `preptime`: Initial preparation time before starting (in seconds)
- `people`: Comma-separated list of participants

If no configuration file is found, the application will use default values:
- Time per person: 60.2 seconds
- Preparation time: 30.2 seconds
- Default participants: "Person 1", "Person 2", "Person 3"

## Usage

Start the application by running:

```bash
./mdsorto
```

### Controls

- `Space`: Start timer / Skip to next person
- `p`: Pause/Unpause timer
- `r`: Restart current person's timer
- `b`: Go back to previous person
- `+`/`=`: Add 10 seconds
- `-`/`_`: Remove 10 seconds
- `q` or `Esc`: Quit application

### Timer Colors

The timer display changes color based on remaining time:
- Green: More than 20 seconds remaining
- Yellow: Less than 20 seconds remaining
- Red: Less than 7.5 seconds remaining

## Logging

The application creates a log file named `mdsorto.log` in the same directory, which can be useful for troubleshooting.

## Development

### Dependencies

Key dependencies include:
- `ratatui`: Terminal UI framework
- `crossterm`: Terminal manipulation
- `tokio`: Async runtime
- `color-eyre`: Error handling
- `simplelog`: Logging functionality
- `ini`: Configuration file parsing

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release
```

### Project Structure

- Terminal UI implementation using Ratatui
- Event handling system for keyboard input
- Timer management with color-coded states
- Configuration management using INI files
- Logging system for debugging

## Version

Current version: V0.3.1

## License

[License information not provided in source code]

## Contributing

[Contribution guidelines not provided in source code]

## Author

0x4D44 (November 2023)