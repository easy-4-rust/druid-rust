package io.github.easy4rust.druid.agent;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.node.JsonNodeFactory;

import java.sql.ResultSet;
import java.sql.SQLException;
import java.sql.Types;
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
}
