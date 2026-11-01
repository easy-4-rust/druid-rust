## ADDED Requirements

### Requirement: Oracle fatal-code properties preserve Java integer parsing
Oracle and OceanBase Oracle classifiers SHALL preserve `AbstractOracleExceptionSorter.configFromProperties` semantics at the pinned Java source and JDK 17 oracle: BMP decimal digits, ASCII signs, signed 32-bit bounds, append and set replacement behavior.

#### Scenario: Unicode decimal codes
- **WHEN** a comma-separated property contains fullwidth, Arabic-Indic or other BMP decimal digits accepted by Java Integer.parseInt
- **THEN** the same integer codes are added to the fatal-code set and affect exception classification

#### Scenario: Invalid and overflowing entries
- **WHEN** entries contain surrounding whitespace, invalid signs, nondecimal numerals, surrogate pairs or overflowing values
- **THEN** those entries add no code while other valid entries remain effective

#### Scenario: Append and replace
- **WHEN** properties are applied repeatedly and the explicit set is subsequently replaced
- **THEN** configuration appends and deduplicates, and replacement removes the previous values exactly as the existing API specifies
