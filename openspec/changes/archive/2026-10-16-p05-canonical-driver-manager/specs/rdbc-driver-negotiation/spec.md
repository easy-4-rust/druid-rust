## ADDED Requirements

### Requirement: DN-04 One canonical process-wide DriverManager

The standard `druid-rdbc::driver::DriverManager` SHALL own the process-wide driver registrations, deregistration callbacks, login timeout, and diagnostic writer. `druid::sql::DriverManager` SHALL be an exact re-export of that same type. Registration and settings through either path SHALL be observable through the other path without a second registry or conversion facade.

#### Scenario: Cross-path registration and callback

- **WHEN** an external consumer registers a driver and action through the standard path, finds the same driver through the Druid path, and deregisters through Druid
- **THEN** both paths observe the same driver identity and the action runs exactly once

#### Scenario: Cross-path settings

- **WHEN** the login timeout and diagnostic writer are set through one path
- **THEN** the other path reads the identical values and writer identity
