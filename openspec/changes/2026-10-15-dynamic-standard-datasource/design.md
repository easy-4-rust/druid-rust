# Design

The canonical `DataSource::connection` method delegates to the existing product `get_connection`, which already selects the current group and returns an owned pooled connection. Closing the returned standard connection recycles through that selected pool. The standard `CommonDataSource` defaults remain explicit for unsupported configuration; the standard credential overload inherits the feature-not-supported default. `Wrapper` returns exact `DataSource` and `CommonDataSource` views of `self`, preserving identity and not borrowing a connection.
