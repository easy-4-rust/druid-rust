package io.github.easy4rust.druid.agent;

import org.junit.jupiter.api.Test;

import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Proxy;
import java.sql.Blob;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.SQLException;
import java.util.Objects;
import java.util.concurrent.atomic.AtomicInteger;
import javax.sql.XAConnection;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

/** 验证 JDBC Blob 会话身份、释放重试以及逻辑连接关闭清理。 */
class AgentBlobSessionTest {

    @Test
    void explicitFreeInvalidatesHandleAndDoesNotDoubleFree() throws Exception {
        try (Connection connection = DriverManager.getConnection("jdbc:h2:mem:blob-session-explicit")) {
            AgentSession session = new AgentSession(connection, null);
            Blob driverBlob = connection.createBlob();
            driverBlob.setBytes(1, new byte[] {1, 2, 3});
            AtomicInteger frees = new AtomicInteger();
            String id = session.registerBlob(countingBlob(driverBlob, frees, false));

            assertEquals(3, session.blob(id).length());
            session.freeBlob(id);
            session.freeBlob(id);
            assertEquals(1, frees.get());
            assertThrows(SQLException.class, () -> session.blob(id));
            session.close();
            assertEquals(1, frees.get());
        }
    }

    @Test
    void failedFreeCanBeRetriedAndSessionCloseDrainsRemainingBlob() throws Exception {
        try (Connection connection = DriverManager.getConnection("jdbc:h2:mem:blob-session-retry")) {
            AgentSession session = new AgentSession(connection, null);
            AtomicInteger retries = new AtomicInteger();
            String retryId = session.registerBlob(countingBlob(connection.createBlob(), retries, true));
            assertThrows(SQLException.class, () -> session.freeBlob(retryId));
            assertNotNull(session.blob(retryId));
            session.freeBlob(retryId);
            assertEquals(2, retries.get());

            AtomicInteger onClose = new AtomicInteger();
            String closeId = session.registerBlob(countingBlob(connection.createBlob(), onClose, false));
            session.close();
            assertEquals(1, onClose.get());
            assertThrows(SQLException.class, () -> session.blob(closeId));
        }
    }

    @Test
    void xaLogicalCloseFreesBlobsBeforeReopen() throws Exception {
        org.h2.jdbcx.JdbcDataSource dataSource = new org.h2.jdbcx.JdbcDataSource();
        dataSource.setURL("jdbc:h2:mem:blob-session-xa");
        XAConnection owner = dataSource.getXAConnection();
        try {
            AgentSession session = new AgentSession(owner, null);
            AtomicInteger frees = new AtomicInteger();
            String id = session.registerBlob(countingBlob(session.connection().createBlob(), frees, false));
            session.closeLogicalConnection();
            assertEquals(1, frees.get());
            assertThrows(SQLException.class, () -> session.blob(id));
            session.reopenLogicalConnection();
            session.close();
            assertEquals(1, frees.get());
        } finally {
            owner.close();
        }
    }

    private Blob countingBlob(Blob delegate, AtomicInteger frees, boolean failFirstFree) {
        return (Blob) Proxy.newProxyInstance(Blob.class.getClassLoader(), new Class<?>[] {Blob.class},
                (proxy, method, arguments) -> {
                    if (Objects.equals(method.getName(), "free")) {
                        int attempt = frees.incrementAndGet();
                        if (failFirstFree && attempt == 1) {
                            throw new SQLException("injected Blob free failure");
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
