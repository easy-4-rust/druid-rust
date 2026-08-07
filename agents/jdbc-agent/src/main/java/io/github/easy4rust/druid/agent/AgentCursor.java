package io.github.easy4rust.druid.agent;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.JsonNodeFactory;
import com.fasterxml.jackson.databind.node.ObjectNode;

import java.io.IOException;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.ResultSetMetaData;
import java.sql.SQLException;
import java.util.Objects;

/** 持有 JDBC ResultSet 并按行数和响应字节上限读取的远程游标。 */
public final class AgentCursor implements AutoCloseable {

    private final PreparedStatement statement;
    private final ResultSet resultSet;
    private final ResultSetMetaData metadata;
    private final ArrayNode columns;
    private final boolean closeStatement;
    private final String statementId;
    private final ObjectMapper objectMapper;
    private ArrayNode pendingRow;
    private boolean exhausted;
    private boolean closed;

    /** 创建结果集游标。 */
    public AgentCursor(
            PreparedStatement statement,
            ResultSet resultSet,
            boolean closeStatement,
            String statementId,
            ObjectMapper objectMapper) throws SQLException {
        this.statement = Objects.requireNonNull(statement, "statement");
        this.resultSet = Objects.requireNonNull(resultSet, "resultSet");
        this.metadata = resultSet.getMetaData();
        this.closeStatement = closeStatement;
        this.statementId = statementId;
        this.objectMapper = Objects.requireNonNull(objectMapper, "objectMapper");
        this.columns = columns(metadata);
    }

    /** 返回底层 Statement，供 requestId 取消表关联。 */
    public PreparedStatement statement() {
        return statement;
    }

    /** 判断游标是否引用指定远程预编译语句。 */
    public boolean referencesStatement(String candidateStatementId) {
        return Objects.equals(statementId, candidateStatementId);
    }

    /** 读取一页结果，并保证序列化主体不超过调用方给出的字节预算。 */
    public ObjectNode fetchPage(String cursorId, int pageSize, int maxResponseBytes)
            throws SQLException, IOException {
        ensureOpen();
        ArrayNode rows = JsonNodeFactory.instance.arrayNode();
        while (rows.size() < pageSize) {
            ArrayNode row = nextRow();
            if (Objects.isNull(row)) {
                exhausted = true;
                break;
            }
            rows.add(row);
            ObjectNode candidate = page(cursorId, rows, true);
            if (objectMapper.writeValueAsBytes(candidate).length > maxResponseBytes) {
                rows.remove(rows.size() - 1);
                pendingRow = row;
                if (rows.isEmpty()) {
                    throw new IOException("single JDBC row exceeds maximum response size");
                }
                break;
            }
        }
        if (!exhausted && Objects.isNull(pendingRow) && rows.size() >= pageSize) {
            pendingRow = nextRow();
            exhausted = Objects.isNull(pendingRow);
        }
        ObjectNode result = page(cursorId, rows, !exhausted);
        if (exhausted) {
            close();
        }
        return result;
    }

    /** 返回当前页之后是否仍有数据。 */
    public boolean hasMore() {
        return !exhausted;
    }

    private ArrayNode nextRow() throws SQLException {
        if (Objects.nonNull(pendingRow)) {
            ArrayNode row = pendingRow;
            pendingRow = null;
            return row;
        }
        if (!resultSet.next()) {
            return null;
        }
        ArrayNode row = JsonNodeFactory.instance.arrayNode();
        for (int index = 1; index <= metadata.getColumnCount(); index++) {
            row.add(objectMapper.valueToTree(AgentValue.fromResultSet(
                    resultSet, metadata.getColumnType(index), index)));
        }
        return row;
    }

    private ObjectNode page(String cursorId, ArrayNode rows, boolean hasMore) {
        ObjectNode result = JsonNodeFactory.instance.objectNode();
        result.put("kind", "result_set");
        if (hasMore) {
            result.put("cursorId", cursorId);
        } else {
            result.putNull("cursorId");
        }
        result.set("columns", columns.deepCopy());
        result.set("rows", rows);
        result.put("hasMore", hasMore);
        return result;
    }

    private ArrayNode columns(ResultSetMetaData resultSetMetaData) throws SQLException {
        ArrayNode result = JsonNodeFactory.instance.arrayNode();
        for (int index = 1; index <= resultSetMetaData.getColumnCount(); index++) {
            ObjectNode column = result.addObject();
            column.put("label", resultSetMetaData.getColumnLabel(index));
            column.put("jdbcType", resultSetMetaData.getColumnType(index));
        }
        return result;
    }

    private void ensureOpen() throws SQLException {
        if (closed) {
            throw new SQLException("JDBC Agent cursor is closed");
        }
    }

    /** 关闭 ResultSet；按创建模式决定是否一并关闭临时 Statement。 */
    @Override
    public void close() throws SQLException {
        if (closed) {
            return;
        }
        closed = true;
        SQLException failure = null;
        try {
            resultSet.close();
        } catch (SQLException exception) {
            failure = exception;
        }
        if (closeStatement) {
            try {
                statement.close();
            } catch (SQLException exception) {
                failure = exception;
            }
        }
        if (Objects.nonNull(failure)) {
            throw failure;
        }
    }
}
