package io.github.easy4rust.druid.agent;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.JsonNodeFactory;
import com.fasterxml.jackson.databind.node.ObjectNode;
import lombok.extern.slf4j.Slf4j;

import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.Closeable;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.io.OutputStreamWriter;
import java.nio.charset.StandardCharsets;
import java.math.BigDecimal;
import java.sql.Connection;
import java.sql.DatabaseMetaData;
import java.sql.Date;
import java.sql.DriverManager;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.ResultSetMetaData;
import java.sql.SQLException;
import java.sql.SQLFeatureNotSupportedException;
import java.sql.Statement;
import java.sql.Time;
import java.sql.Timestamp;
import java.sql.Types;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.time.format.DateTimeFormatter;
import java.util.Base64;
import java.util.Iterator;
import java.util.Map;
import java.util.Objects;
import java.util.Properties;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;

/** JSON-RPC 2.0 NDJSON 服务端；一个共享进程可隔离承载多个 JDBC session。 */
@Slf4j
public final class AgentServer implements Closeable {

    private static final int PROTOCOL_VERSION = 1;
    private static final int MAX_FRAME_BYTES = 16 * 1024 * 1024;
    private static final DateTimeFormatter TIMESTAMP_FORMAT =
            DateTimeFormatter.ofPattern("yyyy-MM-dd'T'HH:mm:ss.SSSSSSSSS");

    private final BufferedReader input;
    private final BufferedWriter output;
    private final ObjectMapper objectMapper;
    private final Map<String, AgentSession> sessions = new ConcurrentHashMap<>();
    private Connection connection;
    private String validationQuery;

    /**
     * 创建使用指定字节流的服务端。
     *
     * @param input  JSON-RPC NDJSON 输入流
     * @param output JSON-RPC NDJSON 输出流
     */
    public AgentServer(InputStream input, OutputStream output) {
        this.input = new BufferedReader(new InputStreamReader(
                Objects.requireNonNull(input, "input"), StandardCharsets.UTF_8));
        this.output = new BufferedWriter(new OutputStreamWriter(
                Objects.requireNonNull(output, "output"), StandardCharsets.UTF_8));
        this.objectMapper = new ObjectMapper();
    }

    /** 发送 ready 通知并执行 JSON-RPC 请求循环，直到 EOF。 */
    public void run() throws IOException {
        ObjectNode ready = JsonNodeFactory.instance.objectNode();
        ready.put("jsonrpc", "2.0");
        ready.put("method", "ready");
        ObjectNode readyParams = ready.putObject("params");
        readyParams.put("protocolVersion", PROTOCOL_VERSION);
        readyParams.put("agentVersion", "0.0.0-design");
        readyParams.putArray("capabilities")
                .add("multi-session")
                .add("structured-errors")
                .add("tagged-values");
        writeLine(ready);

        String line;
        while (Objects.nonNull(line = input.readLine())) {
            AgentRequest request = null;
            AgentResponse response;
            try {
                if (line.getBytes(StandardCharsets.UTF_8).length > MAX_FRAME_BYTES) {
                    throw new IOException("JSON-RPC request exceeds maximum frame size");
                }
                request = objectMapper.readValue(line, AgentRequest.class);
                validateRequest(request);
                JsonNode result = dispatch(request.method(), request.params());
                response = AgentResponse.success(request.id(), result);
            } catch (Exception exception) {
                long requestId = Objects.isNull(request) ? 0 : request.id();
                response = AgentResponse.failure(requestId, exception);
                log.debug("JDBC operation failed: requestId={}", requestId, exception);
            }
            writeLine(objectMapper.valueToTree(response));
        }
    }

    private void validateRequest(AgentRequest request) {
        Objects.requireNonNull(request, "request");
        Objects.requireNonNull(request.method(), "method");
        if (!"2.0".equals(request.jsonrpc())) {
            throw new IllegalArgumentException("unsupported jsonrpc version " + request.jsonrpc());
        }
    }

    private JsonNode dispatch(String method, JsonNode params) throws SQLException, IOException {
        if ("handshake".equals(method)) {
            if (params.path("protocolVersion").asInt() != PROTOCOL_VERSION) {
                throw new IllegalArgumentException("unsupported protocolVersion "
                        + params.path("protocolVersion").asInt());
            }
            ObjectNode result = JsonNodeFactory.instance.objectNode();
            result.put("protocolVersion", PROTOCOL_VERSION);
            result.put("agentVersion", "0.0.0-design");
            return result;
        }
        if ("session.open".equals(method)) {
            return openSession(params);
        }
        if ("session.close".equals(method)) {
            return closeSession(params);
        }
        String operation = method.startsWith("session.") ? method.substring("session.".length()) : method;
        return withSession(params, () -> switch (operation) {
            case "exec" -> exec(params);
            case "fetch" -> fetch(params);
            case "execute" -> execute(params);
            case "begin" -> begin();
            case "commit" -> commit();
            case "rollback" -> rollback();
            case "ping" -> ping();
            case "set_auto_commit" -> setAutoCommit(params);
            case "set_read_only" -> setReadOnly(params);
            case "set_transaction_isolation" -> setTransactionIsolation(params);
            default -> throw new UnsupportedOperationException("unsupported method " + method);
        });
    }

    private JsonNode openSession(JsonNode payload) throws SQLException {
        JsonNode result = connect(payload);
        String sessionId = UUID.randomUUID().toString();
        sessions.put(sessionId, new AgentSession(connection, validationQuery));
        connection = null;
        validationQuery = null;
        ((ObjectNode) result).put("sessionId", sessionId);
        return result;
    }

    private JsonNode closeSession(JsonNode payload) throws SQLException {
        String sessionId = requiredText(payload, "sessionId");
        AgentSession session = sessions.remove(sessionId);
        if (Objects.isNull(session)) {
            return JsonNodeFactory.instance.objectNode();
        }
        try {
            session.connection().close();
        } finally {
            connection = null;
            validationQuery = null;
        }
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode withSession(JsonNode payload, SessionOperation operation) throws SQLException, IOException {
        String sessionId = requiredText(payload, "sessionId");
        AgentSession session = sessions.get(sessionId);
        if (Objects.isNull(session) || session.connection().isClosed()) {
            throw new SQLException("JDBC Agent session is not open: " + sessionId);
        }
        connection = session.connection();
        validationQuery = session.validationQuery();
        try {
            return operation.execute();
        } finally {
            connection = null;
            validationQuery = null;
        }
    }

    @FunctionalInterface
    private interface SessionOperation {
        JsonNode execute() throws SQLException, IOException;
    }

    private JsonNode connect(JsonNode payload) throws SQLException {
        if (Objects.nonNull(connection)) {
            throw new IllegalStateException("JDBC connection is already initialized");
        }
        Objects.requireNonNull(payload, "connect payload");
        JsonNode urlNode = Objects.requireNonNull(payload.get("url"), "url");
        Properties properties = new Properties();
        JsonNode propertyNode = payload.get("properties");
        if (Objects.nonNull(propertyNode) && propertyNode.isObject()) {
            Iterator<Map.Entry<String, JsonNode>> fields = propertyNode.fields();
            while (fields.hasNext()) {
                Map.Entry<String, JsonNode> field = fields.next();
                properties.setProperty(field.getKey(), field.getValue().asText());
            }
        }
        JsonNode validationNode = payload.get("validationQuery");
        validationQuery = Objects.isNull(validationNode) || validationNode.isNull()
                ? null
                : validationNode.asText();
        connection = DriverManager.getConnection(urlNode.asText(), properties);
        DatabaseMetaData metadata = connection.getMetaData();
        ObjectNode result = JsonNodeFactory.instance.objectNode();
        result.put("agentVersion", "0.0.0-design");
        result.put("driverName", metadata.getDriverName());
        result.put("driverVersion", metadata.getDriverVersion());
        result.put("databaseProductName", metadata.getDatabaseProductName());
        result.put("databaseProductVersion", metadata.getDatabaseProductVersion());
        return result;
    }

    private JsonNode exec(JsonNode payload) throws SQLException, IOException {
        Connection activeConnection = activeConnection();
        try (PreparedStatement statement = activeConnection.prepareStatement(
                requiredText(payload, "sql"), Statement.RETURN_GENERATED_KEYS)) {
            bind(statement, payload.path("params"));
            long rowsAffected = Math.max(0, statement.executeUpdate());
            return updateResult(statement, rowsAffected);
        }
    }

    private JsonNode fetch(JsonNode payload) throws SQLException, IOException {
        Connection activeConnection = activeConnection();
        try (PreparedStatement statement = activeConnection.prepareStatement(requiredText(payload, "sql"))) {
            bind(statement, payload.path("params"));
            try (ResultSet resultSet = statement.executeQuery()) {
                return queryResult(resultSet);
            }
        }
    }

    private JsonNode execute(JsonNode payload) throws SQLException, IOException {
        Connection activeConnection = activeConnection();
        try (PreparedStatement statement = prepareForExecute(
                activeConnection,
                requiredText(payload, "sql"),
                payload.path("generatedKeys"))) {
            bind(statement, payload.path("params"));
            boolean query = statement.execute();
            if (query) {
                try (ResultSet resultSet = statement.getResultSet()) {
                    return queryResult(resultSet);
                }
            }
            return updateResult(statement, Math.max(0, statement.getUpdateCount()));
        }
    }

    private PreparedStatement prepareForExecute(
            Connection activeConnection,
            String sql,
            JsonNode generatedKeys) throws SQLException {
        String mode = generatedKeys.path("mode").asText("none");
        return switch (mode) {
            case "auto" -> activeConnection.prepareStatement(sql, generatedKeys.path("value").asInt());
            case "column_indexes" -> activeConnection.prepareStatement(sql, intArray(generatedKeys.path("value")));
            case "column_names" -> activeConnection.prepareStatement(sql, stringArray(generatedKeys.path("value")));
            case "none" -> activeConnection.prepareStatement(sql);
            default -> throw new IllegalArgumentException("unsupported generatedKeys mode " + mode);
        };
    }

    private JsonNode begin() throws SQLException {
        activeConnection().setAutoCommit(false);
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode commit() throws SQLException {
        activeConnection().commit();
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode rollback() throws SQLException {
        activeConnection().rollback();
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode ping() throws SQLException {
        Connection activeConnection = activeConnection();
        try {
            if (activeConnection.isValid(5)) {
                return JsonNodeFactory.instance.objectNode();
            }
        } catch (SQLFeatureNotSupportedException ignored) {
            log.debug("JDBC driver does not implement Connection.isValid");
        }
        if (Objects.nonNull(validationQuery)) {
            try (Statement statement = activeConnection.createStatement()) {
                statement.execute(validationQuery);
                return JsonNodeFactory.instance.objectNode();
            }
        }
        throw new SQLException("JDBC connection validation failed");
    }

    private JsonNode setAutoCommit(JsonNode payload) throws SQLException {
        activeConnection().setAutoCommit(payload.path("value").asBoolean());
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode setReadOnly(JsonNode payload) throws SQLException {
        activeConnection().setReadOnly(payload.path("value").asBoolean());
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode setTransactionIsolation(JsonNode payload) throws SQLException {
        activeConnection().setTransactionIsolation(payload.path("value").asInt());
        return JsonNodeFactory.instance.objectNode();
    }

    private JsonNode closeConnection() throws SQLException {
        if (Objects.nonNull(connection)) {
            connection.close();
            connection = null;
        }
        return JsonNodeFactory.instance.objectNode();
    }

    private ObjectNode queryResult(ResultSet resultSet) throws SQLException {
        ResultSetMetaData metadata = resultSet.getMetaData();
        int columnCount = metadata.getColumnCount();
        ArrayNode columns = JsonNodeFactory.instance.arrayNode();
        for (int index = 1; index <= columnCount; index++) {
            ObjectNode column = columns.addObject();
            column.put("label", metadata.getColumnLabel(index));
            column.put("jdbcType", metadata.getColumnType(index));
        }
        ArrayNode rows = JsonNodeFactory.instance.arrayNode();
        while (resultSet.next()) {
            ArrayNode row = rows.addArray();
            for (int index = 1; index <= columnCount; index++) {
                row.add(objectMapper.valueToTree(readValue(resultSet, metadata.getColumnType(index), index)));
            }
        }
        ObjectNode result = JsonNodeFactory.instance.objectNode();
        result.put("kind", "result_set");
        result.set("columns", columns);
        result.set("rows", rows);
        return result;
    }

    private ObjectNode updateResult(PreparedStatement statement, long rowsAffected) throws SQLException {
        ObjectNode result = JsonNodeFactory.instance.objectNode();
        result.put("kind", "update");
        result.put("rowsAffected", rowsAffected);
        try (ResultSet generatedKeys = statement.getGeneratedKeys()) {
            if (Objects.nonNull(generatedKeys) && generatedKeys.next()) {
                long value = generatedKeys.getLong(1);
                if (!generatedKeys.wasNull()) {
                    result.put("lastInsertId", value);
                }
            }
        } catch (SQLFeatureNotSupportedException ignored) {
            log.debug("JDBC driver does not expose generated keys");
        }
        return result;
    }

    private void bind(PreparedStatement statement, JsonNode parameters) throws SQLException, IOException {
        if (!parameters.isArray()) {
            throw new IllegalArgumentException("params must be an array");
        }
        for (int index = 0; index < parameters.size(); index++) {
            AgentValue value = objectMapper.treeToValue(parameters.get(index), AgentValue.class);
            bindValue(statement, index + 1, value);
        }
    }

    private void bindValue(PreparedStatement statement, int index, AgentValue value) throws SQLException {
        String kind = Objects.requireNonNull(value.kind(), "value kind");
        JsonNode node = value.value();
        switch (kind) {
            case "null" -> statement.setObject(index, null);
            case "bool" -> statement.setBoolean(index, node.asBoolean());
            case "int" -> statement.setLong(index, node.asLong());
            case "float" -> statement.setDouble(index, node.asDouble());
            case "decimal" -> statement.setBigDecimal(index, new BigDecimal(node.asText()));
            case "date" -> statement.setDate(index, Date.valueOf(LocalDate.parse(node.asText())));
            case "time" -> statement.setTime(index, Time.valueOf(LocalTime.parse(node.asText())));
            case "timestamp" -> statement.setTimestamp(index, Timestamp.valueOf(LocalDateTime.parse(node.asText())));
            case "string" -> statement.setString(index, node.asText());
            case "bytes" -> statement.setBytes(index, Base64.getDecoder().decode(node.asText()));
            default -> throw new IllegalArgumentException("unsupported AgentValue kind " + kind);
        }
    }

    private AgentValue readValue(ResultSet resultSet, int jdbcType, int index) throws SQLException {
        Object raw = resultSet.getObject(index);
        if (Objects.isNull(raw)) {
            return new AgentValue("null", null);
        }
        JsonNodeFactory nodes = JsonNodeFactory.instance;
        return switch (jdbcType) {
            case Types.BOOLEAN, Types.BIT -> new AgentValue("bool", nodes.booleanNode(resultSet.getBoolean(index)));
            case Types.TINYINT, Types.SMALLINT, Types.INTEGER, Types.BIGINT ->
                    new AgentValue("int", nodes.numberNode(resultSet.getLong(index)));
            case Types.FLOAT, Types.REAL, Types.DOUBLE ->
                    new AgentValue("float", nodes.numberNode(resultSet.getDouble(index)));
            case Types.NUMERIC, Types.DECIMAL ->
                    new AgentValue("decimal", nodes.textNode(resultSet.getBigDecimal(index).toPlainString()));
            case Types.DATE -> new AgentValue("date", nodes.textNode(resultSet.getDate(index).toLocalDate().toString()));
            case Types.TIME, Types.TIME_WITH_TIMEZONE ->
                    new AgentValue("time", nodes.textNode(resultSet.getTime(index).toLocalTime().toString()));
            case Types.TIMESTAMP, Types.TIMESTAMP_WITH_TIMEZONE -> new AgentValue(
                    "timestamp",
                    nodes.textNode(resultSet.getTimestamp(index).toLocalDateTime().format(TIMESTAMP_FORMAT)));
            case Types.BINARY, Types.VARBINARY, Types.LONGVARBINARY, Types.BLOB ->
                    new AgentValue("bytes", nodes.textNode(Base64.getEncoder().encodeToString(resultSet.getBytes(index))));
            default -> new AgentValue("string", nodes.textNode(resultSet.getString(index)));
        };
    }

    private Connection activeConnection() throws SQLException {
        if (Objects.isNull(connection) || connection.isClosed()) {
            throw new SQLException("JDBC Agent connection is not open");
        }
        return connection;
    }

    private String requiredText(JsonNode payload, String field) {
        Objects.requireNonNull(payload, "payload");
        return Objects.requireNonNull(payload.get(field), field).asText();
    }

    private int[] intArray(JsonNode values) {
        int[] result = new int[values.size()];
        for (int index = 0; index < values.size(); index++) {
            result[index] = values.get(index).asInt();
        }
        return result;
    }

    private String[] stringArray(JsonNode values) {
        String[] result = new String[values.size()];
        for (int index = 0; index < values.size(); index++) {
            result[index] = values.get(index).asText();
        }
        return result;
    }

    private void writeLine(JsonNode value) throws IOException {
        String line = objectMapper.writeValueAsString(value);
        if (line.getBytes(StandardCharsets.UTF_8).length > MAX_FRAME_BYTES) {
            throw new IOException("JSON-RPC response exceeds maximum frame size");
        }
        output.write(line);
        output.newLine();
        output.flush();
    }

    /** 关闭全部 session 和协议流。 */
    @Override
    public void close() throws IOException {
        IOException failure = null;
        try {
            for (AgentSession session : sessions.values()) {
                try {
                    session.connection().close();
                } catch (SQLException exception) {
                    if (Objects.isNull(failure)) {
                        failure = new IOException("failed to close JDBC session", exception);
                    }
                }
            }
            sessions.clear();
        } finally {
            input.close();
            output.close();
        }
        if (Objects.nonNull(failure)) {
            throw failure;
        }
    }
}
