# Design

Normalize JSON keys as the existing policy does, then match a bounded set of sensitive suffixes and aliases. Replace the entire field value when its key is sensitive, including array or object values. Continue recursively visiting non-sensitive containers. This keeps the redaction at the single response boundary and avoids exposing nested values through alternate DTO naming.
