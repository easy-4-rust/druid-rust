package io.github.easy4rust.druid.agent;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.ObjectNode;
import org.junit.jupiter.api.Test;
import org.testcontainers.postgresql.PostgreSQLContainer;

import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.io.PipedInputStream;
import java.io.PipedOutputStream;
import java.nio.charset.StandardCharsets;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.ResultSet;
import java.sql.Statement;
import java.util.Arrays;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** 验证真实 JDBC Agent XA 协议可跨 session 恢复 PostgreSQL prepared transaction。 */
class XaAgentProtocolTest {

    private final ObjectMapper objectMapper = new ObjectMapper();

    @Test
    void preservesXaErrorCodeInStructuredErrorPayload() {
        javax.transaction.xa.XAException failure = new javax.transaction.xa.XAException("duplicate xid");
        failure.errorCode = javax.transaction.xa.XAException.XAER_DUPID;
        AgentError error = AgentError.from(17, "session", failure);
        assertEquals(javax.transaction.xa.XAException.XAER_DUPID, error.vendorCode());
        assertEquals("javax.transaction.xa.XAException", error.exceptionClass());
    }

    @Test
    void recoversAndCommitsPreparedBranchAcrossAgentSessions() throws Exception {
        PostgreSQLContainer postgres = new PostgreSQLContainer("postgres:18.6")
                .withDatabaseName("xa_agent")
                .withUsername("xa_user")
                .withPassword("xa_password")
                .withCommand("postgres", "-c", "max_prepared_transactions=10");
        ExecutorService executor = Executors.newSingleThreadExecutor();
        try {
            postgres.start();
            try (Connection setup = DriverManager.getConnection(
                    postgres.getJdbcUrl(), postgres.getUsername(), postgres.getPassword());
                 Statement statement = setup.createStatement()) {
                statement.execute("CREATE TABLE agent_xa(id INT PRIMARY KEY, payload VARCHAR(32))");
            }

            try (PipedInputStream serverInput = new PipedInputStream();
                 PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
                 PipedInputStream responseInput = new PipedInputStream();
                 PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
                 BufferedWriter requests = new BufferedWriter(
                         new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
                 BufferedReader responses = new BufferedReader(
                         new InputStreamReader(responseInput, StandardCharsets.UTF_8));
                 AgentServer server = new AgentServer(serverInput, serverOutput)) {
                Future<?> serverTask = executor.submit(() -> {
                    server.run();
                    return null;
                });
                assertEquals("ready", read(responses).path("method").asText());

                JsonNode firstSession = rpc(requests, responses, 1, "open_session", connectPayload(postgres));
                String firstSessionId = firstSession.at("/result/sessionId").asText();
                assertTrue(firstSession.at("/result/capabilities/xa").asBoolean(),
                        "XA must only be advertised when an XADataSource created this owner");
                ObjectNode sameRm = sessionPayload(firstSessionId);
                sameRm.put("otherSessionId", firstSessionId);
                assertTrue(rpc(requests, responses, 2, "xa_is_same_rm", sameRm)
                        .at("/result/sameResourceManager").asBoolean());
                ObjectNode timeout = sessionPayload(firstSessionId);
                timeout.put("seconds", 0);
                rpc(requests, responses, 3, "xa_set_timeout", timeout);
                assertTrue(rpc(requests, responses, 4, "xa_get_timeout", sessionPayload(firstSessionId))
                        .at("/result/value").canConvertToInt());

                ObjectNode xid = xid();
                ObjectNode start = sessionPayload(firstSessionId);
                start.set("xid", xid.deepCopy());
                start.put("flags", javax.transaction.xa.XAResource.TMNOFLAGS);
                assertEquals(1, rpc(requests, responses, 5, "xa_start", start).path("result").path("started").asInt());

                ObjectNode insert = sessionPayload(firstSessionId);
                insert.put("sql", "INSERT INTO agent_xa(id, payload) VALUES (1, 'protocol')");
                insert.putArray("params");
                assertEquals(1, rpc(requests, responses, 6, "execute_update", insert)
                        .at("/result/rowsAffected").asInt());

                ObjectNode end = sessionPayload(firstSessionId);
                end.set("xid", xid.deepCopy());
                end.put("flags", javax.transaction.xa.XAResource.TMSUCCESS);
                rpc(requests, responses, 7, "xa_end", end);
                rpc(requests, responses, 8, "close_logical_connection", sessionPayload(firstSessionId));
                JsonNode closedLogicalUse = rpcError(
                        requests, responses, 9, "get_auto_commit", sessionPayload(firstSessionId));
                assertEquals("java.sql.SQLException",
                        closedLogicalUse.at("/error/data/exceptionClass").asText());
                ObjectNode prepare = sessionPayload(firstSessionId);
                prepare.set("xid", xid.deepCopy());
                assertEquals(javax.transaction.xa.XAResource.XA_OK,
                        rpc(requests, responses, 10, "xa_prepare", prepare).at("/result/vote").asInt());
                rpc(requests, responses, 11, "reopen_logical_connection", sessionPayload(firstSessionId));
                rpc(requests, responses, 12, "close_session", sessionPayload(firstSessionId));

                JsonNode secondSession = rpc(requests, responses, 13, "open_session", connectPayload(postgres));
                String secondSessionId = secondSession.at("/result/sessionId").asText();
                ObjectNode recover = sessionPayload(secondSessionId);
                recover.put("flags", javax.transaction.xa.XAResource.TMSTARTRSCAN);
                ArrayNode firstPage = (ArrayNode) rpc(requests, responses, 14, "xa_recover", recover)
                        .at("/result/xids");
                recover.put("flags", javax.transaction.xa.XAResource.TMNOFLAGS);
                ArrayNode nextPage = (ArrayNode) rpc(requests, responses, 15, "xa_recover", recover)
                        .at("/result/xids");
                recover.put("flags", javax.transaction.xa.XAResource.TMENDRSCAN);
                rpc(requests, responses, 16, "xa_recover", recover);
                assertTrue(containsXid(firstPage, xid) || containsXid(nextPage, xid));

                ObjectNode commit = sessionPayload(secondSessionId);
                commit.set("xid", xid.deepCopy());
                commit.put("onePhase", false);
                rpc(requests, responses, 17, "xa_commit", commit);
                rpc(requests, responses, 18, "close_session", sessionPayload(secondSessionId));
                rpc(requests, responses, 19, "shutdown", objectMapper.createObjectNode());
                requests.close();
                serverTask.get();
            }

            try (Connection verification = DriverManager.getConnection(
                    postgres.getJdbcUrl(), postgres.getUsername(), postgres.getPassword());
                 Statement statement = verification.createStatement();
                 ResultSet rows = statement.executeQuery(
                         "SELECT COUNT(*) FROM agent_xa WHERE id=1 AND payload='protocol'")) {
                assertTrue(rows.next());
                assertEquals(1, rows.getInt(1));
            }

            try (PipedInputStream serverInput = new PipedInputStream();
                 PipedOutputStream requestOutput = new PipedOutputStream(serverInput);
                 PipedInputStream responseInput = new PipedInputStream();
                 PipedOutputStream serverOutput = new PipedOutputStream(responseInput);
                 BufferedWriter requests = new BufferedWriter(
                         new OutputStreamWriter(requestOutput, StandardCharsets.UTF_8));
                 BufferedReader responses = new BufferedReader(
                         new InputStreamReader(responseInput, StandardCharsets.UTF_8));
                 AgentServer server = new AgentServer(serverInput, serverOutput)) {
                Future<?> serverTask = executor.submit(() -> {
                    server.run();
                    return null;
                });
                assertEquals("ready", read(responses).path("method").asText());
                ObjectNode ordinary = objectMapper.createObjectNode();
                ordinary.put("url", postgres.getJdbcUrl());
                ordinary.putObject("properties").put("user", postgres.getUsername())
                        .put("password", postgres.getPassword());
                JsonNode opened = rpc(requests, responses, 1, "open_session", ordinary);
                String sessionId = opened.at("/result/sessionId").asText();
                assertTrue(!opened.at("/result/capabilities/xa").asBoolean());
                ObjectNode xaStart = sessionPayload(sessionId);
                xaStart.set("xid", xid());
                xaStart.put("flags", javax.transaction.xa.XAResource.TMNOFLAGS);
                JsonNode rejected = rpcError(requests, responses, 2, "xa_start", xaStart);
                assertEquals("0A000", rejected.at("/error/data/sqlState").asText());
                rpc(requests, responses, 3, "close_session", sessionPayload(sessionId));
                rpc(requests, responses, 4, "shutdown", objectMapper.createObjectNode());
                requests.close();
                serverTask.get();
            }
        } finally {
            executor.shutdownNow();
            postgres.stop();
        }
    }

    private JsonNode rpc(
            BufferedWriter requests,
            BufferedReader responses,
            long id,
            String method,
            ObjectNode params) throws Exception {
        ObjectNode request = objectMapper.createObjectNode();
        request.put("jsonrpc", "2.0");
        request.put("id", id);
        request.put("method", method);
        request.set("params", params);
        requests.write(objectMapper.writeValueAsString(request));
        requests.newLine();
        requests.flush();
        JsonNode response = read(responses);
        assertTrue(response.path("error").isMissingNode() || response.path("error").isNull(),
                response.path("error").toString());
        return response;
    }

    private JsonNode rpcError(
            BufferedWriter requests,
            BufferedReader responses,
            long id,
            String method,
            ObjectNode params) throws Exception {
        ObjectNode request = objectMapper.createObjectNode();
        request.put("jsonrpc", "2.0");
        request.put("id", id);
        request.put("method", method);
        request.set("params", params);
        requests.write(objectMapper.writeValueAsString(request));
        requests.newLine();
        requests.flush();
        JsonNode response = read(responses);
        assertTrue(response.path("error").isObject(), response.toString());
        return response;
    }

    private JsonNode read(BufferedReader responses) throws Exception {
        String line = responses.readLine();
        if (line == null) {
            throw new IllegalStateException("JDBC Agent closed its response stream");
        }
        return objectMapper.readTree(line);
    }

    private ObjectNode connectPayload(PostgreSQLContainer postgres) {
        ObjectNode payload = objectMapper.createObjectNode();
        payload.put("url", postgres.getJdbcUrl());
        payload.putObject("properties").put("user", postgres.getUsername())
                .put("password", postgres.getPassword());
        payload.put("xaDataSourceClassName", "org.postgresql.xa.PGXADataSource");
        ObjectNode xaProperties = payload.putObject("xaDataSourceProperties");
        ArrayNode servers = xaProperties.putArray("serverNames");
        servers.add(postgres.getHost());
        ArrayNode ports = xaProperties.putArray("portNumbers");
        ports.add(postgres.getMappedPort(5432));
        xaProperties.put("databaseName", postgres.getDatabaseName());
        xaProperties.put("user", postgres.getUsername());
        xaProperties.put("password", postgres.getPassword());
        return payload;
    }

    private ObjectNode sessionPayload(String sessionId) {
        ObjectNode payload = objectMapper.createObjectNode();
        payload.put("sessionId", sessionId);
        return payload;
    }

    private ObjectNode xid() {
        ObjectNode xid = objectMapper.createObjectNode();
        xid.put("formatId", 0x44525549);
        xid.putArray("globalTransactionId").add(1).add(2).add(3);
        xid.putArray("branchQualifier").add(4).add(5);
        return xid;
    }

    private boolean sameXid(JsonNode expected, JsonNode actual) {
        return expected.path("formatId").asInt() == actual.path("formatId").asInt()
                && Arrays.equals(bytes(expected.path("globalTransactionId")), bytes(actual.path("globalTransactionId")))
                && Arrays.equals(bytes(expected.path("branchQualifier")), bytes(actual.path("branchQualifier")));
    }

    private boolean containsXid(ArrayNode values, JsonNode expected) {
        for (JsonNode value : values) {
            if (sameXid(expected, value)) {
                return true;
            }
        }
        return false;
    }

    private byte[] bytes(JsonNode values) {
        byte[] result = new byte[values.size()];
        for (int index = 0; index < values.size(); index++) {
            result[index] = (byte) values.get(index).asInt();
        }
        return result;
    }
}
