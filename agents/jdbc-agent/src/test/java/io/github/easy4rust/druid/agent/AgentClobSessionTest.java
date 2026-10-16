package io.github.easy4rust.druid.agent;

import org.junit.jupiter.api.Test;

import java.io.FilterReader;
import java.io.FilterWriter;
import java.io.IOException;
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
import java.util.concurrent.atomic.AtomicInteger;
import javax.sql.XAConnection;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

/** 验证 JDBC Clob 会话身份、释放重试及逻辑连接关闭。 */
class AgentClobSessionTest {

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
            String readerId = session.registerClobReader(clobId, reader);
            String writerId = session.registerClobWriter(clobId, writer);

            SQLException thrown = assertThrows(SQLException.class, () -> session.freeClob(clobId));
            assertEquals(1, thrown.getSuppressed().length);
            assertEquals(3, closeOrder.size());
            assertEquals("clob", closeOrder.get(2));
            assertThrows(SQLException.class, () -> session.clob(clobId));
            assertThrows(SQLException.class, () -> session.clobReader(readerId));
            assertThrows(SQLException.class, () -> session.clobWriter(writerId));
            session.freeClob(clobId);
            assertEquals(3, closeOrder.size());
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
            session.closeLogicalConnection();
            assertEquals(1, frees.get());
            assertThrows(SQLException.class, () -> session.clob(id));
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
