package io.github.easy4rust.druid.agent;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.node.JsonNodeFactory;

import java.sql.ResultSet;
import java.sql.Date;
import java.sql.SQLException;
import java.sql.SQLFeatureNotSupportedException;
import java.sql.Time;
import java.sql.Timestamp;
import java.sql.Types;
import java.math.BigDecimal;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.time.format.DateTimeFormatter;
import java.util.Base64;
import java.util.Objects;

/**
 * Rust 与 JDBC 之间的显式标量值。
 *
 * @param kind  值类型
 * @param value 类型对应的 JSON 值；SQL NULL 时为空
 */
public record AgentValue(String kind, JsonNode value) {

    private static final DateTimeFormatter TIMESTAMP_FORMAT =
            DateTimeFormatter.ofPattern("yyyy-MM-dd'T'HH:mm:ss.SSSSSSSSS");

    /** 从当前 JDBC ResultSet 行无损读取一个带类型标签的标量。 */
    public static AgentValue fromResultSet(ResultSet resultSet, int jdbcType, int index)
            throws SQLException {
        Object raw = resultSet.getObject(index);
        if (Objects.isNull(raw)) {
            return new AgentValue("null", null);
        }
        JsonNodeFactory nodes = JsonNodeFactory.instance;
        return switch (jdbcType) {
            case Types.BOOLEAN, Types.BIT ->
                    new AgentValue("bool", nodes.booleanNode(resultSet.getBoolean(index)));
            case Types.TINYINT, Types.SMALLINT, Types.INTEGER, Types.BIGINT ->
                    new AgentValue("int", nodes.numberNode(resultSet.getLong(index)));
            case Types.FLOAT, Types.REAL, Types.DOUBLE ->
                    new AgentValue("float", nodes.numberNode(resultSet.getDouble(index)));
            case Types.NUMERIC, Types.DECIMAL -> new AgentValue(
                    "decimal", nodes.textNode(resultSet.getBigDecimal(index).toPlainString()));
            case Types.DATE -> new AgentValue(
                    "date", nodes.textNode(resultSet.getDate(index).toLocalDate().toString()));
            case Types.TIME, Types.TIME_WITH_TIMEZONE -> new AgentValue(
                    "time", nodes.textNode(resultSet.getTime(index).toLocalTime().toString()));
            case Types.TIMESTAMP, Types.TIMESTAMP_WITH_TIMEZONE -> new AgentValue(
                    "timestamp",
                    nodes.textNode(resultSet.getTimestamp(index)
                            .toLocalDateTime()
                            .format(TIMESTAMP_FORMAT)));
            case Types.BINARY, Types.VARBINARY, Types.LONGVARBINARY, Types.BLOB -> new AgentValue(
                    "bytes",
                    nodes.textNode(Base64.getEncoder().encodeToString(resultSet.getBytes(index))));
            default -> new AgentValue("string", nodes.textNode(resultSet.getString(index)));
        };
    }

    /** 将协议元素转换为传给 Connection#createArrayOf 的 JDBC 对象。 */
    public Object toJdbcObject() throws SQLException {
        if (Objects.isNull(kind)) {
            throw new SQLException("JDBC Agent array element kind is missing");
        }
        if (!"null".equals(kind) && Objects.isNull(value)) {
            throw new SQLException("JDBC Agent array element value is missing");
        }
        return switch (kind) {
            case "null" -> null;
            case "bool" -> value.booleanValue();
            case "int" -> value.longValue();
            case "float" -> value.doubleValue();
            case "decimal" -> new BigDecimal(value.asText());
            case "date" -> Date.valueOf(LocalDate.parse(value.asText()));
            case "time" -> Time.valueOf(LocalTime.parse(value.asText()));
            case "timestamp" -> Timestamp.valueOf(LocalDateTime.parse(value.asText()));
            case "string" -> value.asText();
            case "bytes" -> Base64.getDecoder().decode(value.asText());
            default -> throw new SQLFeatureNotSupportedException(
                    "JDBC Agent array element kind " + kind + " is not supported", "0A000");
        };
    }

    /** 将驱动 Array#getArray 元素按其 SQL 基础类型编码，保留 NULL。 */
    public static AgentValue fromArrayElement(Object raw, int jdbcType) throws SQLException {
        if (Objects.isNull(raw)) {
            return new AgentValue("null", null);
        }
        JsonNodeFactory nodes = JsonNodeFactory.instance;
        return switch (jdbcType) {
            case Types.BOOLEAN, Types.BIT ->
                    new AgentValue("bool", nodes.booleanNode((Boolean) raw));
            case Types.TINYINT, Types.SMALLINT, Types.INTEGER, Types.BIGINT ->
                    new AgentValue("int", nodes.numberNode(((Number) raw).longValue()));
            case Types.FLOAT, Types.REAL, Types.DOUBLE ->
                    new AgentValue("float", nodes.numberNode(((Number) raw).doubleValue()));
            case Types.NUMERIC, Types.DECIMAL ->
                    new AgentValue("decimal", nodes.textNode(((BigDecimal) raw).toPlainString()));
            case Types.DATE -> new AgentValue("date", nodes.textNode(((Date) raw).toLocalDate().toString()));
            case Types.TIME -> new AgentValue("time", nodes.textNode(((Time) raw).toLocalTime().toString()));
            case Types.TIMESTAMP -> new AgentValue("timestamp", nodes.textNode(
                    ((Timestamp) raw).toLocalDateTime().format(TIMESTAMP_FORMAT)));
            case Types.BINARY, Types.VARBINARY, Types.LONGVARBINARY ->
                    new AgentValue("bytes", nodes.textNode(Base64.getEncoder().encodeToString((byte[]) raw)));
            case Types.CHAR, Types.VARCHAR, Types.LONGVARCHAR, Types.NCHAR, Types.NVARCHAR,
                    Types.LONGNVARCHAR -> new AgentValue("string", nodes.textNode((String) raw));
            default -> throw new SQLFeatureNotSupportedException(
                    "JDBC Agent Array element SQL type " + jdbcType + " is not supported", "0A000");
        };
    }
}
