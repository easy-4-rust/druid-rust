package io.github.easy4rust.druid.agent;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.JsonNodeFactory;
import com.fasterxml.jackson.databind.node.ObjectNode;
import org.junit.jupiter.api.Test;

import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.Reader;
import java.io.OutputStreamWriter;
import java.io.PipedInputStream;
import java.io.PipedOutputStream;
import java.nio.charset.StandardCharsets;
import java.net.SocketTimeoutException;
import java.lang.reflect.Field;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.SQLIntegrityConstraintViolationException;
import java.sql.SQLTransientConnectionException;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.sql.Statement;
import java.util.Base64;
import java.util.Map;
import javax.sql.XAConnection;
import javax.transaction.xa.XAResource;
import javax.transaction.xa.Xid;
import java.util.Arrays;
import java.util.Objects;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** JDBC Agent JSON-RPC、多 session 与真实 H2 JDBC 驱动的最小契约测试。 */
class AgentServerTest {

    private final ObjectMapper objectMapper = new ObjectMapper();

    @Test
    void cancelsOnlyAnActiveRequestStatement() throws Exception {
        try (Connection connection = DriverManager.getConnection("jdbc:h2:mem:cancel-contract");
             Statement statement = connection.createStatement()) {
            AgentSession session = new AgentSession(connection, null);
            assertEquals(false, session.cancel(17));
            session.activate(17, statement);
            assertEquals(true, session.cancel(17));
            session.deactivate(17);
            assertEquals(false, session.cancel(17));
        }
    }

    @Test
    void h2RecoveryCapabilityCheckDoesNotMistakeMissingBranchForRecovery() throws Exception {
        org.h2.jdbcx.JdbcDataSource dataSource = new org.h2.jdbcx.JdbcDataSource();
        dataSource.setURL("jdbc:h2:mem:xa-recovery;DB_CLOSE_DELAY=-1");
        dataSource.setUser("sa");
        dataSource.setPassword("");
        try (Connection setup = DriverManager.getConnection(
                "jdbc:h2:mem:xa-recovery;DB_CLOSE_DELAY=-1", "sa", "");
             Statement statement = setup.createStatement()) {
            statement.execute("CREATE TABLE recovered_xa(id INT PRIMARY KEY, payload VARCHAR(32))");
        }

        Xid xid = new TestXid(0x44525549, new byte[] {1, 2, 3}, new byte[] {4, 5});
        XAConnection owner = dataSource.getXAConnection();
        try {
            XAResource resource = owner.getXAResource();
            resource.start(xid, XAResource.TMNOFLAGS);
            try (Connection logical = owner.getConnection();
                 PreparedStatement statement = logical.prepareStatement(
                     "INSERT INTO recovered_xa(id, payload) VALUES (?, ?)")) {
                statement.setInt(1, 1);
                statement.setString(2, "prepared");
                assertEquals(1, statement.executeUpdate());
            }
            resource.end(xid, XAResource.TMSUCCESS);
            assertEquals(XAResource.XA_OK, resource.prepare(xid));
        } finally {
            owner.close();
        }

        XAConnection recoveryOwner = dataSource.getXAConnection();
        try {
            XAResource recovery = recoveryOwner.getXAResource();
            Xid[] recovered = recovery.recover(XAResource.TMSTARTRSCAN | XAResource.TMENDRSCAN);
            assertTrue(Arrays.stream(recovered).noneMatch(candidate -> sameXid(xid, candidate)),
                    "H2 2.3.232 loses this prepared branch when the original XAConnection closes");
        } finally {
            recoveryOwner.close();
        }
    }

    private boolean sameXid(Xid expected, Xid actual) {
        return expected.getFormatId() == actual.getFormatId()
                && Arrays.equals(expected.getGlobalTransactionId(), actual.getGlobalTransactionId())
                && Arrays.equals(expected.getBranchQualifier(), actual.getBranchQualifier());
    }

    private static final class TestXid implements Xid {
        private final int formatId;
        private final byte[] globalTransactionId;
        private final byte[] branchQualifier;

        private TestXid(int formatId, byte[] globalTransactionId, byte[] branchQualifier) {
            this.formatId = formatId;
            this.globalTransactionId = globalTransactionId.clone();
            this.branchQualifier = branchQualifier.clone();
        }

        @Override
        public int getFormatId() {
            return formatId;
        }

        @Override
        public byte[] getGlobalTransactionId() {
            return globalTransactionId.clone();
        }

        @Override
        public byte[] getBranchQualifier() {
            return branchQualifier.clone();
        }
    }

    @Test
    void rejectsOversizedFrameBeforeNewlineOrEof() throws Exception {
        InputStream endlessUntilLimit = new InputStream() {
            private int remaining = 16 * 1024 * 1024 + 1;

            @Override
            public int read() throws IOException {
                if (remaining == 0) {
                    throw new IOException("reader consumed beyond frame limit");
                }
                remaining--;
                return 'x';
            }

            @Override
            public int read(byte[] target, int offset, int length) throws IOException {
                if (length == 0) {
                    return 0;
                }
                if (remaining == 0) {
                    throw new IOException("reader consumed beyond frame limit");
                }
                int count = Math.min(length, remaining);
                Arrays.fill(target, offset, offset + count, (byte) 'x');
                remaining -= count;
                return count;
            }
        };
        try (AgentServer server = new AgentServer(endlessUntilLimit, new ByteArrayOutputStream())) {
            IOException error = assertThrows(IOException.class, server::run);
            assertEquals("JSON-RPC request exceeds maximum frame size", error.getMessage());
        }
    }

    @Test
    void malformedHandshakeAndJsonDoNotEchoUntrustedPayloads() throws Exception {
        String capabilityMarker = "CAPABILITY-SECRET-MARKER";
        String jsonMarker = "MALFORMED-SECRET-MARKER";
        ExecutorService executor = Executors.newSingleThreadExecutor();
        try (PipedInputStream serverInput = new PipedInputStream();
             PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
             PipedInputStream responseInput = new PipedInputStream();
             PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
             BufferedWriter requests = new BufferedWriter(new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
             BufferedReader responses = new BufferedReader(new InputStreamReader(responseInput, StandardCharsets.UTF_8));
             AgentServer server = new AgentServer(serverInput, serverOutput)) {
            Future<?> serverTask = executor.submit(() -> {
                server.run();
                return null;
            });

            assertEquals("ready", read(responses).path("method").asText());
            ObjectNode handshake = handshakePayload();
            handshake.putArray("capabilities").add(capabilityMarker);
            write(requests, request(1, "handshake", handshake));
            JsonNode capabilityError = read(responses).path("error");
            assertEquals("unsupported JDBC Agent capability", capabilityError.path("message").asText());
            assertTrue(!capabilityError.toString().contains(capabilityMarker));

            requests.write("{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"handshake\",\"params\":{\"token\":\""
                    + jsonMarker + "\", broken}}}");
            requests.newLine();
            requests.flush();
            JsonNode malformedError = read(responses).path("error");
            assertEquals("invalid JSON-RPC request", malformedError.path("message").asText());
            assertTrue(!malformedError.toString().contains(jsonMarker));

            ObjectNode invalidVersion = request(3, "handshake", handshakePayload());
            invalidVersion.put("jsonrpc", capabilityMarker);
            write(requests, invalidVersion);
            JsonNode versionError = read(responses).path("error");
            assertEquals("unsupported JSON-RPC version", versionError.path("message").asText());
            assertTrue(!versionError.toString().contains(capabilityMarker));

            requests.close();
            serverTask.get();
        } finally {
            executor.shutdownNow();
        }
    }

    @Test
    void preservesSQLExceptionHierarchyCauseAndNextException() {
        SQLTransientConnectionException exception =
                new SQLTransientConnectionException("connection timed out", "08006", 77);
        exception.initCause(new SocketTimeoutException("socket timed out"));
        exception.setNextException(new SQLIntegrityConstraintViolationException(
                "duplicate key", "23505", 88));

        AgentError error = AgentError.from(42, "session-1", exception);

        assertEquals("java.sql.SQLTransientConnectionException", error.exceptionClass());
        assertTrue(error.fatal());
        assertTrue(error.transientError());
        assertTrue(error.assignableTypes().contains("java.sql.SQLException"));
        assertEquals("java.net.SocketTimeoutException", error.causes().get(0));
        assertEquals(1, error.nextExceptions().size());
        assertEquals("23505", error.nextExceptions().get(0).sqlState());
    }

    @Test
    void executesJsonRpcLifecycleAcrossTwoSessions() throws Exception {
        ExecutorService executor = Executors.newSingleThreadExecutor();
        try (PipedInputStream serverInput = new PipedInputStream();
             PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
             PipedInputStream responseInput = new PipedInputStream();
             PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
             BufferedWriter requests = new BufferedWriter(new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
             BufferedReader responses = new BufferedReader(new InputStreamReader(responseInput, StandardCharsets.UTF_8));
             AgentServer server = new AgentServer(serverInput, serverOutput)) {
            Future<?> serverTask = executor.submit(() -> {
                server.run();
                return null;
            });

            JsonNode ready = read(responses);
            assertEquals("ready", ready.path("method").asText());
            assertEquals(1, ready.at("/params/protocolVersion").asInt());

            write(requests, request(1, "handshake", handshakePayload()));
            JsonNode handshake = read(responses).path("result");
            assertEquals(1, handshake.path("protocolVersion").asInt());
            assertEquals("test-h2", handshake.path("driverArtifactVersion").asText());
            assertEquals(true, contains(handshake.path("capabilities"), "native-prepared-batch"));
            assertEquals(true, contains(handshake.path("capabilities"), "array-values"));
            assertEquals(true, contains(handshake.path("capabilities"), "array-result-set"));

            write(requests, request(2, "open_session", connectPayload()));
            String firstSession = read(responses).at("/result/sessionId").asText();
            write(requests, request(3, "open_session", connectPayload()));
            String secondSession = read(responses).at("/result/sessionId").asText();
            assertNotEquals(firstSession, secondSession);

            write(requests, request(4, "execute", sessionPayload(
                    firstSession,
                    sqlPayload("CREATE TABLE sample(id BIGINT PRIMARY KEY, name VARCHAR(32))",
                            JsonNodeFactory.instance.arrayNode()))));
            assertEquals(0, read(responses).at("/result/rowsAffected").asLong());

            ObjectNode prepare = sessionOnly(firstSession);
            prepare.put("sql", "INSERT INTO sample(id, name) VALUES (?, ?)");
            prepare.set("generatedKeys", generatedKeys());
            write(requests, request(5, "prepare", prepare));
            String statementId = read(responses).at("/result/statementId").asText();
            ObjectNode executePrepared = sessionOnly(firstSession);
            executePrepared.put("statementId", statementId);
            executePrepared.put("mode", "update");
            executePrepared.set("params", parameters());
            write(requests, request(6, "execute_prepared", executePrepared));
            assertEquals(1, read(responses).at("/result/rowsAffected").asLong());
            ObjectNode closeStatement = sessionOnly(firstSession);
            closeStatement.put("statementId", statementId);
            write(requests, request(7, "close_statement", closeStatement));
            read(responses);

            write(requests, request(8, "execute", sessionPayload(
                    firstSession,
                    sqlPayload(
                            "INSERT INTO sample "
                                    + "SELECT X, 'name-' || X FROM SYSTEM_RANGE(1, 1200) WHERE X <> 7",
                            JsonNodeFactory.instance.arrayNode()))));
            assertEquals(1199, read(responses).at("/result/rowsAffected").asLong());

            ObjectNode queryPayload = sessionPayload(
                    secondSession,
                    sqlPayload("SELECT id, name FROM sample ORDER BY id",
                            JsonNodeFactory.instance.arrayNode()));
            queryPayload.put("pageSize", 500);
            write(requests, request(9, "execute_query", queryPayload));
            JsonNode query = read(responses).path("result");
            assertEquals(500, query.path("rows").size());
            assertEquals(true, query.path("hasMore").asBoolean());
            String cursorId = query.path("cursorId").asText();

            ObjectNode fetchPage = sessionOnly(secondSession);
            fetchPage.put("cursorId", cursorId);
            fetchPage.put("pageSize", 500);
            write(requests, request(10, "fetch_page", fetchPage));
            JsonNode secondPage = read(responses).path("result");
            assertEquals(500, secondPage.path("rows").size());
            assertEquals(true, secondPage.path("hasMore").asBoolean());

            write(requests, request(11, "fetch_page", fetchPage));
            JsonNode finalPage = read(responses).path("result");
            assertEquals(200, finalPage.path("rows").size());
            assertEquals(false, finalPage.path("hasMore").asBoolean());

            ObjectNode setAutoCommit = sessionOnly(firstSession);
            setAutoCommit.put("value", false);
            write(requests, request(12, "set_auto_commit", setAutoCommit));
            read(responses);
            write(requests, request(13, "get_auto_commit", sessionOnly(firstSession)));
            assertEquals(false, read(responses).at("/result/value").asBoolean());
            setAutoCommit.put("value", true);
            write(requests, request(14, "set_auto_commit", setAutoCommit));
            read(responses);

            write(requests, request(15, "database_metadata", sessionOnly(secondSession)));
            assertEquals("H2", read(responses).at("/result/databaseProductName").asText());

            write(requests, request(16, "execute", sessionPayload(
                    firstSession,
                    sqlPayload(
                            "CREATE ALIAS IF NOT EXISTS SLEEP FOR 'java.lang.Thread.sleep(long)'",
                            JsonNodeFactory.instance.arrayNode()))));
            JsonNode aliasResponse = read(responses);
            assertEquals(16, aliasResponse.path("id").asLong());
            assertTrue(aliasResponse.path("result").isObject());
            write(requests, request(17, "execute_query", sessionPayload(
                    firstSession,
                    sqlPayload("CALL SLEEP(400)", JsonNodeFactory.instance.arrayNode()))));
            Thread.sleep(100);
            ObjectNode cancel = sessionOnly(firstSession);
            cancel.put("targetRequestId", 17);
            write(requests, request(18, "cancel", cancel));
            JsonNode firstConcurrentResponse = read(responses);
            JsonNode secondConcurrentResponse = read(responses);
            JsonNode cancelResponse = firstConcurrentResponse.path("id").asLong() == 18
                    ? firstConcurrentResponse
                    : secondConcurrentResponse;
            JsonNode queryResponse = firstConcurrentResponse.path("id").asLong() == 17
                    ? firstConcurrentResponse
                    : secondConcurrentResponse;
            assertEquals(17, queryResponse.path("id").asLong());
            assertEquals(18, cancelResponse.path("id").asLong());
            assertTrue(cancelResponse.at("/result/cancelled").isBoolean());
            // 取消和查询完成存在竞态；目标已完成时返回 false 是正确的协议结果。
            assertTrue(cancelResponse.at("/result/cancelled").asBoolean()
                    || queryResponse.path("result").isObject());

            write(requests, request(19, "unknown_method", sessionOnly(firstSession)));
            JsonNode error = read(responses).path("error");
            assertEquals(-32601, error.path("code").asInt());
            assertEquals(firstSession, error.at("/data/sessionId").asText());
            assertEquals(19, error.at("/data/requestId").asLong());

            write(requests, request(20, "close_session", sessionOnly(firstSession)));
            read(responses);
            write(requests, request(21, "close_session", sessionOnly(secondSession)));
            read(responses);
            requests.close();
            serverTask.get();
        } finally {
            executor.shutdownNow();
        }
    }

    @Test
    void h2ArrayProtocolPreservesNullRangeOwnershipAndFree() throws Exception {
        String nullElementsDriverFailure = null;
        try (Connection direct = DriverManager.getConnection("jdbc:h2:mem:agent;DB_CLOSE_DELAY=-1")) {
            try {
                java.sql.Array directArray = direct.createArrayOf("INTEGER", null);
                directArray.free();
            } catch (SQLException | RuntimeException exception) {
                nullElementsDriverFailure = exception.getClass().getName();
            }
        }
        ExecutorService executor = Executors.newSingleThreadExecutor();
        try (PipedInputStream serverInput = new PipedInputStream();
             PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
             PipedInputStream responseInput = new PipedInputStream();
             PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
             BufferedWriter requests = new BufferedWriter(new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
             BufferedReader responses = new BufferedReader(new InputStreamReader(responseInput, StandardCharsets.UTF_8));
             AgentServer server = new AgentServer(serverInput, serverOutput)) {
            Future<?> serverTask = executor.submit(() -> {
                server.run();
                return null;
            });
            read(responses);
            write(requests, request(101, "handshake", handshakePayload()));
            read(responses);
            write(requests, request(102, "open_session", connectPayload()));
            String owner = read(responses).at("/result/sessionId").asText();
            write(requests, request(103, "open_session", connectPayload()));
            String other = read(responses).at("/result/sessionId").asText();

            ObjectNode create = sessionOnly(owner);
            create.put("typeName", "INTEGER");
            ArrayNode elements = create.putArray("elements");
            elements.addObject().put("kind", "int").put("value", 11);
            elements.addObject().put("kind", "null").putNull("value");
            elements.addObject().put("kind", "int").put("value", 33);
            write(requests, request(104, "create_array", create));
            JsonNode created = read(responses);
            assertTrue(created.path("result").isObject(), created.toString());
            String arrayId = created.at("/result/arrayId").asText();

            ObjectNode typedCreate = sessionOnly(owner);
            typedCreate.put("typeName", "INTEGER");
            typedCreate.putArray("elements").addObject().put("kind", "integer").put("value", 42);
            write(requests, request(121, "create_array", typedCreate));
            JsonNode typedCreated = read(responses);
            assertTrue(typedCreated.path("result").isObject(), typedCreated.toString());
            ObjectNode typedArray = sessionOnly(owner);
            typedArray.put("arrayId", typedCreated.at("/result/arrayId").asText());
            write(requests, request(122, "array_metadata", typedArray));
            assertEquals("INTEGER", read(responses).at("/result/baseTypeName").asText());
            write(requests, request(123, "array_values", typedArray));
            JsonNode typedValue = read(responses).at("/result/values/0");
            assertEquals("integer", typedValue.path("kind").asText());
            assertEquals(42, typedValue.path("value").asInt());
            write(requests, request(125, "array_result_set", typedArray));
            JsonNode typedCursorValue = read(responses).at("/result/rows/0/1");
            assertEquals("integer", typedCursorValue.path("kind").asText());
            assertEquals(42, typedCursorValue.path("value").asInt());
            write(requests, request(124, "free_array", typedArray));
            read(responses);

            ObjectNode nullCreate = sessionOnly(owner);
            nullCreate.put("typeName", "INTEGER");
            nullCreate.putNull("elements");
            write(requests, request(119, "create_array", nullCreate));
            JsonNode nullElementsResponse = read(responses);
            if (Objects.isNull(nullElementsDriverFailure)) {
                assertTrue(nullElementsResponse.path("result").isObject(), nullElementsResponse.toString());
                ObjectNode freeNullArray = sessionOnly(owner);
                freeNullArray.put("arrayId", nullElementsResponse.at("/result/arrayId").asText());
                write(requests, request(120, "free_array", freeNullArray));
                read(responses);
            } else {
                assertEquals(nullElementsDriverFailure,
                        nullElementsResponse.at("/error/data/exceptionClass").asText());
            }

            ObjectNode arrayRequest = sessionOnly(owner);
            arrayRequest.put("arrayId", arrayId);
            write(requests, request(105, "array_metadata", arrayRequest));
            JsonNode metadata = read(responses).path("result");
            // H2 从协议的 i64 值推断 BIGINT；返回实际驱动元数据而非请求的 typeName。
            assertEquals("BIGINT", metadata.path("baseTypeName").asText());
            assertEquals(java.sql.Types.BIGINT, metadata.path("baseType").asInt());

            write(requests, request(106, "array_values", arrayRequest));
            JsonNode values = read(responses).at("/result/values");
            assertEquals(3, values.size());
            assertEquals(11, values.get(0).path("value").asInt());
            assertEquals("null", values.get(1).path("kind").asText());
            assertEquals(33, values.get(2).path("value").asInt());

            arrayRequest.put("index", 2L);
            arrayRequest.put("count", 2);
            write(requests, request(107, "array_values", arrayRequest));
            JsonNode range = read(responses).at("/result/values");
            assertEquals(2, range.size());
            assertEquals("null", range.get(0).path("kind").asText());
            assertEquals(33, range.get(1).path("value").asInt());

            arrayRequest.set("typeMap", JsonNodeFactory.instance.objectNode());
            write(requests, request(113, "array_values", arrayRequest));
            JsonNode unsupportedMap = read(responses).path("error");
            assertEquals("0A000", unsupportedMap.at("/data/sqlState").asText());
            arrayRequest.remove("typeMap");

            ObjectNode cursorRequest = sessionOnly(owner);
            cursorRequest.put("arrayId", arrayId);
            cursorRequest.put("pageSize", 2);
            write(requests, request(114, "array_result_set", cursorRequest));
            JsonNode firstArrayPage = read(responses).path("result");
            assertEquals(2, firstArrayPage.path("rows").size());
            assertEquals("BIGINT", firstArrayPage.at("/columns/1/jdbcTypeName").asText());
            assertEquals(1, firstArrayPage.at("/rows/0/0/value").asInt());
            assertEquals("null", firstArrayPage.at("/rows/1/1/kind").asText());
            String arrayCursorId = firstArrayPage.path("cursorId").asText();
            ObjectNode fetchArrayPage = sessionOnly(owner);
            fetchArrayPage.put("cursorId", arrayCursorId);
            write(requests, request(115, "fetch_page", fetchArrayPage));
            JsonNode finalArrayPage = read(responses).path("result");
            assertEquals(1, finalArrayPage.path("rows").size());
            assertEquals(3, finalArrayPage.at("/rows/0/0/value").asInt());

            cursorRequest.put("index", 2L);
            cursorRequest.put("count", 2);
            write(requests, request(118, "array_result_set", cursorRequest));
            JsonNode rangedCursor = read(responses).path("result");
            assertEquals(2, rangedCursor.path("rows").size());
            assertEquals(2, rangedCursor.at("/rows/0/0/value").asInt());
            assertEquals(3, rangedCursor.at("/rows/1/0/value").asInt());
            cursorRequest.remove("index");
            cursorRequest.remove("count");

            cursorRequest.put("pageSize", 1);
            write(requests, request(116, "array_result_set", cursorRequest));
            JsonNode ownedCursor = read(responses).path("result");
            String ownedCursorId = ownedCursor.path("cursorId").asText();
            assertTrue(ownedCursor.path("hasMore").asBoolean());

            ObjectNode wrongSession = sessionOnly(other);
            wrongSession.put("arrayId", arrayId);
            write(requests, request(108, "array_values", wrongSession));
            assertTrue(read(responses).path("error").isObject());

            write(requests, request(109, "free_array", arrayRequest));
            assertTrue(read(responses).path("result").isObject());
            fetchArrayPage.put("cursorId", ownedCursorId);
            write(requests, request(117, "fetch_page", fetchArrayPage));
            assertTrue(read(responses).path("error").isObject());
            write(requests, request(110, "array_values", arrayRequest));
            assertTrue(read(responses).path("error").isObject());
            write(requests, request(111, "close_session", sessionOnly(owner)));
            read(responses);
            write(requests, request(112, "close_session", sessionOnly(other)));
            read(responses);
            requests.close();
            serverTask.get();
        } finally {
            executor.shutdownNow();
        }
    }

    @Test
    void h2CallableProtocolPreservesOutValueNullAndSessionOwnership() throws Exception {
        // 先证明测试 SQL 在真实驱动上可执行，避免协议测试被方言差异误导。
        String sql = "{? = call ABS(?)}";
        try (Connection direct = DriverManager.getConnection("jdbc:h2:mem:callable-direct");
             java.sql.CallableStatement statement = direct.prepareCall(sql)) {
            statement.registerOutParameter(1, java.sql.Types.INTEGER);
            statement.setInt(2, -7);
            statement.execute();
            assertEquals(7, statement.getInt(1));
        }

        ExecutorService executor = Executors.newSingleThreadExecutor();
        try (PipedInputStream serverInput = new PipedInputStream();
             PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
             PipedInputStream responseInput = new PipedInputStream();
             PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
             BufferedWriter requests = new BufferedWriter(new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
             BufferedReader responses = new BufferedReader(new InputStreamReader(responseInput, StandardCharsets.UTF_8));
             AgentServer server = new AgentServer(serverInput, serverOutput)) {
            Future<?> serverTask = executor.submit(() -> {
                server.run();
                return null;
            });
            assertEquals("ready", read(responses).path("method").asText());
            assertTrue(call(requests, responses, 2201, "handshake", handshakePayload()).path("result").isObject());
            String owner = call(requests, responses, 2202, "open_session", connectPayload())
                    .at("/result/sessionId").asText();
            String other = call(requests, responses, 2203, "open_session", connectPayload())
                    .at("/result/sessionId").asText();
            ObjectNode prepare = sessionOnly(owner);
            prepare.put("sql", sql);
            JsonNode prepared = call(requests, responses, 2204, "prepare_call", prepare);
            assertTrue(prepared.path("result").isObject(), prepared.toString());
            String statementId = prepared.at("/result/statementId").asText();

            ObjectNode execute = sessionOnly(owner);
            execute.put("statementId", statementId);
            ArrayNode input = execute.putArray("inParameters");
            ObjectNode bound = input.addObject();
            bound.put("index", 2);
            bound.set("value", objectMapper.valueToTree(new AgentValue(
                    "int", JsonNodeFactory.instance.numberNode(-7))));
            ObjectNode out = execute.putArray("outParameters").addObject();
            out.put("index", 1);
            out.put("sqlType", java.sql.Types.INTEGER);
            JsonNode executed = call(requests, responses, 2205, "execute_callable", execute);
            assertEquals(7, executed.at("/result/outValues/0/value/value").asInt(), executed.toString());
            assertEquals(false, executed.at("/result/outValues/0/wasNull").asBoolean());

            ObjectNode nullExecute = execute.deepCopy();
            ((ObjectNode) nullExecute.at("/inParameters/0"))
                    .set("value", objectMapper.valueToTree(new AgentValue("null", null)));
            JsonNode nullResult = call(requests, responses, 2211, "execute_callable", nullExecute);
            assertEquals("null", nullResult.at("/result/outValues/0/value/kind").asText(), nullResult.toString());
            assertEquals(true, nullResult.at("/result/outValues/0/wasNull").asBoolean());

            ObjectNode invalid = execute.deepCopy();
            ((ObjectNode) invalid.at("/outParameters/0")).put("index", 0);
            assertTrue(call(requests, responses, 2212, "execute_callable", invalid).path("error").isObject());

            ObjectNode foreign = execute.deepCopy();
            foreign.put("sessionId", other);
            assertTrue(call(requests, responses, 2206, "execute_callable", foreign).path("error").isObject());
            ObjectNode close = sessionOnly(owner);
            close.put("statementId", statementId);
            assertTrue(call(requests, responses, 2207, "close_statement", close).path("result").isObject());
            assertTrue(call(requests, responses, 2208, "execute_callable", execute).path("error").isObject());
            assertTrue(call(requests, responses, 2209, "close_session", sessionOnly(owner)).path("result").isObject());
            assertTrue(call(requests, responses, 2210, "close_session", sessionOnly(other)).path("result").isObject());
            requests.close();
            serverTask.get();
        } finally {
            executor.shutdownNow();
        }
    }

    @Test
    void hsqldbCallableProtocolPreservesProcedureOutParameter() throws Exception {
        String procedure = "CREATE PROCEDURE absolute_value(IN p_input INT, OUT p_output INT) "
                + "READS SQL DATA BEGIN ATOMIC SET p_output = ABS(p_input); END";
        try (Connection connection = DriverManager.getConnection(
                "jdbc:hsqldb:mem:callable-direct", "SA", "");
             Statement setup = connection.createStatement()) {
            setup.execute(procedure);
            try (java.sql.CallableStatement statement = connection.prepareCall(
                    "CALL absolute_value(?, ?)")) {
                statement.setInt(1, -9);
                statement.registerOutParameter(2, java.sql.Types.INTEGER);
                statement.execute();
                assertEquals(9, statement.getInt(2));
            }
        }

        ExecutorService executor = Executors.newSingleThreadExecutor();
        try (PipedInputStream serverInput = new PipedInputStream();
             PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
             PipedInputStream responseInput = new PipedInputStream();
             PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
             BufferedWriter requests = new BufferedWriter(new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
             BufferedReader responses = new BufferedReader(new InputStreamReader(responseInput, StandardCharsets.UTF_8));
             AgentServer server = new AgentServer(serverInput, serverOutput)) {
            Future<?> serverTask = executor.submit(() -> {
                server.run();
                return null;
            });
            assertEquals("ready", read(responses).path("method").asText());
            call(requests, responses, 2301, "handshake", handshakePayload());
            ObjectNode connect = connectPayload();
            connect.put("url", "jdbc:hsqldb:mem:callable-protocol");
            connect.put("validationQuery", "VALUES (1)");
            connect.with("properties").put("user", "SA").put("password", "");
            String sessionId = call(requests, responses, 2302, "open_session", connect)
                    .at("/result/sessionId").asText();
            JsonNode created = call(requests, responses, 2303, "execute", sessionPayload(
                    sessionId, sqlPayload(procedure, JsonNodeFactory.instance.arrayNode())));
            assertTrue(created.path("result").isObject(), created.toString());
            ObjectNode prepare = sessionOnly(sessionId);
            prepare.put("sql", "CALL absolute_value(?, ?)");
            String statementId = call(requests, responses, 2304, "prepare_call", prepare)
                    .at("/result/statementId").asText();
            ObjectNode execute = sessionOnly(sessionId);
            execute.put("statementId", statementId);
            ObjectNode input = execute.putArray("inParameters").addObject();
            input.put("index", 1);
            input.set("value", objectMapper.valueToTree(new AgentValue(
                    "int", JsonNodeFactory.instance.numberNode(-11))));
            ObjectNode output = execute.putArray("outParameters").addObject();
            output.put("index", 2);
            output.put("sqlType", java.sql.Types.INTEGER);
            JsonNode result = call(requests, responses, 2305, "execute_callable", execute);
            assertEquals(11, result.at("/result/outValues/0/value/value").asInt(), result.toString());
            ObjectNode close = sessionOnly(sessionId);
            close.put("statementId", statementId);
            assertTrue(call(requests, responses, 2306, "close_statement", close).path("result").isObject());
            assertTrue(call(requests, responses, 2307, "close_session", sessionOnly(sessionId))
                    .path("result").isObject());
            requests.close();
            serverTask.get();
        } finally {
            executor.shutdownNow();
        }
    }

    @Test
    void h2BlobProtocolPreservesBytesPositionsDriverErrorsAndFree() throws Exception {
        SQLException expectedPositionError;
        try (Connection direct = DriverManager.getConnection("jdbc:h2:mem:agent-blob-direct")) {
            java.sql.Blob blob = direct.createBlob();
            expectedPositionError = assertThrows(SQLException.class, () -> blob.getBytes(1, -1));
            blob.free();
        }
        ExecutorService executor = Executors.newSingleThreadExecutor();
        try (PipedInputStream serverInput = new PipedInputStream();
             PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
             PipedInputStream responseInput = new PipedInputStream();
             PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
             BufferedWriter requests = new BufferedWriter(new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
             BufferedReader responses = new BufferedReader(new InputStreamReader(responseInput, StandardCharsets.UTF_8));
             AgentServer server = new AgentServer(serverInput, serverOutput)) {
            Future<?> serverTask = executor.submit(() -> {
                server.run();
                return null;
            });
            assertEquals("ready", read(responses).path("method").asText());
            assertTrue(call(requests, responses, 201, "handshake", handshakePayload()).path("result").isObject());
            String sessionId = call(requests, responses, 202, "open_session", connectPayload())
                    .at("/result/sessionId").asText();
            String blobId = call(requests, responses, 203, "create_blob", sessionOnly(sessionId))
                    .at("/result/blobId").asText();
            ObjectNode blobRequest = sessionOnly(sessionId);
            blobRequest.put("blobId", blobId);
            assertEquals(0, call(requests, responses, 204, "blob_length", blobRequest)
                    .at("/result/length").asLong());

            ObjectNode write = blobRequest.deepCopy();
            write.put("position", 1L);
            write.put("bytes", Base64.getEncoder().encodeToString(new byte[] {10, 20, 30, 40}));
            assertEquals(4, call(requests, responses, 205, "blob_set_bytes", write)
                    .at("/result/written").asInt());
            ObjectNode read = blobRequest.deepCopy();
            read.put("position", 2L);
            read.put("length", 2);
            byte[] range = Base64.getDecoder().decode(call(requests, responses, 206, "blob_get_bytes", read)
                    .at("/result/bytes").asText());
            assertTrue(Arrays.equals(new byte[] {20, 30}, range));

            ObjectNode rangedWrite = blobRequest.deepCopy();
            rangedWrite.put("position", 3L);
            rangedWrite.put("bytes", Base64.getEncoder().encodeToString(new byte[] {1, 2, 3, 4}));
            rangedWrite.put("offset", 1);
            rangedWrite.put("length", 2);
            JsonNode rangedResponse = call(requests, responses, 207, "blob_set_bytes", rangedWrite);
            assertEquals("HYC00", rangedResponse.at("/error/data/sqlState").asText());
            assertEquals(50100, rangedResponse.at("/error/data/vendorCode").asInt());
            ObjectNode search = blobRequest.deepCopy();
            search.put("pattern", Base64.getEncoder().encodeToString(new byte[] {20, 30}));
            search.put("start", 1L);
            JsonNode searchResponse = call(requests, responses, 208, "blob_position_bytes", search);
            assertEquals("HYC00", searchResponse.at("/error/data/sqlState").asText());
            assertEquals(50100, searchResponse.at("/error/data/vendorCode").asInt());
            ObjectNode truncate = blobRequest.deepCopy();
            truncate.put("length", 3L);
            JsonNode truncateResponse = call(requests, responses, 209, "blob_truncate", truncate);
            assertEquals("HYC00", truncateResponse.at("/error/data/sqlState").asText());
            assertEquals(50100, truncateResponse.at("/error/data/vendorCode").asInt());
            assertEquals(4, call(requests, responses, 210, "blob_length", blobRequest)
                    .at("/result/length").asLong());
            ObjectNode invalid = blobRequest.deepCopy();
            invalid.put("position", 1L);
            invalid.put("length", -1);
            JsonNode error = call(requests, responses, 211, "blob_get_bytes", invalid).path("error");
            assertEquals(expectedPositionError.getSQLState(), error.at("/data/sqlState").asText());
            assertEquals(expectedPositionError.getErrorCode(), error.at("/data/vendorCode").asInt());

            assertTrue(call(requests, responses, 212, "free_blob", blobRequest).path("result").isObject());
            assertTrue(call(requests, responses, 213, "blob_length", blobRequest).path("error").isObject());
            String rangedBlobId = call(requests, responses, 215, "create_blob", sessionOnly(sessionId))
                    .at("/result/blobId").asText();
            ObjectNode initialRangedWrite = sessionOnly(sessionId);
            initialRangedWrite.put("blobId", rangedBlobId);
            initialRangedWrite.put("position", 1L);
            initialRangedWrite.put("bytes", Base64.getEncoder().encodeToString(new byte[] {7, 8, 9, 10}));
            initialRangedWrite.put("offset", 1);
            initialRangedWrite.put("length", 2);
            JsonNode initialRangedResponse = call(requests, responses, 216, "blob_set_bytes", initialRangedWrite);
            assertEquals(2, initialRangedResponse.at("/result/written").asInt(), initialRangedResponse.toString());
            ObjectNode rangedRead = sessionOnly(sessionId);
            rangedRead.put("blobId", rangedBlobId);
            rangedRead.put("position", 1L);
            rangedRead.put("length", 2);
            assertTrue(Arrays.equals(new byte[] {8, 9}, Base64.getDecoder().decode(
                    call(requests, responses, 217, "blob_get_bytes", rangedRead)
                            .at("/result/bytes").asText())));
            assertTrue(call(requests, responses, 218, "free_blob", rangedRead).path("result").isObject());
            assertTrue(call(requests, responses, 214, "close_session", sessionOnly(sessionId))
                    .path("result").isObject());
            requests.close();
            serverTask.get();
        } finally {
            executor.shutdownNow();
        }
    }

    @Test
    void h2ClobProtocolPreservesUtf16LengthAndOneBasedRead() throws Exception {
        SQLException expectedReadError;
        SQLException expectedSearchError;
        SQLException expectedTruncateError;
        SQLException expectedAsciiOutputError;
        String driverIsolatedSurrogate;
        try (Connection direct = DriverManager.getConnection("jdbc:h2:mem:agent-clob-direct")) {
            java.sql.Clob driverClob = direct.createClob();
            driverClob.setString(1, "A😀Z");
            expectedReadError = assertThrows(SQLException.class, () -> driverClob.getSubString(1, -1));
            expectedSearchError = assertThrows(SQLException.class, () -> driverClob.position("😀", 1));
            expectedTruncateError = assertThrows(SQLException.class, () -> driverClob.truncate(3));
            expectedAsciiOutputError = assertThrows(SQLException.class,
                    () -> driverClob.setAsciiStream(1));
            driverClob.free();
            java.sql.Clob isolated = direct.createClob();
            isolated.setString(1, new String(new char[] {(char) 0xd800}));
            driverIsolatedSurrogate = isolated.getSubString(1, 1);
            isolated.free();
        }
        ExecutorService executor = Executors.newSingleThreadExecutor();
        try (PipedInputStream serverInput = new PipedInputStream();
             PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
             PipedInputStream responseInput = new PipedInputStream();
             PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
             BufferedWriter requests = new BufferedWriter(new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
             BufferedReader responses = new BufferedReader(new InputStreamReader(responseInput, StandardCharsets.UTF_8));
             AgentServer server = new AgentServer(serverInput, serverOutput)) {
            Future<?> serverTask = executor.submit(() -> {
                server.run();
                return null;
            });
            JsonNode ready = read(responses);
            assertEquals("ready", ready.path("method").asText());
            for (JsonNode capability : ready.at("/params/capabilities")) {
                assertNotEquals("clob-position-clob", capability.asText());
            }
            JsonNode handshake = call(requests, responses, 801, "handshake", handshakePayload());
            assertTrue(handshake.path("result").isObject());
            for (JsonNode capability : handshake.at("/result/capabilities")) {
                assertNotEquals("clob-position-clob", capability.asText());
            }
            String sessionId = call(requests, responses, 802, "open_session", connectPayload())
                    .at("/result/sessionId").asText();
            String clobId = call(requests, responses, 803, "create_clob", sessionOnly(sessionId))
                    .at("/result/clobId").asText();
            ObjectNode clob = sessionOnly(sessionId);
            clob.put("clobId", clobId);
            assertEquals(0, call(requests, responses, 804, "clob_length", clob)
                    .at("/result/length").asLong());

            ObjectNode write = clob.deepCopy();
            write.put("position", 1L);
            write.put("utf16", encodeUtf16("A😀Z"));
            assertEquals(4, call(requests, responses, 805, "clob_set_string", write)
                    .at("/result/written").asInt());
            assertEquals(4, call(requests, responses, 806, "clob_length", clob)
                    .at("/result/length").asLong());
            ObjectNode range = clob.deepCopy();
            range.put("position", 2L);
            range.put("length", 2);
            assertEquals("😀", decodeUtf16(call(requests, responses, 807, "clob_get_sub_string", range)
                    .at("/result/utf16").asText()));
            ObjectNode invalid = range.deepCopy();
            invalid.put("length", -1);
            JsonNode readError = call(requests, responses, 811, "clob_get_sub_string", invalid).path("error");
            assertEquals(expectedReadError.getSQLState(), readError.at("/data/sqlState").asText());
            assertEquals(expectedReadError.getErrorCode(), readError.at("/data/vendorCode").asInt());
            ObjectNode search = clob.deepCopy();
            search.put("utf16", encodeUtf16("😀"));
            search.put("start", 1L);
            JsonNode searchError = call(requests, responses, 812, "clob_position_string", search).path("error");
            assertEquals(expectedSearchError.getSQLState(), searchError.at("/data/sqlState").asText());
            assertEquals(expectedSearchError.getErrorCode(), searchError.at("/data/vendorCode").asInt());
            ObjectNode truncate = clob.deepCopy();
            truncate.put("length", 3L);
            JsonNode truncateError = call(requests, responses, 813, "clob_truncate", truncate).path("error");
            assertEquals(expectedTruncateError.getSQLState(), truncateError.at("/data/sqlState").asText());
            assertEquals(expectedTruncateError.getErrorCode(), truncateError.at("/data/vendorCode").asInt());
            ObjectNode asciiOutput = clob.deepCopy();
            asciiOutput.put("position", 1L);
            JsonNode asciiError = call(requests, responses, 834, "clob_open_ascii_output", asciiOutput)
                    .path("error");
            assertEquals(expectedAsciiOutputError.getSQLState(), asciiError.at("/data/sqlState").asText());
            assertEquals(expectedAsciiOutputError.getErrorCode(), asciiError.at("/data/vendorCode").asInt());
            ObjectNode unsupported = clob.deepCopy();
            unsupported.put("patternClobId", clobId);
            unsupported.put("start", 1L);
            assertEquals("0A000", call(requests, responses, 814, "clob_position_clob", unsupported)
                    .at("/error/data/sqlState").asText());
            assertEquals(4, call(requests, responses, 815, "clob_length", clob)
                    .at("/result/length").asLong());
            assertTrue(call(requests, responses, 808, "free_clob", clob).path("result").isObject());
            assertTrue(call(requests, responses, 809, "clob_length", clob).path("error").isObject());
            String isolatedId = call(requests, responses, 816, "create_clob", sessionOnly(sessionId))
                    .at("/result/clobId").asText();
            ObjectNode isolated = sessionOnly(sessionId);
            isolated.put("clobId", isolatedId);
            ObjectNode isolatedWrite = isolated.deepCopy();
            isolatedWrite.put("position", 1L);
            isolatedWrite.put("utf16", encodeUtf16(new String(new char[] {(char) 0xd800})));
            assertEquals(1, call(requests, responses, 817, "clob_set_string", isolatedWrite)
                    .at("/result/written").asInt());
            ObjectNode isolatedRead = isolated.deepCopy();
            isolatedRead.put("position", 1L);
            isolatedRead.put("length", 1);
            assertEquals(driverIsolatedSurrogate,
                    decodeUtf16(call(requests, responses, 818, "clob_get_sub_string", isolatedRead)
                            .at("/result/utf16").asText()));
            isolatedWrite.put("utf16", Base64.getEncoder().encodeToString(new byte[] {(byte) 0xd8}));
            assertTrue(call(requests, responses, 819, "clob_set_string", isolatedWrite)
                    .path("error").isObject());
            isolatedWrite.putNull("utf16");
            assertTrue(call(requests, responses, 832, "clob_set_string", isolatedWrite)
                    .path("error").isObject());
            assertEquals(1L, call(requests, responses, 833, "clob_length", isolated)
                    .at("/result/length").asLong());
            assertTrue(call(requests, responses, 820, "free_clob", isolated).path("result").isObject());
            assertTrue(call(requests, responses, 810, "close_session", sessionOnly(sessionId))
                    .path("result").isObject());
            requests.close();
            serverTask.get();
        } finally {
            executor.shutdownNow();
        }
    }

    @Test
    void nClobCreationMatchesDirectH2DriverWithoutClobFallback() throws Exception {
        SQLException driverFailure = null;
        try (Connection direct = DriverManager.getConnection("jdbc:h2:mem:agent-nclob-direct")) {
            try {
                java.sql.NClob national = direct.createNClob();
                national.free();
            } catch (SQLException exception) {
                driverFailure = exception;
            }
        }

        ExecutorService executor = Executors.newSingleThreadExecutor();
        try (PipedInputStream serverInput = new PipedInputStream();
             PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
             PipedInputStream responseInput = new PipedInputStream();
             PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
             BufferedWriter requests = new BufferedWriter(new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
             BufferedReader responses = new BufferedReader(new InputStreamReader(responseInput, StandardCharsets.UTF_8));
             AgentServer server = new AgentServer(serverInput, serverOutput)) {
            Future<?> serverTask = executor.submit(() -> {
                server.run();
                return null;
            });
            assertEquals("ready", read(responses).path("method").asText());
            assertTrue(call(requests, responses, 930, "handshake", handshakePayload()).path("result").isObject());
            ObjectNode connect = connectPayload();
            connect.put("url", "jdbc:h2:mem:agent-nclob-protocol");
            String sessionId = call(requests, responses, 931, "open_session", connect)
                    .at("/result/sessionId").asText();
            JsonNode created = call(requests, responses, 932, "create_n_clob", sessionOnly(sessionId));
            if (Objects.nonNull(driverFailure)) {
                assertEquals(driverFailure.getSQLState(), created.at("/error/data/sqlState").asText());
                assertEquals(driverFailure.getErrorCode(), created.at("/error/data/vendorCode").asInt());
            } else {
                assertTrue(created.path("result").isObject());
                String nClobId = created.at("/result/nClobId").asText();
                assertNotEquals("", nClobId);
                ObjectNode national = sessionOnly(sessionId);
                national.put("clobId", nClobId);
                JsonNode length = call(requests, responses, 933, "clob_length", national);
                assertTrue(length.path("result").isObject());
                assertEquals(0L, length.at("/result/length").asLong());
                national.put("nClobId", nClobId);
                assertTrue(call(requests, responses, 934, "free_n_clob", national)
                        .path("result").isObject());
                assertTrue(call(requests, responses, 935, "clob_length", national)
                        .path("error").isObject());
            }
            assertTrue(call(requests, responses, 936, "close_session", sessionOnly(sessionId))
                    .path("result").isObject());
            requests.close();
            serverTask.get();
        } finally {
            executor.shutdownNow();
        }
    }

    private String encodeUtf16(String value) {
        byte[] bytes = new byte[value.length() * 2];
        for (int index = 0; index < value.length(); index++) {
            char unit = value.charAt(index);
            bytes[index * 2] = (byte) (unit >>> 8);
            bytes[index * 2 + 1] = (byte) unit;
        }
        return Base64.getEncoder().encodeToString(bytes);
    }

    private String decodeUtf16(String encoded) {
        byte[] bytes = Base64.getDecoder().decode(encoded);
        char[] units = new char[bytes.length / 2];
        for (int index = 0; index < units.length; index++) {
            units[index] = (char) (((bytes[index * 2] & 0xff) << 8) | (bytes[index * 2 + 1] & 0xff));
        }
        return new String(units);
    }

    @Test
    void hsqldbClobProtocolSearchesAndTruncatesWithUtf16Positions() throws Exception {
        ExecutorService executor = Executors.newSingleThreadExecutor();
        try (PipedInputStream serverInput = new PipedInputStream();
             PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
             PipedInputStream responseInput = new PipedInputStream();
             PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
             BufferedWriter requests = new BufferedWriter(new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
             BufferedReader responses = new BufferedReader(new InputStreamReader(responseInput, StandardCharsets.UTF_8));
             AgentServer server = new AgentServer(serverInput, serverOutput)) {
            Future<?> serverTask = executor.submit(() -> {
                server.run();
                return null;
            });
            assertEquals("ready", read(responses).path("method").asText());
            assertTrue(call(requests, responses, 821, "handshake", handshakePayload()).path("result").isObject());
            ObjectNode connect = connectPayload();
            connect.put("url", "jdbc:hsqldb:mem:agent_clob_search");
            connect.put("validationQuery", "VALUES (1)");
            connect.with("properties").put("user", "SA").put("password", "");
            String sessionId = call(requests, responses, 822, "open_session", connect)
                    .at("/result/sessionId").asText();
            String clobId = call(requests, responses, 823, "create_clob", sessionOnly(sessionId))
                    .at("/result/clobId").asText();
            ObjectNode clob = sessionOnly(sessionId);
            clob.put("clobId", clobId);
            ObjectNode write = clob.deepCopy();
            write.put("position", 1L);
            write.put("utf16", encodeUtf16("A😀BCD"));
            assertEquals(6, call(requests, responses, 824, "clob_set_string", write)
                    .at("/result/written").asInt());
            ObjectNode rangedWrite = clob.deepCopy();
            rangedWrite.put("position", 5L);
            rangedWrite.put("utf16", encodeUtf16("xYz"));
            rangedWrite.put("offset", 1);
            rangedWrite.put("length", 1);
            assertEquals(1, call(requests, responses, 825, "clob_set_string", rangedWrite)
                    .at("/result/written").asInt());
            ObjectNode search = clob.deepCopy();
            search.put("utf16", encodeUtf16("BY"));
            search.put("start", 1L);
            assertEquals(4L, call(requests, responses, 826, "clob_position_string", search)
                    .at("/result/position").asLong());
            ObjectNode truncate = clob.deepCopy();
            truncate.put("length", 5L);
            assertTrue(call(requests, responses, 827, "clob_truncate", truncate).path("result").isObject());
            assertEquals(5L, call(requests, responses, 828, "clob_length", clob)
                    .at("/result/length").asLong());
            ObjectNode read = clob.deepCopy();
            read.put("position", 1L);
            read.put("length", 5);
            assertEquals("A😀BY", decodeUtf16(call(requests, responses, 829, "clob_get_sub_string", read)
                    .at("/result/utf16").asText()));
            assertTrue(call(requests, responses, 830, "free_clob", clob).path("result").isObject());
            assertTrue(call(requests, responses, 831, "close_session", sessionOnly(sessionId))
                    .path("result").isObject());
            requests.close();
            serverTask.get();
        } finally {
            executor.shutdownNow();
        }
    }

    @Test
    void h2ClobReaderTransfersBoundedUtf16PagesAndClosesWithParent() throws Exception {
        String expected = "A😀".repeat(35_000);
        ExecutorService executor = Executors.newSingleThreadExecutor();
        try (PipedInputStream serverInput = new PipedInputStream();
             PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
             PipedInputStream responseInput = new PipedInputStream();
             PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
             BufferedWriter requests = new BufferedWriter(new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
             BufferedReader responses = new BufferedReader(new InputStreamReader(responseInput, StandardCharsets.UTF_8));
             AgentServer server = new AgentServer(serverInput, serverOutput)) {
            Future<?> serverTask = executor.submit(() -> {
                server.run();
                return null;
            });
            read(responses);
            call(requests, responses, 841, "handshake", handshakePayload());
            String sessionId = call(requests, responses, 842, "open_session", connectPayload())
                    .at("/result/sessionId").asText();
            String clobId = call(requests, responses, 843, "create_clob", sessionOnly(sessionId))
                    .at("/result/clobId").asText();
            ObjectNode clob = sessionOnly(sessionId);
            clob.put("clobId", clobId);
            ObjectNode write = clob.deepCopy();
            write.put("position", 1L);
            write.put("utf16", encodeUtf16(expected));
            assertEquals(expected.length(), call(requests, responses, 844, "clob_set_string", write)
                    .at("/result/written").asInt());
            String streamId = call(requests, responses, 845, "clob_open_reader", clob)
                    .at("/result/streamId").asText();
            ObjectNode stream = sessionOnly(sessionId);
            stream.put("streamId", streamId);
            stream.put("maxUnits", 32_768);
            StringBuilder actual = new StringBuilder();
            for (int index = 0; index < 5; index++) {
                JsonNode page = call(requests, responses, 846 + index, "clob_read_reader", stream).path("result");
                assertTrue(page.isObject(), page.toString());
                assertTrue(decodeUtf16(page.path("utf16").asText()).length() <= 32_768);
                actual.append(decodeUtf16(page.path("utf16").asText()));
                if (page.path("eof").asBoolean()) {
                    break;
                }
            }
            assertEquals(expected, actual.toString());
            ObjectNode oversized = stream.deepCopy();
            oversized.put("maxUnits", 32_769);
            assertTrue(call(requests, responses, 852, "clob_read_reader", oversized).path("error").isObject());
            ObjectNode range = clob.deepCopy();
            range.put("position", 2L);
            range.put("length", 2L);
            String rangeId = call(requests, responses, 853, "clob_open_reader", range)
                    .at("/result/streamId").asText();
            ObjectNode rangedStream = sessionOnly(sessionId);
            rangedStream.put("streamId", rangeId);
            rangedStream.put("maxUnits", 2);
            assertEquals("😀", decodeUtf16(call(requests, responses, 854, "clob_read_reader", rangedStream)
                    .at("/result/utf16").asText()));
            assertTrue(call(requests, responses, 855, "free_clob", clob).path("result").isObject());
            assertTrue(call(requests, responses, 856, "clob_read_reader", rangedStream).path("error").isObject());
            assertTrue(call(requests, responses, 857, "clob_close_stream", rangedStream).path("result").isObject());
            assertTrue(call(requests, responses, 858, "close_session", sessionOnly(sessionId))
                    .path("result").isObject());
            requests.close();
            serverTask.get();
        } finally {
            executor.shutdownNow();
        }
    }

    @Test
    void h2ClobWriterTransfersBoundedUtf16PagesAndClosesWithParent() throws Exception {
        String expected = "A😀".repeat(35_000);
        ExecutorService executor = Executors.newSingleThreadExecutor();
        try (PipedInputStream serverInput = new PipedInputStream();
             PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
             PipedInputStream responseInput = new PipedInputStream();
             PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
             BufferedWriter requests = new BufferedWriter(new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
             BufferedReader responses = new BufferedReader(new InputStreamReader(responseInput, StandardCharsets.UTF_8));
             AgentServer server = new AgentServer(serverInput, serverOutput)) {
            Future<?> serverTask = executor.submit(() -> {
                server.run();
                return null;
            });
            read(responses);
            call(requests, responses, 871, "handshake", handshakePayload());
            String sessionId = call(requests, responses, 872, "open_session", connectPayload())
                    .at("/result/sessionId").asText();
            String clobId = call(requests, responses, 873, "create_clob", sessionOnly(sessionId))
                    .at("/result/clobId").asText();
            ObjectNode clob = sessionOnly(sessionId);
            clob.put("clobId", clobId);
            clob.put("position", 1L);
            String streamId = call(requests, responses, 874, "clob_open_writer", clob)
                    .at("/result/streamId").asText();
            ObjectNode stream = sessionOnly(sessionId);
            stream.put("streamId", streamId);
            for (int index = 0; index < expected.length(); index += 30_000) {
                String page = expected.substring(index, Math.min(index + 30_000, expected.length()));
                ObjectNode write = stream.deepCopy();
                write.put("utf16", encodeUtf16(page));
                assertEquals(page.length(), call(requests, responses, 875 + index / 30_000,
                        "clob_write_writer", write).at("/result/written").asInt());
            }
            ObjectNode oversized = stream.deepCopy();
            oversized.put("utf16", encodeUtf16("x".repeat(32_769)));
            assertTrue(call(requests, responses, 879, "clob_write_writer", oversized).path("error").isObject());
            assertTrue(call(requests, responses, 880, "clob_flush_writer", stream).path("result").isObject());
            assertTrue(call(requests, responses, 881, "clob_close_stream", stream).path("result").isObject());
            assertEquals(expected.length(), call(requests, responses, 882, "clob_length", clob)
                    .at("/result/length").asLong());
            ObjectNode read = clob.deepCopy();
            read.put("length", expected.length());
            assertEquals(expected, decodeUtf16(call(requests, responses, 883, "clob_get_sub_string", read)
                    .at("/result/utf16").asText()));

            String secondId = call(requests, responses, 884, "create_clob", sessionOnly(sessionId))
                    .at("/result/clobId").asText();
            ObjectNode second = sessionOnly(sessionId);
            second.put("clobId", secondId);
            second.put("position", 1L);
            String activeId = call(requests, responses, 885, "clob_open_writer", second)
                    .at("/result/streamId").asText();
            ObjectNode active = sessionOnly(sessionId);
            active.put("streamId", activeId);
            assertTrue(call(requests, responses, 886, "free_clob", second).path("result").isObject());
            active.put("utf16", encodeUtf16("X"));
            assertTrue(call(requests, responses, 887, "clob_write_writer", active).path("error").isObject());
            assertTrue(call(requests, responses, 888, "free_clob", clob).path("result").isObject());
            assertTrue(call(requests, responses, 889, "close_session", sessionOnly(sessionId))
                    .path("result").isObject());
            requests.close();
            serverTask.get();
        } finally {
            executor.shutdownNow();
        }
    }

    @Test
    void hsqldbClobAsciiStreamsTransferBoundedPagesAndCloseWithParent() throws Exception {
        byte[] expected = new byte[160_000];
        for (int index = 0; index < expected.length; index++) {
            expected[index] = (byte) ('A' + index % 26);
        }
        ExecutorService executor = Executors.newSingleThreadExecutor();
        try (PipedInputStream serverInput = new PipedInputStream();
             PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
             PipedInputStream responseInput = new PipedInputStream();
             PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
             BufferedWriter requests = new BufferedWriter(new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
             BufferedReader responses = new BufferedReader(new InputStreamReader(responseInput, StandardCharsets.UTF_8));
             AgentServer server = new AgentServer(serverInput, serverOutput)) {
            Future<?> serverTask = executor.submit(() -> {
                server.run();
                return null;
            });
            read(responses);
            call(requests, responses, 901, "handshake", handshakePayload());
            ObjectNode connect = connectPayload();
            connect.put("url", "jdbc:hsqldb:mem:agent_clob_ascii_stream");
            connect.put("validationQuery", "VALUES (1)");
            connect.with("properties").put("user", "SA").put("password", "");
            String sessionId = call(requests, responses, 902, "open_session", connect)
                    .at("/result/sessionId").asText();
            String clobId = call(requests, responses, 903, "create_clob", sessionOnly(sessionId))
                    .at("/result/clobId").asText();
            ObjectNode clob = sessionOnly(sessionId);
            clob.put("clobId", clobId);
            clob.put("position", 1L);
            JsonNode openedOutput = call(requests, responses, 904, "clob_open_ascii_output", clob);
            assertTrue(openedOutput.path("result").isObject(), openedOutput.toString());
            String outputId = openedOutput.at("/result/streamId").asText();
            ObjectNode output = sessionOnly(sessionId);
            output.put("streamId", outputId);
            for (int index = 0; index < expected.length; index += 32_000) {
                byte[] page = Arrays.copyOfRange(expected, index, Math.min(index + 32_000, expected.length));
                ObjectNode write = output.deepCopy();
                write.put("bytes", Base64.getEncoder().encodeToString(page));
                assertEquals(page.length, call(requests, responses, 905 + index / 32_000,
                        "clob_write_ascii_output", write).at("/result/written").asInt());
            }
            ObjectNode oversized = output.deepCopy();
            oversized.put("bytes", Base64.getEncoder().encodeToString(new byte[65_537]));
            assertTrue(call(requests, responses, 910, "clob_write_ascii_output", oversized)
                    .path("error").isObject());
            assertTrue(call(requests, responses, 911, "clob_flush_ascii_output", output)
                    .path("result").isObject());
            assertTrue(call(requests, responses, 912, "clob_close_stream", output).path("result").isObject());
            assertEquals(expected.length, call(requests, responses, 913, "clob_length", clob)
                    .at("/result/length").asLong());
            String inputId = call(requests, responses, 914, "clob_open_ascii_input", clob)
                    .at("/result/streamId").asText();
            ObjectNode input = sessionOnly(sessionId);
            input.put("streamId", inputId);
            input.put("maxBytes", 65_536);
            ByteArrayOutputStream actual = new ByteArrayOutputStream();
            for (int index = 0; index < 5; index++) {
                JsonNode page = call(requests, responses, 915 + index, "clob_read_ascii_input", input)
                        .path("result");
                assertTrue(page.isObject(), page.toString());
                byte[] bytes = Base64.getDecoder().decode(page.path("bytes").asText());
                assertTrue(bytes.length <= 65_536);
                actual.write(bytes);
                if (page.path("eof").asBoolean()) {
                    break;
                }
            }
            assertTrue(Arrays.equals(expected, actual.toByteArray()));
            ObjectNode oversizedRead = input.deepCopy();
            oversizedRead.put("maxBytes", 65_537);
            assertTrue(call(requests, responses, 920, "clob_read_ascii_input", oversizedRead)
                    .path("error").isObject());
            assertTrue(call(requests, responses, 921, "free_clob", clob).path("result").isObject());
            assertTrue(call(requests, responses, 922, "clob_read_ascii_input", input).path("error").isObject());
            assertTrue(call(requests, responses, 923, "close_session", sessionOnly(sessionId))
                    .path("result").isObject());
            requests.close();
            serverTask.get();
        } finally {
            executor.shutdownNow();
        }
    }

    @Test
    void hsqldbBlobProtocolSearchesAndTruncatesSuccessfully() throws Exception {
        ExecutorService executor = Executors.newSingleThreadExecutor();
        try (PipedInputStream serverInput = new PipedInputStream();
             PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
             PipedInputStream responseInput = new PipedInputStream();
             PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
             BufferedWriter requests = new BufferedWriter(new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
             BufferedReader responses = new BufferedReader(new InputStreamReader(responseInput, StandardCharsets.UTF_8));
             AgentServer server = new AgentServer(serverInput, serverOutput)) {
            Future<?> serverTask = executor.submit(() -> {
                server.run();
                return null;
            });
            assertEquals("ready", read(responses).path("method").asText());
            assertTrue(call(requests, responses, 701, "handshake", handshakePayload()).path("result").isObject());
            ObjectNode connect = connectPayload();
            connect.put("url", "jdbc:hsqldb:mem:agent_blob_search");
            connect.put("validationQuery", "VALUES (1)");
            connect.with("properties").put("user", "SA").put("password", "");
            String sessionId = call(requests, responses, 702, "open_session", connect)
                    .at("/result/sessionId").asText();
            String blobId = call(requests, responses, 703, "create_blob", sessionOnly(sessionId))
                    .at("/result/blobId").asText();
            ObjectNode blob = sessionOnly(sessionId);
            blob.put("blobId", blobId);
            ObjectNode write = blob.deepCopy();
            write.put("position", 1L);
            write.put("bytes", Base64.getEncoder().encodeToString(new byte[] {10, 20, 30, 40}));
            assertEquals(4, call(requests, responses, 704, "blob_set_bytes", write)
                    .at("/result/written").asInt());
            ObjectNode search = blob.deepCopy();
            search.put("pattern", Base64.getEncoder().encodeToString(new byte[] {20, 30}));
            search.put("start", 1L);
            assertEquals(2L, call(requests, responses, 705, "blob_position_bytes", search)
                    .at("/result/position").asLong());
            search.put("pattern", Base64.getEncoder().encodeToString(new byte[] {99, 98}));
            assertEquals(-1L, call(requests, responses, 706, "blob_position_bytes", search)
                    .at("/result/position").asLong());
            ObjectNode truncate = blob.deepCopy();
            truncate.put("length", 3L);
            assertTrue(call(requests, responses, 707, "blob_truncate", truncate).path("result").isObject());
            assertEquals(3L, call(requests, responses, 708, "blob_length", blob)
                    .at("/result/length").asLong());
            ObjectNode read = blob.deepCopy();
            read.put("position", 1L);
            read.put("length", 3);
            assertTrue(Arrays.equals(new byte[] {10, 20, 30}, Base64.getDecoder().decode(
                    call(requests, responses, 709, "blob_get_bytes", read).at("/result/bytes").asText())));
            assertTrue(call(requests, responses, 710, "free_blob", blob).path("result").isObject());
            assertTrue(call(requests, responses, 711, "close_session", sessionOnly(sessionId))
                    .path("result").isObject());
            requests.close();
            serverTask.get();
        } finally {
            executor.shutdownNow();
        }
    }

    @Test
    void unsupportedBlobSearchIsNotAdvertisedAndReturnsStandardSqlState() throws Exception {
        ExecutorService executor = Executors.newSingleThreadExecutor();
        try (PipedInputStream serverInput = new PipedInputStream();
             PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
             PipedInputStream responseInput = new PipedInputStream();
             PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
             BufferedWriter requests = new BufferedWriter(new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
             BufferedReader responses = new BufferedReader(new InputStreamReader(responseInput, StandardCharsets.UTF_8));
             AgentServer server = new AgentServer(serverInput, serverOutput)) {
            Future<?> serverTask = executor.submit(() -> {
                server.run();
                return null;
            });
            JsonNode ready = read(responses);
            assertTrue(ready.at("/params/capabilities").isArray());
            for (JsonNode capability : ready.at("/params/capabilities")) {
                assertNotEquals("blob-position-blob", capability.asText());
            }
            JsonNode handshake = call(requests, responses, 301, "handshake", handshakePayload());
            for (JsonNode capability : handshake.at("/result/capabilities")) {
                assertNotEquals("blob-position-blob", capability.asText());
            }
            String sessionId = call(requests, responses, 302, "open_session", connectPayload())
                    .at("/result/sessionId").asText();
            String blobId = call(requests, responses, 303, "create_blob", sessionOnly(sessionId))
                    .at("/result/blobId").asText();
            ObjectNode search = sessionOnly(sessionId);
            search.put("blobId", blobId);
            search.put("patternBlobId", blobId);
            search.put("start", 1L);
            JsonNode error = call(requests, responses, 304, "blob_position_blob", search).path("error");
            assertEquals("0A000", error.at("/data/sqlState").asText());
            assertEquals("java.sql.SQLFeatureNotSupportedException", error.at("/data/exceptionClass").asText());
            assertTrue(call(requests, responses, 305, "free_blob", search).path("result").isObject());
            assertTrue(call(requests, responses, 306, "close_session", sessionOnly(sessionId))
                    .path("result").isObject());
            requests.close();
            serverTask.get();
        } finally {
            executor.shutdownNow();
        }
    }

    @Test
    void h2BlobInputStreamReadsMultipleBoundedPagesAndClosesWithParent() throws Exception {
        byte[] expected = new byte[160_000];
        for (int index = 0; index < expected.length; index++) {
            expected[index] = (byte) (index * 31);
        }
        ExecutorService executor = Executors.newSingleThreadExecutor();
        try (PipedInputStream serverInput = new PipedInputStream();
             PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
             PipedInputStream responseInput = new PipedInputStream();
             PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
             BufferedWriter requests = new BufferedWriter(new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
             BufferedReader responses = new BufferedReader(new InputStreamReader(responseInput, StandardCharsets.UTF_8));
             AgentServer server = new AgentServer(serverInput, serverOutput)) {
            Future<?> serverTask = executor.submit(() -> {
                server.run();
                return null;
            });
            assertEquals("ready", read(responses).path("method").asText());
            assertTrue(call(requests, responses, 401, "handshake", handshakePayload()).path("result").isObject());
            String sessionId = call(requests, responses, 402, "open_session", connectPayload())
                    .at("/result/sessionId").asText();
            String blobId = call(requests, responses, 403, "create_blob", sessionOnly(sessionId))
                    .at("/result/blobId").asText();
            ObjectNode blob = sessionOnly(sessionId);
            blob.put("blobId", blobId);
            ObjectNode write = blob.deepCopy();
            write.put("position", 1L);
            write.put("bytes", Base64.getEncoder().encodeToString(expected));
            assertEquals(expected.length, call(requests, responses, 404, "blob_set_bytes", write)
                    .at("/result/written").asInt());

            String streamId = call(requests, responses, 405, "blob_open_input_stream", blob)
                    .at("/result/streamId").asText();
            ObjectNode oversizedRead = sessionOnly(sessionId);
            oversizedRead.put("streamId", streamId);
            oversizedRead.put("maxBytes", 65_537);
            assertTrue(call(requests, responses, 419, "blob_read_input_stream", oversizedRead)
                    .path("error").isObject());
            ByteArrayOutputStream actual = new ByteArrayOutputStream();
            for (int page = 0; page < 10; page++) {
                ObjectNode next = sessionOnly(sessionId);
                next.put("streamId", streamId);
                next.put("maxBytes", 32_768);
                JsonNode result = call(requests, responses, 406 + page, "blob_read_input_stream", next)
                        .path("result");
                assertTrue(result.path("bytes").isTextual(), result.toString());
                byte[] chunk = Base64.getDecoder().decode(result.path("bytes").asText());
                assertTrue(chunk.length <= 32_768);
                actual.write(chunk);
                if (result.path("eof").asBoolean()) {
                    break;
                }
            }
            assertTrue(Arrays.equals(expected, actual.toByteArray()));
            ObjectNode stream = sessionOnly(sessionId);
            stream.put("streamId", streamId);
            assertTrue(call(requests, responses, 420, "blob_close_stream", stream).path("result").isObject());
            assertTrue(call(requests, responses, 421, "blob_read_input_stream", stream).path("error").isObject());

            ObjectNode range = blob.deepCopy();
            range.put("position", 100L);
            range.put("length", 100L);
            String rangedId = call(requests, responses, 422, "blob_open_input_stream", range)
                    .at("/result/streamId").asText();
            ObjectNode rangedRead = sessionOnly(sessionId);
            rangedRead.put("streamId", rangedId);
            rangedRead.put("maxBytes", 100);
            byte[] rangedBytes = Base64.getDecoder().decode(call(requests, responses, 423,
                    "blob_read_input_stream", rangedRead).at("/result/bytes").asText());
            assertTrue(Arrays.equals(Arrays.copyOfRange(expected, 99, 199), rangedBytes));
            assertTrue(call(requests, responses, 424, "free_blob", blob).path("result").isObject());
            assertTrue(call(requests, responses, 425, "blob_read_input_stream", rangedRead).path("error").isObject());
            assertTrue(call(requests, responses, 426, "close_session", sessionOnly(sessionId))
                    .path("result").isObject());
            requests.close();
            serverTask.get();
        } finally {
            executor.shutdownNow();
        }
    }

    @Test
    void h2BlobOutputStreamWritesMultipleBoundedPagesAndClosesWithParent() throws Exception {
        byte[] expected = new byte[160_000];
        for (int index = 0; index < expected.length; index++) {
            expected[index] = (byte) (index * 17);
        }
        ExecutorService executor = Executors.newSingleThreadExecutor();
        try (PipedInputStream serverInput = new PipedInputStream();
             PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
             PipedInputStream responseInput = new PipedInputStream();
             PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
             BufferedWriter requests = new BufferedWriter(new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
             BufferedReader responses = new BufferedReader(new InputStreamReader(responseInput, StandardCharsets.UTF_8));
             AgentServer server = new AgentServer(serverInput, serverOutput)) {
            Future<?> serverTask = executor.submit(() -> {
                server.run();
                return null;
            });
            assertEquals("ready", read(responses).path("method").asText());
            assertTrue(call(requests, responses, 501, "handshake", handshakePayload()).path("result").isObject());
            String sessionId = call(requests, responses, 502, "open_session", connectPayload())
                    .at("/result/sessionId").asText();
            String blobId = call(requests, responses, 503, "create_blob", sessionOnly(sessionId))
                    .at("/result/blobId").asText();
            ObjectNode blob = sessionOnly(sessionId);
            blob.put("blobId", blobId);
            blob.put("position", 1L);
            String streamId = call(requests, responses, 504, "blob_open_output_stream", blob)
                    .at("/result/streamId").asText();
            ObjectNode stream = sessionOnly(sessionId);
            stream.put("streamId", streamId);
            ObjectNode oversized = stream.deepCopy();
            oversized.put("bytes", Base64.getEncoder().encodeToString(new byte[65_537]));
            assertTrue(call(requests, responses, 521, "blob_write_output_stream", oversized)
                    .path("error").isObject());
            for (int offset = 0; offset < expected.length; offset += 32_768) {
                byte[] chunk = Arrays.copyOfRange(expected, offset, Math.min(expected.length, offset + 32_768));
                ObjectNode write = stream.deepCopy();
                write.put("bytes", Base64.getEncoder().encodeToString(chunk));
                assertEquals(chunk.length, call(requests, responses, 505 + offset / 32_768,
                        "blob_write_output_stream", write).at("/result/written").asInt());
            }
            assertTrue(call(requests, responses, 511, "blob_flush_stream", stream).path("result").isObject());
            assertTrue(call(requests, responses, 512, "blob_close_stream", stream).path("result").isObject());
            assertTrue(call(requests, responses, 513, "blob_write_output_stream", stream).path("error").isObject());
            ObjectNode read = sessionOnly(sessionId);
            read.put("blobId", blobId);
            read.put("position", 1L);
            read.put("length", expected.length);
            assertTrue(Arrays.equals(expected, Base64.getDecoder().decode(
                    call(requests, responses, 514, "blob_get_bytes", read).at("/result/bytes").asText())));
            assertTrue(call(requests, responses, 515, "free_blob", blob).path("result").isObject());
            String secondBlobId = call(requests, responses, 517, "create_blob", sessionOnly(sessionId))
                    .at("/result/blobId").asText();
            ObjectNode secondBlob = sessionOnly(sessionId);
            secondBlob.put("blobId", secondBlobId);
            secondBlob.put("position", 1L);
            String activeStreamId = call(requests, responses, 518, "blob_open_output_stream", secondBlob)
                    .at("/result/streamId").asText();
            ObjectNode activeStream = sessionOnly(sessionId);
            activeStream.put("streamId", activeStreamId);
            assertTrue(call(requests, responses, 519, "free_blob", secondBlob).path("result").isObject());
            ObjectNode afterFreeWrite = activeStream.deepCopy();
            afterFreeWrite.put("bytes", Base64.getEncoder().encodeToString(new byte[] {1}));
            assertTrue(call(requests, responses, 520, "blob_write_output_stream", afterFreeWrite)
                    .path("error").isObject());
            assertTrue(call(requests, responses, 516, "close_session", sessionOnly(sessionId))
                    .path("result").isObject());
            requests.close();
            serverTask.get();
        } finally {
            executor.shutdownNow();
        }
    }

    @Test
    @SuppressWarnings("unchecked")
    void cancelClosesActiveBlobReadAndReturnsCancellationSqlState() throws Exception {
        CountDownLatch reading = new CountDownLatch(1);
        CountDownLatch closed = new CountDownLatch(1);
        ExecutorService executor = Executors.newSingleThreadExecutor();
        try (Connection connection = DriverManager.getConnection("jdbc:h2:mem:blob-protocol-cancel");
             PipedInputStream serverInput = new PipedInputStream();
             PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
             PipedInputStream responseInput = new PipedInputStream();
             PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
             BufferedWriter requests = new BufferedWriter(new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
             BufferedReader responses = new BufferedReader(new InputStreamReader(responseInput, StandardCharsets.UTF_8));
             AgentServer server = new AgentServer(serverInput, serverOutput)) {
            AgentSession session = new AgentSession(connection, null);
            String blobId = session.registerBlob(connection.createBlob());
            InputStream blocked = new InputStream() {
                @Override
                public int read() throws IOException {
                    reading.countDown();
                    try {
                        if (!closed.await(5, TimeUnit.SECONDS)) {
                            throw new IOException("read was not cancelled");
                        }
                    } catch (InterruptedException exception) {
                        Thread.currentThread().interrupt();
                        throw new IOException("read interrupted", exception);
                    }
                    throw new IOException("read closed by cancellation");
                }

                @Override
                public void close() {
                    closed.countDown();
                }
            };
            String streamId = session.registerBlobInputStream(blobId, blocked);
            Field sessionsField = AgentServer.class.getDeclaredField("sessions");
            sessionsField.setAccessible(true);
            ((Map<String, AgentSession>) sessionsField.get(server)).put("blob-cancel-session", session);
            Future<?> serverTask = executor.submit(() -> {
                server.run();
                return null;
            });
            assertEquals("ready", read(responses).path("method").asText());
            ObjectNode readRequest = sessionOnly("blob-cancel-session");
            readRequest.put("streamId", streamId);
            readRequest.put("maxBytes", 1);
            write(requests, request(601, "blob_read_input_stream", readRequest));
            assertTrue(reading.await(5, TimeUnit.SECONDS));
            ObjectNode cancelRequest = sessionOnly("blob-cancel-session");
            cancelRequest.put("targetRequestId", 601);
            write(requests, request(602, "cancel", cancelRequest));
            JsonNode first = read(responses);
            JsonNode second = read(responses);
            JsonNode cancellation = first.path("id").asLong() == 602 ? first : second;
            JsonNode readResult = first.path("id").asLong() == 601 ? first : second;
            assertTrue(cancellation.at("/result/cancelled").asBoolean());
            assertEquals("HY008", readResult.at("/error/data/sqlState").asText());
            assertTrue(call(requests, responses, 603, "blob_read_input_stream", readRequest)
                    .path("error").isObject());
            requests.close();
            serverTask.get();
        } finally {
            closed.countDown();
            executor.shutdownNow();
        }
    }

    @Test
    @SuppressWarnings("unchecked")
    void cancelClosesActiveClobReaderAndReturnsCancellationSqlState() throws Exception {
        CountDownLatch reading = new CountDownLatch(1);
        CountDownLatch closed = new CountDownLatch(1);
        ExecutorService executor = Executors.newSingleThreadExecutor();
        try (Connection connection = DriverManager.getConnection("jdbc:h2:mem:clob-protocol-cancel");
             PipedInputStream serverInput = new PipedInputStream();
             PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
             PipedInputStream responseInput = new PipedInputStream();
             PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
             BufferedWriter requests = new BufferedWriter(new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
             BufferedReader responses = new BufferedReader(new InputStreamReader(responseInput, StandardCharsets.UTF_8));
             AgentServer server = new AgentServer(serverInput, serverOutput)) {
            AgentSession session = new AgentSession(connection, null);
            String clobId = session.registerClob(connection.createClob());
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
            Field sessionsField = AgentServer.class.getDeclaredField("sessions");
            sessionsField.setAccessible(true);
            ((Map<String, AgentSession>) sessionsField.get(server)).put("clob-cancel-session", session);
            Future<?> serverTask = executor.submit(() -> {
                server.run();
                return null;
            });
            assertEquals("ready", read(responses).path("method").asText());
            ObjectNode readRequest = sessionOnly("clob-cancel-session");
            readRequest.put("streamId", streamId);
            readRequest.put("maxUnits", 1);
            write(requests, request(861, "clob_read_reader", readRequest));
            assertTrue(reading.await(5, TimeUnit.SECONDS));
            ObjectNode cancelRequest = sessionOnly("clob-cancel-session");
            cancelRequest.put("targetRequestId", 861);
            write(requests, request(862, "cancel", cancelRequest));
            JsonNode first = read(responses);
            JsonNode second = read(responses);
            JsonNode cancellation = first.path("id").asLong() == 862 ? first : second;
            JsonNode readResult = first.path("id").asLong() == 861 ? first : second;
            assertTrue(cancellation.at("/result/cancelled").asBoolean());
            assertEquals("HY008", readResult.at("/error/data/sqlState").asText());
            assertTrue(call(requests, responses, 863, "clob_read_reader", readRequest)
                    .path("error").isObject());
            requests.close();
            serverTask.get();
        } finally {
            closed.countDown();
            executor.shutdownNow();
        }
    }

    @Test
    @SuppressWarnings("unchecked")
    void cancelClosesActiveClobAsciiInputAndReturnsCancellationSqlState() throws Exception {
        CountDownLatch reading = new CountDownLatch(1);
        CountDownLatch closed = new CountDownLatch(1);
        ExecutorService executor = Executors.newSingleThreadExecutor();
        try (Connection connection = DriverManager.getConnection("jdbc:hsqldb:mem:clob-ascii-protocol-cancel");
             PipedInputStream serverInput = new PipedInputStream();
             PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
             PipedInputStream responseInput = new PipedInputStream();
             PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
             BufferedWriter requests = new BufferedWriter(new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
             BufferedReader responses = new BufferedReader(new InputStreamReader(responseInput, StandardCharsets.UTF_8));
             AgentServer server = new AgentServer(serverInput, serverOutput)) {
            AgentSession session = new AgentSession(connection, null);
            String clobId = session.registerClob(connection.createClob());
            InputStream blocked = new InputStream() {
                @Override
                public int read() throws IOException {
                    reading.countDown();
                    try {
                        if (!closed.await(5, TimeUnit.SECONDS)) {
                            throw new IOException("read was not cancelled");
                        }
                    } catch (InterruptedException exception) {
                        Thread.currentThread().interrupt();
                        throw new IOException("read interrupted", exception);
                    }
                    throw new IOException("ASCII input closed by cancellation");
                }

                @Override
                public void close() {
                    closed.countDown();
                }
            };
            String streamId = session.registerClobInputStream(clobId, blocked);
            Field sessionsField = AgentServer.class.getDeclaredField("sessions");
            sessionsField.setAccessible(true);
            ((Map<String, AgentSession>) sessionsField.get(server)).put("clob-ascii-cancel-session", session);
            Future<?> serverTask = executor.submit(() -> {
                server.run();
                return null;
            });
            assertEquals("ready", read(responses).path("method").asText());
            ObjectNode readRequest = sessionOnly("clob-ascii-cancel-session");
            readRequest.put("streamId", streamId);
            readRequest.put("maxBytes", 1);
            write(requests, request(864, "clob_read_ascii_input", readRequest));
            assertTrue(reading.await(5, TimeUnit.SECONDS));
            ObjectNode cancelRequest = sessionOnly("clob-ascii-cancel-session");
            cancelRequest.put("targetRequestId", 864);
            write(requests, request(865, "cancel", cancelRequest));
            JsonNode first = read(responses);
            JsonNode second = read(responses);
            JsonNode cancellation = first.path("id").asLong() == 865 ? first : second;
            JsonNode readResult = first.path("id").asLong() == 864 ? first : second;
            assertTrue(cancellation.at("/result/cancelled").asBoolean());
            assertEquals("HY008", readResult.at("/error/data/sqlState").asText());
            assertTrue(call(requests, responses, 866, "clob_read_ascii_input", readRequest)
                    .path("error").isObject());
            requests.close();
            serverTask.get();
        } finally {
            closed.countDown();
            executor.shutdownNow();
        }
    }

    private JsonNode call(BufferedWriter requests, BufferedReader responses, long id,
            String method, ObjectNode params) throws Exception {
        write(requests, request(id, method, params));
        JsonNode response = read(responses);
        assertEquals(id, response.path("id").asLong());
        return response;
    }

    private ObjectNode request(long id, String method, JsonNode params) {
        ObjectNode request = JsonNodeFactory.instance.objectNode();
        request.put("jsonrpc", "2.0");
        request.put("id", id);
        request.put("method", method);
        request.set("params", params);
        return request;
    }

    private ObjectNode handshakePayload() {
        ObjectNode payload = JsonNodeFactory.instance.objectNode();
        payload.put("protocolVersion", 1);
        payload.put("client", "agent-test");
        payload.put("driverArtifactVersion", "test-h2");
        ArrayNode capabilities = payload.putArray("capabilities");
        capabilities.add("multi-session");
        capabilities.add("structured-errors");
        capabilities.add("tagged-values");
        capabilities.add("concurrent-requests");
        capabilities.add("cursor-paging");
        capabilities.add("cancel");
        capabilities.add("remote-prepare");
        capabilities.add("native-prepared-batch");
        return payload;
    }

    private boolean contains(JsonNode values, String expected) {
        for (JsonNode value : values) {
            if (expected.equals(value.asText())) {
                return true;
            }
        }
        return false;
    }

    private ObjectNode connectPayload() {
        ObjectNode payload = JsonNodeFactory.instance.objectNode();
        payload.put("url", "jdbc:h2:mem:agent;DB_CLOSE_DELAY=-1");
        payload.set("properties", JsonNodeFactory.instance.objectNode());
        payload.put("validationQuery", "SELECT 1");
        return payload;
    }

    private ObjectNode sessionPayload(String sessionId, ObjectNode payload) {
        payload.put("sessionId", sessionId);
        return payload;
    }

    private ObjectNode sessionOnly(String sessionId) {
        ObjectNode payload = JsonNodeFactory.instance.objectNode();
        payload.put("sessionId", sessionId);
        return payload;
    }

    private ObjectNode sqlPayload(String sql, ArrayNode parameters) {
        ObjectNode payload = JsonNodeFactory.instance.objectNode();
        payload.put("sql", sql);
        payload.set("params", parameters);
        payload.set("generatedKeys", generatedKeys());
        return payload;
    }

    private ObjectNode generatedKeys() {
        ObjectNode generatedKeys = JsonNodeFactory.instance.objectNode();
        generatedKeys.put("mode", "none");
        return generatedKeys;
    }

    private ArrayNode parameters() {
        ArrayNode parameters = JsonNodeFactory.instance.arrayNode();
        parameters.add(objectMapper.valueToTree(new AgentValue(
                "int", JsonNodeFactory.instance.numberNode(7))));
        parameters.add(objectMapper.valueToTree(new AgentValue(
                "string", JsonNodeFactory.instance.textNode("sample"))));
        return parameters;
    }

    private void write(BufferedWriter output, JsonNode request) throws Exception {
        output.write(objectMapper.writeValueAsString(request));
        output.newLine();
        output.flush();
    }

    private JsonNode read(BufferedReader input) throws Exception {
        return objectMapper.readTree(input.readLine());
    }
}
