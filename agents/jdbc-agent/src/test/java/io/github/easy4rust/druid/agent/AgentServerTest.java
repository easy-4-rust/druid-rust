package io.github.easy4rust.druid.agent;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.JsonNodeFactory;
import com.fasterxml.jackson.databind.node.ObjectNode;
import org.junit.jupiter.api.Test;

import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.io.PipedInputStream;
import java.io.PipedOutputStream;
import java.nio.charset.StandardCharsets;
import java.net.SocketTimeoutException;
import java.sql.SQLIntegrityConstraintViolationException;
import java.sql.SQLTransientConnectionException;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** JDBC Agent JSON-RPC、多 session 与真实 H2 JDBC 驱动的最小契约测试。 */
class AgentServerTest {

    private final ObjectMapper objectMapper = new ObjectMapper();

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
            read(responses);
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
            assertEquals(true, cancelResponse.at("/result/cancelled").asBoolean());

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
