## ADDED Requirements

### Requirement: JDBC Agent callable capability reflects executable operations

The JDBC Agent adapter SHALL return an owned standard `CallableStatement` from `Connection::prepare_call` only when the current session can prepare and execute the call. Its reported callable capability SHALL reflect the implemented protocol and connected driver, not the presence of a Rust trait declaration alone. Other adapters SHALL retain their verified unsupported behavior.

#### Scenario: Supported driver

- **WHEN** a connected driver supports a callable operation and an application prepares it through the standard connection
- **THEN** the returned handle can register OUT parameters, bind IN parameters, execute, read OUT values, and close within the same session

#### Scenario: Unsupported driver or method

- **WHEN** the driver or protocol cannot perform a requested callable operation
- **THEN** the capability and operation report a specific standard unsupported SQL error, without a false successful value or leaked server handle

### Requirement: Callable parameter and result semantics survive the protocol

The Agent protocol SHALL preserve indexed versus named parameter selection, SQL type and optional scale/type name, SQL NULL and `was_null`, typed OUT conversion, the current result sequence, and SQLState/vendor-code/cause information. It SHALL reject invalid 1-based indexes and unknown names before claiming execution success. Re-execution SHALL invalidate stale OUT values and result handles.

#### Scenario: IN and OUT round trip

- **WHEN** a caller registers an OUT parameter, binds an IN value, executes and reads the result against a real JDBC database
- **THEN** the value and null status match the driver, with the specified selector and SQL type retained

#### Scenario: Standard max-rows property reaches each callable result set

- **WHEN** a caller sets a positive `Statement::set_max_rows` limit before executing a call that returns multiple result sets
- **THEN** the JDBC statement applies that limit to every result set, including results reached through `getMoreResults`, without discarding their order

#### Scenario: Failure and retry

- **WHEN** a call fails or is cancelled, then the statement is executed again
- **THEN** the old OUT values and cursors are inaccessible, the original JDBC error remains observable, and the same open CallableStatement can execute again

#### Scenario: Cancel the current execution

- **WHEN** a caller cancels an active callable execution or its result-page fetch
- **THEN** only that execution is interrupted, its dependent cursors are released, and cancellation cannot affect a later execution on the same statement

### Requirement: Callable ownership is bounded by session and statement lifetimes

The Agent SHALL close callable statements and dependent cursors on explicit close, session close, and connection failure. Cancellation SHALL release the current execution's dependent cursors while keeping the callable statement open for retry. A handle from another session or a closed statement SHALL fail without reaching its former JDBC object. Callback and resource cleanup SHALL happen once.

#### Scenario: Close and cross-session access

- **WHEN** a caller closes a callable statement or sends its handle through another session
- **THEN** the Agent rejects subsequent operations and releases the original driver resources exactly once
