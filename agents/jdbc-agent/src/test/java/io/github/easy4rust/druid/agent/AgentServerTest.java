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
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;

/** JDBC Agent JSON-RPC、多 session 与真实 H2 JDBC 驱动的最小契约测试。 */
class AgentServerTest {

    private final ObjectMapper objectMapper = new ObjectMapper();

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
            assertEquals(1, read(responses).path("result").path("protocolVersion").asInt());

            write(requests, request(2, "session.open", connectPayload()));
            String firstSession = read(responses).at("/result/sessionId").asText();
            write(requests, request(3, "session.open", connectPayload()));
            String secondSession = read(responses).at("/result/sessionId").asText();
            assertNotEquals(firstSession, secondSession);

            write(requests, request(4, "session.exec", sessionPayload(
                    firstSession,
                    sqlPayload("CREATE TABLE sample(id BIGINT PRIMARY KEY, name VARCHAR(32))",
                            JsonNodeFactory.instance.arrayNode()))));
            assertEquals(0, read(responses).at("/result/rowsAffected").asLong());
            write(requests, request(5, "session.exec", sessionPayload(
                    firstSession,
                    sqlPayload("INSERT INTO sample(id, name) VALUES (?, ?)", parameters()))));
            assertEquals(1, read(responses).at("/result/rowsAffected").asLong());

            write(requests, request(6, "session.fetch", sessionPayload(
                    secondSession,
                    sqlPayload("SELECT id, name FROM sample ORDER BY id",
                            JsonNodeFactory.instance.arrayNode()))));
            JsonNode query = read(responses).path("result");
            assertEquals(1, query.path("rows").size());
            assertEquals("sample", query.path("rows").path(0).path(1).path("value").asText());

            write(requests, request(7, "session.close", sessionOnly(firstSession)));
            read(responses);
            write(requests, request(8, "session.close", sessionOnly(secondSession)));
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
        return payload;
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
        return payload;
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
