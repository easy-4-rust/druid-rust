package io.github.easy4rust.druid.agent;

import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.Test;

import java.lang.reflect.Proxy;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.ResultSetMetaData;
import java.sql.SQLException;
import java.util.Arrays;
import java.util.Set;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.stream.Collectors;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** 验证 JDBC Agent 资源关闭级联完整保留 SQL 异常。 */
class AgentCloseFailureTest {

    @Test
    void cursorClosePreservesResultSetFailureWhenTemporaryStatementAlsoFails() throws Exception {
        AtomicInteger resultSetCloses = new AtomicInteger();
        AtomicInteger statementCloses = new AtomicInteger();
        SQLException resultSetFailure = new SQLException("result-set close failed");
        SQLException statementFailure = new SQLException("temporary statement close failed");
        AgentCursor cursor = new AgentCursor(
                statement(statementFailure, statementCloses),
                resultSet(resultSetFailure, resultSetCloses),
                true, null, new ObjectMapper());

        SQLException thrown = assertThrows(SQLException.class, cursor::close);
        assertEquals(resultSetFailure, thrown);
        assertEquals(1, thrown.getSuppressed().length);
        assertEquals(statementFailure, thrown.getSuppressed()[0]);
        assertEquals(1, resultSetCloses.get());
        assertEquals(1, statementCloses.get());
        cursor.close();
        assertEquals(1, resultSetCloses.get());
    }

    @Test
    void preparedStatementClosePreservesAllCursorFailuresAndRemovesResources() throws Exception {
        try (Connection connection = DriverManager.getConnection("jdbc:h2:mem:close-failure-contract")) {
            AgentSession session = new AgentSession(connection, null);
            AtomicInteger statementCloses = new AtomicInteger();
            AtomicInteger firstCursorCloses = new AtomicInteger();
            AtomicInteger secondCursorCloses = new AtomicInteger();
            SQLException statementFailure = new SQLException("prepared statement close failed");
            SQLException firstCursorFailure = new SQLException("first cursor close failed");
            SQLException secondCursorFailure = new SQLException("second cursor close failed");
            PreparedStatement statement = statement(statementFailure, statementCloses);
            String statementId = session.registerPreparedStatement(statement);
            session.registerCursor("first", new AgentCursor(statement,
                    resultSet(firstCursorFailure, firstCursorCloses), false, statementId, new ObjectMapper()));
            session.registerCursor("second", new AgentCursor(statement,
                    resultSet(secondCursorFailure, secondCursorCloses), false, statementId, new ObjectMapper()));

            SQLException thrown = assertThrows(SQLException.class,
                    () -> session.closePreparedStatement(statementId));
            assertTrue(thrown == firstCursorFailure || thrown == secondCursorFailure);
            assertEquals(2, thrown.getSuppressed().length);
            Set<SQLException> observed = Arrays.stream(thrown.getSuppressed())
                    .map(SQLException.class::cast).collect(Collectors.toSet());
            observed.add(thrown);
            assertEquals(Set.of(firstCursorFailure, secondCursorFailure, statementFailure), observed);
            assertEquals(1, firstCursorCloses.get());
            assertEquals(1, secondCursorCloses.get());
            assertEquals(1, statementCloses.get());
            assertThrows(SQLException.class, () -> session.cursor("first"));
            assertThrows(SQLException.class, () -> session.cursor("second"));
            assertThrows(SQLException.class, () -> session.preparedStatement(statementId));
            session.closePreparedStatement(statementId);
            assertEquals(1, statementCloses.get());
        }
    }

    private PreparedStatement statement(SQLException failure, AtomicInteger closes) {
        return (PreparedStatement) Proxy.newProxyInstance(getClass().getClassLoader(),
                new Class<?>[] {PreparedStatement.class}, (proxy, method, arguments) -> {
                    if (method.getName().equals("close")) {
                        closes.incrementAndGet();
                        throw failure;
                    }
                    throw new UnsupportedOperationException(method.getName());
                });
    }

    private ResultSet resultSet(SQLException failure, AtomicInteger closes) {
        ResultSetMetaData metadata = (ResultSetMetaData) Proxy.newProxyInstance(
                getClass().getClassLoader(), new Class<?>[] {ResultSetMetaData.class},
                (proxy, method, arguments) -> {
                    if (method.getName().equals("getColumnCount")) {
                        return 0;
                    }
                    throw new UnsupportedOperationException(method.getName());
                });
        return (ResultSet) Proxy.newProxyInstance(getClass().getClassLoader(),
                new Class<?>[] {ResultSet.class}, (proxy, method, arguments) -> {
                    if (method.getName().equals("getMetaData")) {
                        return metadata;
                    }
                    if (method.getName().equals("close")) {
                        closes.incrementAndGet();
                        throw failure;
                    }
                    throw new UnsupportedOperationException(method.getName());
                });
    }
}
