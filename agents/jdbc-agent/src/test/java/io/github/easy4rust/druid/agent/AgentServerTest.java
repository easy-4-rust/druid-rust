package io.github.easy4rust.druid.agent;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.JsonNodeFactory;
import com.fasterxml.jackson.databind.node.ObjectNode;
import org.junit.jupiter.api.Test;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.DataInputStream;
import java.io.DataOutputStream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** JDBC Agent 进程协议与真实 H2 JDBC 驱动的最小契约测试。 */
class AgentServerTest {

    private final ObjectMapper objectMapper = new ObjectMapper();

    @Test
    void executesFramedJdbcLifecycle() throws Exception {
        ByteArrayOutputStream requestBytes = new ByteArrayOutputStream();
        try (DataOutputStream requests = new DataOutputStream(requestBytes)) {
            write(requests, request(1, "connect", connectPayload()));
            write(requests, request(2, "exec", sqlPayload(
                    "CREATE TABLE sample(id BIGINT PRIMARY KEY, name VARCHAR(32))",
                    JsonNodeFactory.instance.arrayNode())));
            write(requests, request(3, "exec", sqlPayload(
                    "INSERT INTO sample(id, name) VALUES (?, ?)",
                    parameters())));
            write(requests, request(4, "fetch", sqlPayload(
                    "SELECT id, name FROM sample ORDER BY id",
                    JsonNodeFactory.instance.arrayNode())));
            write(requests, request(5, "close", JsonNodeFactory.instance.nullNode()));
        }

        ByteArrayOutputStream responseBytes = new ByteArrayOutputStream();
        try (AgentServer server = new AgentServer(
                new ByteArrayInputStream(requestBytes.toByteArray()), responseBytes)) {
            server.run();
        }

        try (DataInputStream responses = new DataInputStream(
                new ByteArrayInputStream(responseBytes.toByteArray()))) {
            assertTrue(read(responses).success());
            assertTrue(read(responses).success());
            AgentResponse insert = read(responses);
            assertEquals(1, insert.payload().path("rowsAffected").asLong());
            AgentResponse query = read(responses);
            assertEquals(1, query.payload().path("rows").size());
            assertEquals("sample", query.payload().path("rows").path(0).path(1).path("value").asText());
            assertTrue(read(responses).success());
        }
    }

    private AgentRequest request(long requestId, String operation, JsonNode payload) {
        return new AgentRequest(1, requestId, operation, payload);
    }

    private ObjectNode connectPayload() {
        ObjectNode payload = JsonNodeFactory.instance.objectNode();
        payload.put("url", "jdbc:h2:mem:agent;DB_CLOSE_DELAY=-1");
        payload.set("properties", JsonNodeFactory.instance.objectNode());
        payload.put("validationQuery", "SELECT 1");
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

    private void write(DataOutputStream output, AgentRequest request) throws Exception {
        byte[] frame = objectMapper.writeValueAsBytes(request);
        output.writeInt(frame.length);
        output.write(frame);
    }

    private AgentResponse read(DataInputStream input) throws Exception {
        int length = input.readInt();
        return objectMapper.readValue(input.readNBytes(length), AgentResponse.class);
    }
}
