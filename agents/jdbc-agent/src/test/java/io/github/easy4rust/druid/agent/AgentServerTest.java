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
import java.io.OutputStreamWriter;
import java.io.PipedInputStream;
import java.io.PipedOutputStream;
import java.nio.charset.StandardCharsets;
import java.net.SocketTimeoutException;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.SQLIntegrityConstraintViolationException;
import java.sql.SQLTransientConnectionException;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.sql.Statement;
import javax.sql.XAConnection;
import javax.transaction.xa.XAResource;
import javax.transaction.xa.Xid;
import java.util.Arrays;
import java.util.Objects;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;

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
