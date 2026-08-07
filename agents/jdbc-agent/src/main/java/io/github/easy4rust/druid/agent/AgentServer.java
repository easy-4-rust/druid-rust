package io.github.easy4rust.druid.agent;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.JsonNodeFactory;
import com.fasterxml.jackson.databind.node.ObjectNode;
import lombok.extern.slf4j.Slf4j;

import java.io.BufferedInputStream;
import java.io.BufferedOutputStream;
import java.io.Closeable;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.EOFException;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
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

/** DAP1 有界帧服务端；每个进程只拥有一个物理 JDBC Connection。 */
@Slf4j
public final class AgentServer implements Closeable {

    private static final int PROTOCOL_VERSION = 1;
    private static final int MAX_FRAME_BYTES = 16 * 1024 * 1024;
    private static final DateTimeFormatter TIMESTAMP_FORMAT =
            DateTimeFormatter.ofPattern("yyyy-MM-dd'T'HH:mm:ss.SSSSSSSSS");

    private final DataInputStream input;
    private final DataOutputStream output;
    private final ObjectMapper objectMapper;
    private Connection connection;
    private String validationQuery;

    /**
     * 创建使用指定字节流的服务端。
     *
     * @param input  DAP1 输入流
     * @param output DAP1 输出流
     */
    public AgentServer(InputStream input, OutputStream output) {
        this.input = new DataInputStream(new BufferedInputStream(Objects.requireNonNull(input, "input")));
        this.output = new DataOutputStream(new BufferedOutputStream(Objects.requireNonNull(output, "output")));
        this.objectMapper = new ObjectMapper();
    }

    /** 执行串行请求循环，直到 EOF 或 close 操作。 */
    public void run() throws IOException {
        boolean running = true;
        while (running) {
            byte[] frame;
            try {
                frame = readFrame();
            } catch (EOFException ignored) {
                return;
            }

            AgentRequest request = null;
            AgentResponse response;
            try {
                request = objectMapper.readValue(frame, AgentRequest.class);
                validateRequest(request);
                JsonNode payload = dispatch(request.operation(), request.payload());
                response = AgentResponse.success(request.requestId(), payload);
                running = !"close".equals(request.operation());
            } catch (Exception exception) {
                long requestId = Objects.isNull(request) ? 0 : request.requestId();
                response = AgentResponse.failure(requestId, exception);
                log.debug("JDBC operation failed: requestId={}", requestId, exception);
            }
            writeFrame(objectMapper.writeValueAsBytes(response));
        }
    }

    private void validateRequest(AgentRequest request) {
        Objects.requireNonNull(request, "request");
        Objects.requireNonNull(request.operation(), "operation");
        if (request.protocolVersion() != PROTOCOL_VERSION) {
            throw new IllegalArgumentException("unsupported protocolVersion " + request.protocolVersion());
        }
    }

    private JsonNode dispatch(String operation, JsonNode payload) throws SQLException, IOException {
        return switch (operation) {
            case "connect" -> connect(payload);
            case "exec" -> exec(payload);
            case "fetch" -> fetch(payload);
            case "execute" -> execute(payload);
            case "begin" -> begin();
            case "commit" -> commit();
            case "rollback" -> rollback();
            case "ping" -> ping();
            case "set_auto_commit" -> setAutoCommit(payload);
            case "set_read_only" -> setReadOnly(payload);
            case "set_transaction_isolation" -> setTransactionIsolation(payload);
            case "close" -> closeConnection();
            default -> throw new IllegalArgumentException("unsupported operation " + operation);
        };
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

    private byte[] readFrame() throws IOException {
        int length = input.readInt();
        if (length <= 0 || length > MAX_FRAME_BYTES) {
            throw new IOException("invalid DAP1 frame length " + length);
        }
        byte[] frame = input.readNBytes(length);
        if (frame.length != length) {
            throw new EOFException("incomplete DAP1 frame");
        }
        return frame;
    }

    private void writeFrame(byte[] frame) throws IOException {
        if (frame.length > MAX_FRAME_BYTES) {
            throw new IOException("DAP1 response exceeds maximum frame size");
        }
        output.writeInt(frame.length);
        output.write(frame);
        output.flush();
    }

    /** 关闭物理连接和协议流。 */
    @Override
    public void close() throws IOException {
        try {
            if (Objects.nonNull(connection)) {
                connection.close();
                connection = null;
            }
        } catch (SQLException exception) {
            throw new IOException("failed to close JDBC connection", exception);
        } finally {
            input.close();
            output.close();
        }
    }
}
