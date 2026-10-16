package io.github.easy4rust.druid.agent;

import org.junit.jupiter.api.Test;

import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Proxy;
import java.sql.Array;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.SQLException;
import java.util.Objects;
import java.util.concurrent.atomic.AtomicInteger;
import javax.sql.XAConnection;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

/** 验证 JDBC Array 在 Agent 会话内的身份、释放重试及关闭清理。 */
class AgentArraySessionTest {

    @Test
    void explicitFreeInvalidatesHandleAndDoesNotDoubleFree() throws Exception {
        try (Connection connection = DriverManager.getConnection("jdbc:h2:mem:array-session-explicit")) {
            AgentSession session = new AgentSession(connection, null);
            Array driverArray = connection.createArrayOf("INTEGER", new Object[] {1, null, 3});
            AtomicInteger frees = new AtomicInteger();
            String id = session.registerArray(countingArray(driverArray, frees, false));

            assertEquals(3, ((Object[]) session.array(id).getArray()).length);
            session.freeArray(id);
            session.freeArray(id);
            assertEquals(1, frees.get());
            assertThrows(SQLException.class, () -> session.array(id));
            session.close();
            assertEquals(1, frees.get());
        }
    }

    @Test
    void failedFreeCanBeRetriedAndSessionCloseDrainsRemainingArray() throws Exception {
        try (Connection connection = DriverManager.getConnection("jdbc:h2:mem:array-session-retry")) {
            AgentSession session = new AgentSession(connection, null);
            AtomicInteger retries = new AtomicInteger();
            String retryId = session.registerArray(countingArray(
                    connection.createArrayOf("INTEGER", new Object[] {7}), retries, true));
            assertThrows(SQLException.class, () -> session.freeArray(retryId));
            assertNotNull(session.array(retryId));
            session.freeArray(retryId);
            assertEquals(2, retries.get());

            AtomicInteger onClose = new AtomicInteger();
            String closeId = session.registerArray(countingArray(
                    connection.createArrayOf("INTEGER", new Object[] {8}), onClose, false));
            session.close();
            assertEquals(1, onClose.get());
            assertThrows(SQLException.class, () -> session.array(closeId));
        }
    }

    @Test
    void xaLogicalCloseFreesArraysBeforeReopen() throws Exception {
        org.h2.jdbcx.JdbcDataSource dataSource = new org.h2.jdbcx.JdbcDataSource();
        dataSource.setURL("jdbc:h2:mem:array-session-xa");
        XAConnection owner = dataSource.getXAConnection();
        try {
            AgentSession session = new AgentSession(owner, null);
            AtomicInteger frees = new AtomicInteger();
            String id = session.registerArray(countingArray(
                    session.connection().createArrayOf("INTEGER", new Object[] {9}), frees, false));
            session.closeLogicalConnection();
            assertEquals(1, frees.get());
            assertThrows(SQLException.class, () -> session.array(id));
            session.reopenLogicalConnection();
            session.close();
            assertEquals(1, frees.get());
        } finally {
            owner.close();
        }
    }

    private Array countingArray(Array delegate, AtomicInteger frees, boolean failFirstFree) {
        return (Array) Proxy.newProxyInstance(Array.class.getClassLoader(), new Class<?>[] {Array.class},
                (proxy, method, arguments) -> {
                    if (Objects.equals(method.getName(), "free")) {
                        int attempt = frees.incrementAndGet();
                        if (failFirstFree && attempt == 1) {
                            throw new SQLException("injected Array free failure");
                        }
                    }
                    try {
                        return method.invoke(delegate, arguments);
                    } catch (InvocationTargetException exception) {
                        throw exception.getCause();
                    }
                });
    }
}
