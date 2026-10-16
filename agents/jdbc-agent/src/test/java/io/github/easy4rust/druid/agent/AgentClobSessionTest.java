package io.github.easy4rust.druid.agent;

import org.junit.jupiter.api.Test;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.FilterInputStream;
import java.io.FilterOutputStream;
import java.io.FilterReader;
import java.io.FilterWriter;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.io.Reader;
import java.io.StringReader;
import java.io.StringWriter;
import java.io.Writer;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Proxy;
import java.sql.Clob;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.SQLException;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import javax.sql.XAConnection;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

/** 验证 JDBC Clob 会话身份、释放重试及逻辑连接关闭。 */
class AgentClobSessionTest {

    @Test
    void cancellingAnActiveClobReadClosesItsChildHandle() throws Exception {
        try (Connection connection = DriverManager.getConnection("jdbc:h2:mem:clob-read-cancel")) {
            AgentSession session = new AgentSession(connection, null);
            String clobId = session.registerClob(connection.createClob());
            CountDownLatch reading = new CountDownLatch(1);
            CountDownLatch closed = new CountDownLatch(1);
            Reader blocked = new Reader() {
                @Override
                public int read(char[] buffer, int offset, int length) throws IOException {
                    reading.countDown();
                    try {
                        if (!closed.await(5, TimeUnit.SECONDS)) {
                            throw new IOException("read was not cancelled");
                        }
                    } catch (InterruptedException exception) {
                        Thread.currentThread().interrupt();
                        throw new IOException("read interrupted", exception);
                    }
                    throw new IOException("reader closed by cancellation");
                }

                @Override
                public void close() {
                    closed.countDown();
                }
            };
            String streamId = session.registerClobReader(clobId, blocked);
            ExecutorService worker = Executors.newSingleThreadExecutor();
            try {
                Future<?> pending = worker.submit(() -> {
                    session.activateClobRead(77L, streamId);
                    try {
                        session.clobReader(streamId).read(new char[1]);
                    } finally {
                        session.finishClobRead(77L);
                    }
                    return null;
                });
                assertEquals(true, reading.await(5, TimeUnit.SECONDS));
                assertEquals(true, session.cancel(77L));
                assertThrows(SQLException.class, () -> session.clobReader(streamId));
                assertThrows(java.util.concurrent.ExecutionException.class,
                        () -> pending.get(5, TimeUnit.SECONDS));
                assertEquals(false, session.cancel(77L));
            } finally {
                closed.countDown();
                worker.shutdownNow();
                session.close();
            }
        }
    }

    @Test
    void childCloseFailuresAreAggregatedBeforeParentFree() throws Exception {
        try (Connection connection = DriverManager.getConnection("jdbc:h2:mem:clob-session-streams")) {
            AgentSession session = new AgentSession(connection, null);
            List<String> closeOrder = new ArrayList<>();
            Clob driver = connection.createClob();
            Clob observed = (Clob) Proxy.newProxyInstance(Clob.class.getClassLoader(),
                    new Class<?>[] {Clob.class}, (proxy, method, arguments) -> {
                        if (Objects.equals(method.getName(), "free")) {
                            closeOrder.add("clob");
                        }
                        try {
                            return method.invoke(driver, arguments);
                        } catch (InvocationTargetException exception) {
                            throw exception.getCause();
                        }
                    });
            String clobId = session.registerClob(observed);
            Reader reader = new FilterReader(new StringReader("a")) {
                @Override
                public void close() throws IOException {
                    closeOrder.add("reader");
                    throw new IOException("reader close failed");
                }
            };
            Writer writer = new FilterWriter(new StringWriter()) {
                @Override
                public void close() throws IOException {
                    closeOrder.add("writer");
                    throw new IOException("writer close failed");
                }
            };
            InputStream input = new FilterInputStream(new ByteArrayInputStream(new byte[] {65})) {
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
            String readerId = session.registerClobReader(clobId, reader);
            String writerId = session.registerClobWriter(clobId, writer);
            String inputId = session.registerClobInputStream(clobId, input);
            String outputId = session.registerClobOutputStream(clobId, output);

            SQLException thrown = assertThrows(SQLException.class, () -> session.freeClob(clobId));
            assertEquals(3, thrown.getSuppressed().length);
            assertEquals(5, closeOrder.size());
            assertEquals("clob", closeOrder.get(4));
            assertThrows(SQLException.class, () -> session.clob(clobId));
            assertThrows(SQLException.class, () -> session.clobReader(readerId));
            assertThrows(SQLException.class, () -> session.clobWriter(writerId));
            assertThrows(SQLException.class, () -> session.clobInputStream(inputId));
            assertThrows(SQLException.class, () -> session.clobOutputStream(outputId));
            session.freeClob(clobId);
            assertEquals(5, closeOrder.size());
            session.close();
        }
    }

    @Test
    void explicitFreeIsIdempotentAndInvalidatesHandle() throws Exception {
        try (Connection connection = DriverManager.getConnection("jdbc:h2:mem:clob-session-explicit")) {
            AgentSession session = new AgentSession(connection, null);
            AtomicInteger frees = new AtomicInteger();
            Clob driver = connection.createClob();
            driver.setString(1, "A😀Z");
            String id = session.registerClob(countingClob(driver, frees, false));

            assertEquals(4, session.clob(id).length());
            session.freeClob(id);
            session.freeClob(id);
            assertEquals(1, frees.get());
            assertThrows(SQLException.class, () -> session.clob(id));
            session.close();
            assertEquals(1, frees.get());
        }
    }

    @Test
    void failedFreeCanBeRetriedAndSessionCloseDrainsRemainingClob() throws Exception {
        try (Connection connection = DriverManager.getConnection("jdbc:h2:mem:clob-session-retry")) {
            AgentSession session = new AgentSession(connection, null);
            AtomicInteger retries = new AtomicInteger();
            String retryId = session.registerClob(countingClob(connection.createClob(), retries, true));
            assertThrows(SQLException.class, () -> session.freeClob(retryId));
            assertNotNull(session.clob(retryId));
            session.freeClob(retryId);
            assertEquals(2, retries.get());

            AtomicInteger onClose = new AtomicInteger();
            String closeId = session.registerClob(countingClob(connection.createClob(), onClose, false));
            session.close();
            assertEquals(1, onClose.get());
            assertThrows(SQLException.class, () -> session.clob(closeId));
        }
    }

    @Test
    void xaLogicalCloseFreesClobBeforeReopen() throws Exception {
        org.h2.jdbcx.JdbcDataSource dataSource = new org.h2.jdbcx.JdbcDataSource();
        dataSource.setURL("jdbc:h2:mem:clob-session-xa");
        XAConnection owner = dataSource.getXAConnection();
        try {
            AgentSession session = new AgentSession(owner, null);
            AtomicInteger frees = new AtomicInteger();
            String id = session.registerClob(countingClob(session.connection().createClob(), frees, false));
            AtomicInteger writerCloses = new AtomicInteger();
            String writerId = session.registerClobWriter(id, new FilterWriter(new StringWriter()) {
                @Override
                public void close() throws IOException {
                    writerCloses.incrementAndGet();
                    super.close();
                }
            });
            session.closeLogicalConnection();
            assertEquals(1, frees.get());
            assertEquals(1, writerCloses.get());
            assertThrows(SQLException.class, () -> session.clob(id));
            assertThrows(SQLException.class, () -> session.clobWriter(writerId));
            session.reopenLogicalConnection();
            session.close();
            assertEquals(1, frees.get());
        } finally {
            owner.close();
        }
    }

    private Clob countingClob(Clob delegate, AtomicInteger frees, boolean failFirstFree) {
        return (Clob) Proxy.newProxyInstance(Clob.class.getClassLoader(), new Class<?>[] {Clob.class},
                (proxy, method, arguments) -> {
                    if (Objects.equals(method.getName(), "free")) {
                        int attempt = frees.incrementAndGet();
                        if (failFirstFree && attempt == 1) {
                            throw new SQLException("injected Clob free failure");
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
