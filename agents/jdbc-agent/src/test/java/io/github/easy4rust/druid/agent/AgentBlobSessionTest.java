package io.github.easy4rust.druid.agent;

import org.junit.jupiter.api.Test;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.FilterInputStream;
import java.io.FilterOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Proxy;
import java.sql.Blob;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.SQLException;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.atomic.AtomicInteger;
import javax.sql.XAConnection;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

/** 验证 JDBC Blob 会话身份、释放重试以及逻辑连接关闭清理。 */
class AgentBlobSessionTest {

    @Test
    void childStreamCloseFailuresAreAggregatedBeforeParentFree() throws Exception {
        try (Connection connection = DriverManager.getConnection("jdbc:h2:mem:blob-session-streams")) {
            AgentSession session = new AgentSession(connection, null);
            List<String> closeOrder = new ArrayList<>();
            Blob driverBlob = connection.createBlob();
            Blob observedBlob = (Blob) Proxy.newProxyInstance(Blob.class.getClassLoader(),
                    new Class<?>[] {Blob.class}, (proxy, method, arguments) -> {
                        if (Objects.equals(method.getName(), "free")) {
                            closeOrder.add("blob");
                        }
                        try {
                            return method.invoke(driverBlob, arguments);
                        } catch (InvocationTargetException exception) {
                            throw exception.getCause();
                        }
                    });
            String blobId = session.registerBlob(observedBlob);
            InputStream input = new FilterInputStream(new ByteArrayInputStream(new byte[] {1})) {
                @Override
                public void close() throws IOException {
                    closeOrder.add("input");
                    throw new IOException("input close failed");
                }
            };
            OutputStream output = new FilterOutputStream(new ByteArrayOutputStream()) {
                @Override
                public void close() throws IOException {
                    closeOrder.add("output");
                    throw new IOException("output close failed");
                }
            };
            String inputId = session.registerBlobInputStream(blobId, input);
            String outputId = session.registerBlobOutputStream(blobId, output);
            assertNotNull(session.blobInputStream(inputId));
            assertNotNull(session.blobOutputStream(outputId));

            SQLException thrown = assertThrows(SQLException.class, () -> session.freeBlob(blobId));
            assertEquals(1, thrown.getSuppressed().length);
            assertEquals(3, closeOrder.size());
            assertEquals("blob", closeOrder.get(2));
            assertThrows(SQLException.class, () -> session.blob(blobId));
            assertThrows(SQLException.class, () -> session.blobInputStream(inputId));
            assertThrows(SQLException.class, () -> session.blobOutputStream(outputId));
            session.freeBlob(blobId);
            assertEquals(3, closeOrder.size());
            session.close();
        }
    }

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
