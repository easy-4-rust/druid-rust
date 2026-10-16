package io.github.easy4rust.druid.agent;

import org.junit.jupiter.api.Test;

import java.io.FilterWriter;
import java.io.IOException;
import java.io.Reader;
import java.io.StringWriter;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Proxy;
import java.sql.Clob;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.NClob;
import java.sql.SQLException;
import java.util.Objects;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import javax.sql.XAConnection;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;

/** 验证 NClob 与普通 Clob 的会话身份、释放重试和关闭语义。 */
class AgentNClobSessionTest {

    @Test
    void cancellationClosesNationalCharacterChild() throws Exception {
        try (Connection connection = DriverManager.getConnection("jdbc:h2:mem:nclob-read-cancel")) {
            AgentSession session = new AgentSession(connection, null);
            String id = session.registerNClob(connection.createNClob());
            CountDownLatch reading = new CountDownLatch(1);
            CountDownLatch closed = new CountDownLatch(1);
            Reader blocked = new Reader() {
                @Override
                public int read(char[] buffer, int offset, int length) throws IOException {
                    reading.countDown();
                    try {
                        if (!closed.await(5, TimeUnit.SECONDS)) {
                            throw new IOException("NClob read was not cancelled");
                        }
                    } catch (InterruptedException exception) {
                        Thread.currentThread().interrupt();
                        throw new IOException("NClob read interrupted", exception);
                    }
                    throw new IOException("NClob reader closed by cancellation");
                }

                @Override
                public void close() {
                    closed.countDown();
                }
            };
            String streamId = session.registerClobReader(id, blocked);
            ExecutorService worker = Executors.newSingleThreadExecutor();
            try {
                Future<?> pending = worker.submit(() -> {
                    session.activateClobRead(177L, streamId);
                    try {
                        session.clobReader(streamId).read(new char[1]);
                    } finally {
                        session.finishClobRead(177L);
                    }
                    return null;
                });
                assertEquals(true, reading.await(5, TimeUnit.SECONDS));
                assertEquals(true, session.cancel(177L));
                assertThrows(SQLException.class, () -> session.clobReader(streamId));
                assertThrows(java.util.concurrent.ExecutionException.class,
                        () -> pending.get(5, TimeUnit.SECONDS));
                assertEquals(false, session.cancel(177L));
            } finally {
                closed.countDown();
                worker.shutdownNow();
                session.close();
            }
        }
    }

    @Test
    void nationalHandleIsDistinctAndFailedFreeRemainsRetryable() throws Exception {
        try (Connection connection = DriverManager.getConnection("jdbc:h2:mem:nclob-session-retry")) {
            AgentSession session = new AgentSession(connection, null);
            AtomicInteger frees = new AtomicInteger();
            String ordinaryId = session.registerClob(connection.createClob());
            NClob national = countingNClob(connection.createClob(), frees, true);
            String nationalId = session.registerNClob(national);

            assertThrows(SQLException.class, () -> session.nClob(ordinaryId));
            assertSame(national, session.nClob(nationalId));
            assertSame(national, session.clob(nationalId));
            assertThrows(SQLException.class, () -> session.freeNClob(nationalId));
            assertSame(national, session.nClob(nationalId));
            session.freeNClob(nationalId);
            session.freeNClob(nationalId);
            assertEquals(2, frees.get());
            assertThrows(SQLException.class, () -> session.nClob(nationalId));
            session.close();
            assertEquals(2, frees.get());
        }
    }

    @Test
    void xaLogicalCloseInvalidatesNClobAndChildBeforeReopen() throws Exception {
        org.h2.jdbcx.JdbcDataSource dataSource = new org.h2.jdbcx.JdbcDataSource();
        dataSource.setURL("jdbc:h2:mem:nclob-session-xa");
        XAConnection owner = dataSource.getXAConnection();
        try {
            AgentSession session = new AgentSession(owner, null);
            AtomicInteger frees = new AtomicInteger();
            AtomicInteger writerCloses = new AtomicInteger();
            NClob national = countingNClob(session.connection().createClob(), frees, false);
            String id = session.registerNClob(national);
            String writerId = session.registerClobWriter(id, new FilterWriter(new StringWriter()) {
                @Override
                public void close() throws IOException {
                    writerCloses.incrementAndGet();
                    super.close();
                }
            });

            session.closeLogicalConnection();
            assertEquals(1, writerCloses.get());
            assertEquals(1, frees.get());
            assertThrows(SQLException.class, () -> session.nClob(id));
            assertThrows(SQLException.class, () -> session.clobWriter(writerId));
            session.reopenLogicalConnection();
            session.close();
            assertEquals(1, frees.get());
        } finally {
            owner.close();
        }
    }

    private NClob countingNClob(Clob delegate, AtomicInteger frees, boolean failFirstFree) {
        return (NClob) Proxy.newProxyInstance(NClob.class.getClassLoader(),
                new Class<?>[] {NClob.class}, (proxy, method, arguments) -> {
                    if (Objects.equals(method.getName(), "free")) {
                        int attempt = frees.incrementAndGet();
                        if (failFirstFree && attempt == 1) {
                            throw new SQLException("injected NClob free failure");
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
