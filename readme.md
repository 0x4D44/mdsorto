# MDSORTO: MD Standup Ordering Randomization Tool

MDSORTO is a command-line standup meeting helper written in Rust that randomizes the order of speakers and manages time during your meetings. With built-in timers and dynamic, color-coded feedback, it ensures that everyone gets a fair chance to speak while keeping the meeting on track.

---

## Table of Contents

- [Overview](#overview)
- [Features](#features)
- [Installation](#installation)
- [Configuration](#configuration)
- [Usage](#usage)
- [Code Structure](#code-structure)
- [Contributing](#contributing)
- [License & Acknowledgements](#license--acknowledgements)

---

## Overview

MDSORTO (MD Standup Ordering Randomization Tool) was created by **mdavidson** in November 2023 to help teams conduct standup meetings more effectively. It takes a list of participants, shuffles their order, and assigns each person a fixed amount of time to speak. During each speaker’s turn, the timer counts down and changes color as the allotted time diminishes, alerting the speaker when their time is almost up.

---

## Features

- **Randomized Order:** Automatically shuffles a list of users to ensure a fair speaking order.
- **Timer with Visual Cues:**  
  - **Green:** Ample time remaining.  
  - **Yellow:** 15 seconds remaining.  
  - **Red:** 5 seconds remaining.
- **Dynamic Control:**  
  - **Space/Enter:** Start the meeting or move to the next speaker.  
  - **P:** Pause/unpause the timer.  
  - **R:** Restart the timer for the current speaker.  
  - **B:** Go back to the previous speaker.  
  - **+ / =:** Add an extra 10 seconds.  
  - **- / _ (or X):** Subtract 10 seconds.
- **Configuration File Support:** Customize timings and participant lists via an INI configuration file.
- **Real-time UI:** Built using [ratatui](https://github.com/ratatui-org/ratatui) and [crossterm](https://github.com/crossterm-rs/crossterm) for a smooth terminal experience.
- **Logging:** Integrated logging via [simplelog](https://github.com/drakulix/simplelog) to track application events and configuration loads.

---

## Installation

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable version recommended)
- Cargo (comes with Rust)

### Building from Source

1. **Clone the repository:**

   ```bash
   git clone https://github.com/yourusername/mdsorto.git
   cd mdsorto
